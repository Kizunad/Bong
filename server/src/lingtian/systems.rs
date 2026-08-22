//! plan-lingtian-v1 P1 ECS — 把事件 / session 状态机接到 ECS 世界。
//!
//! 职责：
//!   * `handle_start_till` / `handle_start_renew` —— 收意图请求 → 验前置 → 起 session
//!   * `tick_lingtian_sessions` —— 每 Update tick 推进所有活跃 session
//!   * `apply_completed_sessions` —— Finished 的 session：spawn / reset Plot Entity，
//!     扣玩家主手锄耐久（归零则从 equipped 移除）
//!
//! 单 actor 单 session：`ActiveLingtianSessions` 以 actor Entity 为 key，
//! 进新请求时若已有活 session 直接拒。
//!
//! plot 实体：当前切片把 LingtianPlot 作为独立 Entity（`spawn(LingtianPlot, ...)`）
//! 而非真正的 valence BlockEntity（后者依 plan-persistence-v1）。Renew 通过
//! `Query<&mut LingtianPlot>` 按 BlockPos 反查匹配 plot。

use std::collections::{HashMap, HashSet};

use valence::prelude::bevy_ecs::system::SystemParam;
use valence::prelude::{
    bevy_ecs, Added, BlockPos, BlockState, ChunkLayer, Client, Commands, DVec3, Despawned, Entity,
    EventReader, EventWriter, Events, ParamSet, Position, Query, Res, ResMut, Resource, Username,
    With, Without,
};

use crate::alchemy::residue::{consume_one_residue, inventory_has_usable_residue};
use crate::botany::{PlantId, PlantKindRegistry};
use crate::combat::CombatClock;
use crate::cultivation::components::Cultivation;
use crate::cultivation::life_record::{BiographyEntry, LifeRecord};
use crate::inventory::{
    add_item_to_player_inventory, InventoryInstanceIdAllocator, ItemRegistry, PlayerInventory,
};
use crate::network::inventory_snapshot_emit::send_inventory_snapshot_to_client;
use crate::network::{gameplay_vfx, vfx_event_emit::VfxEventRequest};
use crate::npc::lifecycle::NpcTerminalSettlementSucceeded;
use crate::npc::spawn::NpcMarker;
use crate::player::state::{canonical_player_id, PlayerState};
use crate::qi_physics::{
    constants::QI_NPC_ABSORB_FLOOR, QiAccountId, QiTransfer, QiTransferReason,
};
use crate::schema::common::GameEventType;
use crate::schema::world_state::GameEvent;
use crate::skill::components::{SkillId, SkillSet};
use crate::skill::events::{SkillXpGain, XpGainSource};
use crate::world::events::ActiveEventsResource;

use super::contamination::{apply_dye_contamination_on_replenish, dye_contamination_decay_tick};
use super::environment::read_environment_at;
use super::environment::{compute_plot_qi_cap, PlotEnvironment};
use super::events::{
    DrainQiCompleted, DyeContaminationWarning, HarvestCompleted, PlantingCompleted, RenewCompleted,
    ReplenishCompleted, StartDrainQiRequest, StartHarvestRequest, StartPlantingRequest,
    StartRenewRequest, StartReplenishRequest, StartTillRequest, TillCompleted, ZonePressureCrossed,
};
use super::growth::advance_one_lingtian_tick;
use super::hoe::HoeKind;
use super::network_emit::replenish_source_wire;
use super::plot::{CropInstance, LingtianPlot};
use super::pressure::{
    compute_zone_pressure, derive_supply_jitter, PressureLevel, ZonePressureTracker,
};
use super::qi_account::{
    LingtianTickAccumulator, ZoneQiAccount, BEVY_TICKS_PER_LINGTIAN_TICK, DEFAULT_ZONE,
};
use super::range_gate::{log_lingtian_interaction_denial, validate_lingtian_interaction};
use super::requests::{LingtianDispatchWriters, PendingLingtianRequest, PendingLingtianRequests};
use super::seed::{seed_id_for, SeedRegistry};
use super::session::{
    DrainQiSession, HarvestSession, PlantingSession, RenewSession, ReplenishSession,
    ReplenishSource, SessionMode, TillSession, DRAIN_QI_TO_PLAYER_RATIO, DRAIN_QI_TO_ZONE_RATIO,
    REPLENISH_COOLDOWN_LINGTIAN_TICKS,
};
use super::terrain::{classify_for_till, terrain_from_block_kind, TerrainKind};
use crate::world::dimension::CurrentDimension;
use crate::world::events::EVENT_REALM_COLLAPSE;
use crate::world::zone::ZoneRegistry;

const LING_SHUI_ITEM_ID: &str = "ling_shui";
const BEAST_CORE_ITEM_ID: &str = "mutant_beast_core";

const MAIN_HAND_SLOT: &str = "main_hand";

#[cfg(test)]
#[derive(Resource, Default)]
pub(crate) struct StartHandlerPlotScanCount {
    index_builds: usize,
    scanned_plots: usize,
}

#[cfg(not(test))]
fn build_start_plot_index<'a>(
    plots: impl Iterator<Item = &'a LingtianPlot>,
) -> HashMap<BlockPos, Vec<&'a LingtianPlot>> {
    plots.fold(HashMap::new(), |mut index, plot| {
        index.entry(plot.pos).or_default().push(plot);
        index
    })
}

#[cfg(test)]
fn build_start_plot_index<'a>(
    plots: impl Iterator<Item = &'a LingtianPlot>,
    mut plot_scan_count: Option<&mut StartHandlerPlotScanCount>,
) -> HashMap<BlockPos, Vec<&'a LingtianPlot>> {
    if let Some(count) = plot_scan_count.as_deref_mut() {
        count.index_builds += 1;
    }
    plots
        .inspect(|_| {
            if let Some(count) = plot_scan_count.as_deref_mut() {
                count.scanned_plots += 1;
            }
        })
        .fold(HashMap::new(), |mut index, plot| {
            index.entry(plot.pos).or_default().push(plot);
            index
        })
}

fn plot_zone_key(plot: &LingtianPlot) -> &str {
    let zone = plot.zone.trim();
    if zone.is_empty() {
        DEFAULT_ZONE
    } else {
        zone
    }
}

fn plot_zone_key_at(plots: &Query<&LingtianPlot>, pos: &valence::prelude::BlockPos) -> String {
    plots
        .iter()
        .find(|plot| plot.pos == *pos)
        .map(|plot| plot_zone_key(plot).to_string())
        .unwrap_or_else(|| DEFAULT_ZONE.to_string())
}

#[derive(Debug)]
pub enum ActiveSession {
    Till(TillSession),
    Renew(RenewSession),
    Planting(PlantingSession),
    Harvest(HarvestSession),
    Replenish(ReplenishSession),
    DrainQi(DrainQiSession),
}

impl ActiveSession {
    fn tick(&mut self) {
        match self {
            ActiveSession::Till(s) => s.tick(),
            ActiveSession::Renew(s) => s.tick(),
            ActiveSession::Planting(s) => s.tick(),
            ActiveSession::Harvest(s) => s.tick(),
            ActiveSession::Replenish(s) => s.tick(),
            ActiveSession::DrainQi(s) => s.tick(),
        }
    }

    fn is_finished(&self) -> bool {
        match self {
            ActiveSession::Till(s) => s.is_finished(),
            ActiveSession::Renew(s) => s.is_finished(),
            ActiveSession::Planting(s) => s.is_finished(),
            ActiveSession::Harvest(s) => s.is_finished(),
            ActiveSession::Replenish(s) => s.is_finished(),
            ActiveSession::DrainQi(s) => s.is_finished(),
        }
    }

    fn position(&self) -> valence::prelude::BlockPos {
        match self {
            ActiveSession::Till(s) => s.pos,
            ActiveSession::Renew(s) => s.pos,
            ActiveSession::Planting(s) => s.pos,
            ActiveSession::Harvest(s) => s.pos,
            ActiveSession::Replenish(s) => s.pos,
            ActiveSession::DrainQi(s) => s.pos,
        }
    }
}

/// 累计的 lingtian-tick（lingtian_growth_tick 触发时 ++）。用于补灵冷却比对。
#[derive(Debug, Default, Resource)]
pub struct LingtianClock {
    pub lingtian_tick: u64,
}

// ============================================================================
// fix-spec-1901-v2 §4.3 — 唯一的 post-transfer validator
// ============================================================================

/// 唯一生产 emit site：把 `PendingLingtianRequests` 批次（FIFO）逐条过
/// post-transfer gate 后转为六类 `Start*Request`。
///
/// 调度合同（mod.rs）：本系统位于 `LingtianPostTransferValidationSet`，
/// 排在 `AuthoritativePositionCommitSet` 之后、`LingtianStartSet` 之前——
/// 读到的 `Position` / `CurrentDimension` 是本 tick 的最终权威状态。
///
/// 对 batch 中每个请求：
/// 1. 非 live client（断线 / despawn）直接丢弃；
/// 2. `validate_lingtian_interaction` gate 失败只 log 并继续（不读 plot /
///    inventory / terrain，不建 session，不发完成事件）；
/// 3. Till 只有在 gate 成功后才读 `OverworldLayer.block(pos)` 派生真实
///    `TerrainKind` / `PlotEnvironment`；layer 缺失走既有 `Unknown` fallback，
///    由 Till business handler 拒绝；
/// 4. gate 成功后才发对应的 `Start*Request` event。
///
/// 同 actor 同批多个 gate-valid 请求严格按 ingress sequence 处理：本 tick 只 dispatch
/// 第一条，其余保序放回下一批队首。这样六类 typed event handler 的内部调度顺序不会
/// 反转跨 action FIFO；首条业务 handler 本 tick 失败时，下一条仍会在下 tick 获得机会。
#[allow(clippy::too_many_arguments)]
pub fn validate_and_dispatch_lingtian_requests(
    mut pending: ResMut<PendingLingtianRequests>,
    positions: Query<&Position>,
    dimensions: Query<&CurrentDimension>,
    clients: Query<(), (With<Client>, Without<Despawned>)>,
    layers: Query<&ChunkLayer, With<crate::world::dimension::OverworldLayer>>,
    mut writers: LingtianDispatchWriters,
) {
    let batch = pending.take_batch();
    let mut dispatched_actors = HashSet::new();
    let mut deferred = std::collections::VecDeque::new();
    for request in batch {
        let (actor, pos) = request.actor_and_pos();
        if clients.get(actor).is_err() {
            // 客户端已断线或 entity 已 despawn：不产生 start event。
            tracing::debug!(
                "[bong][lingtian] pending request dropped: actor={actor:?} is not a live client"
            );
            continue;
        }
        if let Err(reason) = validate_lingtian_interaction(actor, pos, &positions, &dimensions) {
            log_lingtian_interaction_denial("post-transfer", actor, pos, reason);
            continue;
        }
        if dispatched_actors.contains(&actor) {
            deferred.push_back(request);
            continue;
        }
        dispatched_actors.insert(actor);
        match request {
            PendingLingtianRequest::Till {
                actor,
                pos,
                hoe_instance_id,
                mode,
                ..
            } => {
                // gate 成功后才读 chunk 派生真实地形（避免 layer lookup 回到
                // gate 之前；layer 缺失走 Unknown fallback 让业务 handler 拒）。
                let (terrain, environment) = match layers.get_single() {
                    Ok(layer) => {
                        let terrain = layer
                            .block(pos)
                            .map(|b| terrain_from_block_kind(b.state.to_kind()))
                            .unwrap_or(TerrainKind::Unknown);
                        (terrain, read_environment_at(layer, pos))
                    }
                    Err(err) => {
                        tracing::warn!(
                            "[bong][lingtian] validate_and_dispatch: chunk layer unavailable ({err:?}); \
                             falling back to Unknown terrain — session will reject."
                        );
                        (TerrainKind::Unknown, PlotEnvironment::base())
                    }
                };
                writers.till.send(StartTillRequest {
                    player: actor,
                    pos,
                    hoe_instance_id,
                    mode,
                    terrain,
                    environment,
                });
            }
            PendingLingtianRequest::Renew {
                actor,
                pos,
                hoe_instance_id,
            } => {
                writers.renew.send(StartRenewRequest {
                    player: actor,
                    pos,
                    hoe_instance_id,
                });
            }
            PendingLingtianRequest::Planting {
                actor,
                pos,
                plant_id,
            } => {
                writers.planting.send(StartPlantingRequest {
                    player: actor,
                    pos,
                    plant_id,
                });
            }
            PendingLingtianRequest::Harvest { actor, pos, mode } => {
                writers.harvest.send(StartHarvestRequest {
                    player: actor,
                    pos,
                    mode,
                });
            }
            PendingLingtianRequest::Replenish { actor, pos, source } => {
                writers.replenish.send(StartReplenishRequest {
                    player: actor,
                    pos,
                    source,
                });
            }
            PendingLingtianRequest::DrainQi { actor, pos } => {
                writers
                    .drain_qi
                    .send(StartDrainQiRequest { player: actor, pos });
            }
        }
    }
    pending.prepend_batch(deferred);
}

/// session 完成事件写出 — 6 类合一以避开 Bevy 16 system-param 限制。
#[derive(SystemParam)]
pub struct CompletionEventWriters<'w> {
    pub till: EventWriter<'w, TillCompleted>,
    pub renew: EventWriter<'w, RenewCompleted>,
    pub planting: EventWriter<'w, PlantingCompleted>,
    pub harvest: EventWriter<'w, HarvestCompleted>,
    pub replenish: EventWriter<'w, ReplenishCompleted>,
    pub drain_qi: EventWriter<'w, DrainQiCompleted>,
    pub dye_warning: EventWriter<'w, DyeContaminationWarning>,
    pub qi_transfer: EventWriter<'w, QiTransfer>,
    pub vfx_events: Option<ResMut<'w, Events<VfxEventRequest>>>,
}

/// Actor components used to revalidate player sessions before completion.
#[derive(SystemParam)]
pub struct CompletionActorQueries<'w, 's> {
    pub positions: Query<'w, 's, &'static Position>,
    pub dimensions: Query<'w, 's, &'static CurrentDimension>,
    pub clients: Query<'w, 's, (), (With<Client>, Without<Despawned>)>,
    pub npcs: Query<'w, 's, (), (With<NpcMarker>, Without<Despawned>)>,
}

/// 灵田逻辑时间：冷却仍用 lingtian-tick，残料保鲜用真实 server tick。
#[derive(SystemParam)]
pub struct LingtianTime<'w> {
    clock: Res<'w, LingtianClock>,
    combat_clock: Option<Res<'w, CombatClock>>,
}

impl LingtianTime<'_> {
    fn lingtian_tick(&self) -> u64 {
        self.clock.lingtian_tick
    }

    fn residue_tick(&self) -> u64 {
        residue_now_tick(self.combat_clock.as_deref(), &self.clock)
    }
}

/// Completion context shared by time-dependent settlement and actor revalidation.
#[derive(SystemParam)]
pub struct CompletionContext<'w, 's> {
    pub time: LingtianTime<'w>,
    pub actor_queries: CompletionActorQueries<'w, 's>,
}

/// xorshift64 — 确定性 RNG，用于种子掉落决策。测试可注入种子。
#[derive(Debug, Resource)]
pub struct LingtianHarvestRng {
    state: u64,
}

impl LingtianHarvestRng {
    pub fn new(seed: u64) -> Self {
        Self { state: seed.max(1) }
    }

    pub fn next_f32(&mut self) -> f32 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        // 取低 24 位避免 f32 精度噪音 → [0, 1)
        ((x & 0x00FF_FFFF) as f32) / (0x0100_0000_u32 as f32)
    }
}

impl Default for LingtianHarvestRng {
    fn default() -> Self {
        // 某个磨过的"魔数"，只要每次启动一致即可
        Self::new(0x9E37_79B9_7F4A_7C15)
    }
}

#[derive(Debug, Default, Resource)]
pub struct ActiveLingtianSessions {
    by_actor: HashMap<Entity, ActiveSession>,
    /// fix-spec-1901-v2 §6.2 — target-level Till reservation。
    ///
    /// 只覆盖会创建新 plot 的 Till：同一个 `BlockPos` 在同一时刻只允许一个
    /// 起 session 的 agent（player 与 NPC 共用，不允许跨入口绕过）。
    ///
    /// 生命周期：session 从插入到完成结算（含 deferred `commands.spawn(plot)`
    /// 未应用的窗口）都占用 reservation。`drain_finished` 把 session 移出
    /// `by_actor` 后，reservation 仍保留到该 block 已实际存在 `LingtianPlot`；
    /// 取消 / 超时（`clear`）立即释放，失败完成（如 completion gate 拒绝、
    /// 目标已有 plot）同步释放，避免永久锁死。
    reserved_targets: HashMap<valence::prelude::BlockPos, Entity>,
}

impl ActiveLingtianSessions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn has_session(&self, actor: Entity) -> bool {
        self.by_actor.contains_key(&actor)
    }

    pub fn get(&self, actor: Entity) -> Option<&ActiveSession> {
        self.by_actor.get(&actor)
    }

    pub fn len(&self) -> usize {
        self.by_actor.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_actor.is_empty()
    }

    /// 已保留但尚无 plot 的 block 数量（只用于诊断/测试）。
    pub fn pending_reservations(&self) -> usize {
        self.reserved_targets.len()
    }

    /// 插入非 Till session。Till 必须走 [`Self::try_insert_till`]，避免调用方
    /// 绕过已有 plot 与 target reservation 的统一门禁。
    pub fn try_insert(&mut self, actor: Entity, session: ActiveSession) -> bool {
        if matches!(session, ActiveSession::Till(_)) {
            tracing::warn!(
                "[bong][lingtian] try_insert rejected Till for actor={actor:?}; use try_insert_till"
            );
            return false;
        }
        if self.by_actor.contains_key(&actor) {
            return false;
        }
        self.by_actor.insert(actor, session);
        true
    }

    /// 原子插入 Till session：actor 空闲、目标尚无 plot 且未被其他 actor 保留
    /// 时才同时写入 session 与 reservation。玩家和 NPC producer 共用此入口。
    pub fn try_insert_till(
        &mut self,
        actor: Entity,
        session: TillSession,
        plot_exists: bool,
    ) -> bool {
        if self.by_actor.contains_key(&actor) {
            tracing::debug!(
                "[bong][lingtian] try_insert_till rejected: actor={actor:?} already active"
            );
            return false;
        }
        if plot_exists {
            tracing::debug!(
                "[bong][lingtian] try_insert_till rejected: plot already exists at {:?}",
                session.pos
            );
            return false;
        }
        if self.reserved_targets.contains_key(&session.pos) {
            tracing::debug!(
                "[bong][lingtian] try_insert_till rejected: target {:?} already reserved",
                session.pos
            );
            return false;
        }

        self.reserved_targets.insert(session.pos, actor);
        self.by_actor.insert(actor, ActiveSession::Till(session));
        true
    }

    /// 清掉某 actor 的 session（cancel / 超时 / 外部取消）。
    ///
    /// fix-spec-1901-v2 §6.2 — 取消立即释放 target reservation（不等待
    /// 结算），让该 block 可被重新使用。
    pub fn clear(&mut self, actor: Entity) -> Option<ActiveSession> {
        let removed = self.by_actor.remove(&actor);
        if let Some(session) = &removed {
            if matches!(session, ActiveSession::Till(_)) {
                self.reserved_targets.remove(&session.position());
            }
        }
        removed
    }

    /// 返回所有当前已 Finished 的 (actor, session) 对，并从表中移除。
    fn drain_finished(&mut self) -> Vec<(Entity, ActiveSession)> {
        let finished_actors: Vec<Entity> = self
            .by_actor
            .iter()
            .filter(|(_, s)| s.is_finished())
            .map(|(e, _)| *e)
            .collect();
        finished_actors
            .into_iter()
            .map(|e| (e, self.by_actor.remove(&e).expect("just iterated")))
            .collect()
    }

    /// fix-spec-1901-v2 §6.2 — 结算完成后释放 Till reservation。
    ///
    /// `drain_finished` 已把完成 session 从 `by_actor` 移出；完成处理结束后，
    /// 这里是 deferred plot command 已应用的下一 reconciliation point。所有
    /// 不再属于活跃 session 的 reservation 都可释放。
    pub fn settle_reservations(&mut self) {
        self.reserved_targets
            .retain(|_, actor| self.by_actor.contains_key(actor));
    }

    fn tick_all(&mut self) {
        for s in self.by_actor.values_mut() {
            s.tick();
        }
    }
}

// ============================================================================
// 起 session
// ============================================================================

/// 单次扫描读出主手锄：返回 `(HoeKind, instance_id)`，否则 None。
///
/// 调用方法用：起 session 时验请求 `hoe_instance_id` 与主手实物匹配；
/// apply 路径同样靠它定位锄实物再扣耐久。
pub fn equipped_main_hand_hoe(inventory: &PlayerInventory) -> Option<(HoeKind, u64)> {
    // plan-layered-equip-v1 P0.2（桶①）— 锄在 main_hand held。
    let item = inventory
        .equipped
        .get(MAIN_HAND_SLOT)
        .and_then(|s| s.held.as_ref())?;
    let kind = HoeKind::from_item_id(&item.template_id)?;
    Some((kind, item.instance_id))
}

pub fn handle_start_till(
    mut events: EventReader<StartTillRequest>,
    mut sessions: ResMut<ActiveLingtianSessions>,
    inventories: Query<&PlayerInventory>,
    plots: Query<&LingtianPlot>,
    #[cfg(test)] mut plot_scan_count: Option<ResMut<StartHandlerPlotScanCount>>,
) {
    if events.is_empty() {
        return;
    }
    // fix-spec-1901-v2 #8（central review 1984-31332727941 finding [4]）— 每批
    // 请求快照一次 plot 位置索引，共用 O(1) 查找；空闲 tick 不触碰 plot query。
    #[cfg(not(test))]
    let plot_positions = build_start_plot_index(plots.iter());
    #[cfg(test)]
    let plot_positions = build_start_plot_index(plots.iter(), plot_scan_count.as_deref_mut());
    for req in events.read() {
        // fix-spec-1901-v2 §4.4 — 距离/维度 gate 已由唯一 post-transfer validator
        // 完成；本 handler 只做业务前置。直接注入本 event 的测试只能算
        // "validated event business test"，不能当作 C2S 安全测试。
        if sessions.has_session(req.player) {
            tracing::warn!(
                "[bong][lingtian] StartTillRequest rejected: player={:?} already has active session",
                req.player
            );
            continue;
        }
        let plot_exists = plot_positions.contains_key(&req.pos);
        let Ok(inv) = inventories.get(req.player) else {
            tracing::warn!(
                "[bong][lingtian] StartTillRequest rejected: player={:?} has no PlayerInventory",
                req.player
            );
            continue;
        };
        let Some((kind, instance_id)) = equipped_main_hand_hoe(inv) else {
            tracing::warn!(
                "[bong][lingtian] StartTillRequest rejected: player={:?} main hand is not a hoe",
                req.player
            );
            continue;
        };
        if instance_id != req.hoe_instance_id {
            tracing::warn!(
                "[bong][lingtian] StartTillRequest rejected: player={:?} main hand instance_id={} != requested {}",
                req.player,
                instance_id,
                req.hoe_instance_id
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
        let session = TillSession::new(req.pos, kind, instance_id, req.mode, req.environment);
        sessions.try_insert_till(req.player, session, plot_exists);
    }
}

pub fn handle_start_renew(
    mut events: EventReader<StartRenewRequest>,
    mut sessions: ResMut<ActiveLingtianSessions>,
    inventories: Query<&PlayerInventory>,
    plots: Query<&LingtianPlot>,
    #[cfg(test)] mut plot_scan_count: Option<ResMut<StartHandlerPlotScanCount>>,
) {
    if events.is_empty() {
        return;
    }
    // central review 1984-31332727941 finding [4] — 与 handle_start_till 同款：
    // 每批请求快照一次 plot 位置索引；空闲 tick 不扫描，批内不做二次方扫描。
    #[cfg(not(test))]
    let plot_positions = build_start_plot_index(plots.iter());
    #[cfg(test)]
    let plot_positions = build_start_plot_index(plots.iter(), plot_scan_count.as_deref_mut());
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
        let Some((kind, instance_id)) = equipped_main_hand_hoe(inv) else {
            tracing::warn!(
                "[bong][lingtian] StartRenewRequest rejected: player={:?} main hand is not a hoe",
                req.player
            );
            continue;
        };
        if instance_id != req.hoe_instance_id {
            tracing::warn!(
                "[bong][lingtian] StartRenewRequest rejected: player={:?} main hand instance_id={} != requested {}",
                req.player,
                instance_id,
                req.hoe_instance_id
            );
            continue;
        }
        // 必须有处于"贫瘠"状态的 plot
        let barren = plot_positions
            .get(&req.pos)
            .is_some_and(|plots| plots.iter().any(|plot| plot.is_barren()));
        if !barren {
            tracing::warn!(
                "[bong][lingtian] StartRenewRequest rejected: no barren plot at {:?}",
                req.pos
            );
            continue;
        }
        let session = RenewSession::new(req.pos, kind, instance_id);
        sessions.try_insert(req.player, ActiveSession::Renew(session));
    }
}

pub fn handle_start_planting(
    mut events: EventReader<StartPlantingRequest>,
    mut sessions: ResMut<ActiveLingtianSessions>,
    seeds: Res<SeedRegistry>,
    inventories: Query<&PlayerInventory>,
    plots: Query<&LingtianPlot>,
    #[cfg(test)] mut plot_scan_count: Option<ResMut<StartHandlerPlotScanCount>>,
) {
    if events.is_empty() {
        return;
    }
    // central review 1984-31332727941 finding [4] — 与 handle_start_till 同款：
    // 每批请求快照一次 plot 位置索引；空闲 tick 不扫描，批内不做二次方扫描。
    #[cfg(not(test))]
    let plot_positions = build_start_plot_index(plots.iter());
    #[cfg(test)]
    let plot_positions = build_start_plot_index(plots.iter(), plot_scan_count.as_deref_mut());
    for req in events.read() {
        if sessions.has_session(req.player) {
            tracing::warn!(
                "[bong][lingtian] StartPlantingRequest rejected: player={:?} already has active session",
                req.player
            );
            continue;
        }
        if seeds.seed_for_plant(&req.plant_id).is_none() {
            tracing::warn!(
                "[bong][lingtian] StartPlantingRequest rejected: unknown plant_id={}",
                req.plant_id
            );
            continue;
        }
        let Ok(inv) = inventories.get(req.player) else {
            continue;
        };
        if !player_has_seed_for(inv, &seeds, &req.plant_id) {
            tracing::warn!(
                "[bong][lingtian] StartPlantingRequest rejected: player={:?} has no seed for {}",
                req.player,
                req.plant_id
            );
            continue;
        }
        // 目标 plot 必须存在 + 空 + 未贫瘠
        let target_ok = plot_positions.get(&req.pos).is_some_and(|plots| {
            plots
                .iter()
                .any(|plot| plot.is_empty() && !plot.is_barren())
        });
        if !target_ok {
            tracing::warn!(
                "[bong][lingtian] StartPlantingRequest rejected: no empty/non-barren plot at {:?}",
                req.pos
            );
            continue;
        }
        let session = PlantingSession::new(req.pos, req.plant_id.clone());
        sessions.try_insert(req.player, ActiveSession::Planting(session));
    }
}

pub fn handle_start_drain_qi(
    mut events: EventReader<StartDrainQiRequest>,
    mut sessions: ResMut<ActiveLingtianSessions>,
    plots: Query<&LingtianPlot>,
    #[cfg(test)] mut plot_scan_count: Option<ResMut<StartHandlerPlotScanCount>>,
) {
    if events.is_empty() {
        return;
    }
    // central review 1984-31332727941 finding [4] — 与 handle_start_till 同款：
    // 每批请求快照一次 plot 位置索引；空闲 tick 不扫描，批内不做二次方扫描。
    #[cfg(not(test))]
    let plot_positions = build_start_plot_index(plots.iter());
    #[cfg(test)]
    let plot_positions = build_start_plot_index(plots.iter(), plot_scan_count.as_deref_mut());
    for req in events.read() {
        if sessions.has_session(req.player) {
            tracing::warn!(
                "[bong][lingtian] StartDrainQiRequest rejected: player={:?} already has active session",
                req.player
            );
            continue;
        }
        let exists_with_qi = plot_positions
            .get(&req.pos)
            .is_some_and(|plots| plots.iter().any(|plot| plot.plot_qi > 0.0));
        if !exists_with_qi {
            tracing::warn!(
                "[bong][lingtian] StartDrainQiRequest rejected: no plot with plot_qi at {:?}",
                req.pos
            );
            continue;
        }
        sessions.try_insert(
            req.player,
            ActiveSession::DrainQi(DrainQiSession::new(req.pos)),
        );
    }
}

pub fn handle_start_harvest(
    mut events: EventReader<StartHarvestRequest>,
    mut sessions: ResMut<ActiveLingtianSessions>,
    plots: Query<&LingtianPlot>,
    cultivations: Query<&Cultivation>,
    skill_sets: Query<&SkillSet>,
    #[cfg(test)] mut plot_scan_count: Option<ResMut<StartHandlerPlotScanCount>>,
) {
    if events.is_empty() {
        return;
    }
    // central review 1984-31332727941 finding [4] — 与 handle_start_till 同款：
    // 每批请求快照一次 plot 位置索引；空闲 tick 不扫描，批内不做二次方扫描。
    #[cfg(not(test))]
    let plot_positions = build_start_plot_index(plots.iter());
    #[cfg(test)]
    let plot_positions = build_start_plot_index(plots.iter(), plot_scan_count.as_deref_mut());
    for req in events.read() {
        if sessions.has_session(req.player) {
            tracing::warn!(
                "[bong][lingtian] StartHarvestRequest rejected: player={:?} already has active session",
                req.player
            );
            continue;
        }
        // F23 — Auto 模式（herbalism Lv.3+ 解锁）此前只在 client UI 层 gating，
        // req.mode 直接来自客户端，server 从不校验 → 可绕过协议直发 Auto 拿免手动
        // 采集。这里补服务端权威门禁：不足解锁等级则拒（仿下方"已有 session/无
        // 熟瓜"两条拒绝分支；另参 `lingtian::processing::validate_processing_start`
        // 的 SkillLocked 校验先例）。纯权限门禁，不涉 qi。
        if req.mode == SessionMode::Auto {
            let cultivation = cultivations.get(req.player).ok();
            let skill_set = skill_sets.get(req.player).ok();
            let effective_lv =
                crate::botany::harvest::herbalism_effective_lv(cultivation, skill_set);
            let auto_unlock_level =
                crate::botany::components::BotanySkillState::default().auto_unlock_level;
            if effective_lv < auto_unlock_level {
                tracing::warn!(
                    "[bong][lingtian] StartHarvestRequest(Auto) rejected: player={:?} \
                     herbalism_lv={effective_lv} < auto_unlock_level={auto_unlock_level}",
                    req.player
                );
                continue;
            }
        }
        let plant_id = plot_positions
            .get(&req.pos)
            .and_then(|plots| plots.first().copied())
            .and_then(|plot| plot.crop.as_ref())
            .filter(|c| c.is_ripe())
            .map(|c| c.kind.clone());
        let Some(plant_id) = plant_id else {
            tracing::warn!(
                "[bong][lingtian] StartHarvestRequest rejected: no ripe crop at {:?}",
                req.pos
            );
            continue;
        };
        let session = HarvestSession::new(req.pos, plant_id, req.mode);
        sessions.try_insert(req.player, ActiveSession::Harvest(session));
    }
}

#[allow(clippy::too_many_arguments)]
pub fn handle_start_replenish(
    mut events: EventReader<StartReplenishRequest>,
    mut sessions: ResMut<ActiveLingtianSessions>,
    time: LingtianTime,
    inventories: Query<&PlayerInventory>,
    plots: Query<&LingtianPlot>,
    zone_qi: Res<ZoneQiAccount>,
    #[cfg(test)] mut plot_scan_count: Option<ResMut<StartHandlerPlotScanCount>>,
) {
    if events.is_empty() {
        return;
    }
    let residue_tick = time.residue_tick();
    // central review 1984-31332727941 finding [4] — 与 handle_start_till 同款：
    // 每批请求快照一次 plot 位置索引；空闲 tick 不扫描，批内不做二次方扫描。
    #[cfg(not(test))]
    let plot_positions = build_start_plot_index(plots.iter());
    #[cfg(test)]
    let plot_positions = build_start_plot_index(plots.iter(), plot_scan_count.as_deref_mut());
    for req in events.read() {
        if sessions.has_session(req.player) {
            tracing::warn!(
                "[bong][lingtian] StartReplenishRequest rejected: player={:?} already has active session",
                req.player
            );
            continue;
        }
        let Some(plot) = plot_positions
            .get(&req.pos)
            .and_then(|plots| plots.first().copied())
        else {
            tracing::warn!(
                "[bong][lingtian] StartReplenishRequest rejected: no plot at {:?}",
                req.pos
            );
            continue;
        };
        // 冷却检查：last_replenish_at = 0 视为从未补过（允许）
        if plot.last_replenish_at != 0 {
            let elapsed = time.lingtian_tick().saturating_sub(plot.last_replenish_at);
            if elapsed < REPLENISH_COOLDOWN_LINGTIAN_TICKS {
                tracing::warn!(
                    "[bong][lingtian] StartReplenishRequest rejected: plot at {:?} on cooldown ({elapsed}/{REPLENISH_COOLDOWN_LINGTIAN_TICKS} lingtian-ticks)",
                    req.pos
                );
                continue;
            }
        }
        // 来源材料检查
        let material_ok = match req.source {
            // plan-zone-qi-economy-v1 P2：地板红线——zone 抽吸来源必须留住
            // QI_NPC_ABSORB_FLOOR 以上的底仓，不能把 zone 抽穿地板。
            ReplenishSource::Zone => {
                zone_qi.get(plot_zone_key(plot))
                    >= req.source.plot_qi_amount() + QI_NPC_ABSORB_FLOOR as f32
            }
            ReplenishSource::BoneCoin => inventories
                .get(req.player)
                .map(|inv| inv.bone_coins >= 1)
                .unwrap_or(false),
            ReplenishSource::BeastCore => inventories
                .get(req.player)
                .map(|inv| inventory_has_template(inv, BEAST_CORE_ITEM_ID))
                .unwrap_or(false),
            ReplenishSource::LingShui => inventories
                .get(req.player)
                .map(|inv| inventory_has_template(inv, LING_SHUI_ITEM_ID))
                .unwrap_or(false),
            ReplenishSource::PillResidue { residue_kind } => inventories
                .get(req.player)
                .map(|inv| inventory_has_usable_residue(inv, residue_kind, residue_tick))
                .unwrap_or(false),
        };
        if !material_ok {
            tracing::warn!(
                "[bong][lingtian] StartReplenishRequest rejected: insufficient material for source={:?}",
                req.source
            );
            continue;
        }
        let session = ReplenishSession::new(req.pos, req.source);
        sessions.try_insert(req.player, ActiveSession::Replenish(session));
    }
}

fn player_has_seed_for(inventory: &PlayerInventory, seeds: &SeedRegistry, plant_id: &str) -> bool {
    let Some(seed_id) = seeds.seed_for_plant(plant_id) else {
        return false;
    };
    inventory_has_template(inventory, seed_id)
}

fn inventory_has_template(inventory: &PlayerInventory, template_id: &str) -> bool {
    for c in &inventory.containers {
        if c.items
            .iter()
            .any(|p| p.instance.template_id == template_id && p.instance.stack_count > 0)
        {
            return true;
        }
    }
    inventory
        .hotbar
        .iter()
        .flatten()
        .any(|i| i.template_id == template_id && i.stack_count > 0)
}

/// 在 inventory 内找指定 template_id 的 item，stack -=1，归零移除。返回是否成功。
/// 风格仿 `network::cast_emit::consume_one_stack`，但按 template_id 而非 instance_id
/// （种子是 stackable，玩家关心 plant 类，不关心是哪一个 instance）。
fn consume_one_seed(inventory: &mut PlayerInventory, template_id: &str) -> bool {
    inventory.revision =
        crate::inventory::InventoryRevision(inventory.revision.0.saturating_add(1));
    for c in &mut inventory.containers {
        if let Some(idx) = c
            .items
            .iter()
            .position(|p| p.instance.template_id == template_id && p.instance.stack_count > 0)
        {
            let placed = &mut c.items[idx];
            if placed.instance.stack_count > 1 {
                placed.instance.stack_count -= 1;
            } else {
                c.items.remove(idx);
            }
            return true;
        }
    }
    for slot in inventory.hotbar.iter_mut() {
        if let Some(item) = slot.as_mut() {
            if item.template_id == template_id && item.stack_count > 0 {
                if item.stack_count > 1 {
                    item.stack_count -= 1;
                } else {
                    *slot = None;
                }
                return true;
            }
        }
    }
    false
}

// ============================================================================
// tick + 结算
// ============================================================================

pub fn tick_lingtian_sessions(mut sessions: ResMut<ActiveLingtianSessions>) {
    sessions.tick_all();
}

#[allow(clippy::too_many_arguments)]
pub fn apply_completed_sessions(
    mut commands: Commands,
    mut sessions: ResMut<ActiveLingtianSessions>,
    mut inventories: Query<&mut PlayerInventory>,
    mut plots: Query<(Entity, &mut LingtianPlot)>,
    mut life_records: Query<&mut LifeRecord>,
    mut cultivations: Query<&mut Cultivation>,
    seeds: Res<SeedRegistry>,
    plant_registry: Res<PlantKindRegistry>,
    item_registry: Res<ItemRegistry>,
    mut allocator: ResMut<InventoryInstanceIdAllocator>,
    mut harvest_rng: ResMut<LingtianHarvestRng>,
    mut zone_qi: ResMut<ZoneQiAccount>,
    mut writers: CompletionEventWriters,
    mut layers: Query<&mut ChunkLayer, With<crate::world::dimension::OverworldLayer>>,
    mut skill_xp_events: Option<ResMut<Events<SkillXpGain>>>,
    context: CompletionContext,
) {
    let drained = sessions.drain_finished();
    if drained.is_empty() {
        return;
    }
    let needs_existing_plot_snapshot = drained
        .iter()
        .any(|(_, session)| matches!(session, ActiveSession::Till(_)));
    let existing_plot_positions: HashSet<_> = if needs_existing_plot_snapshot {
        plots.iter().map(|(_, plot)| plot.pos).collect()
    } else {
        HashSet::new()
    };
    let mut completion_till_positions = HashSet::new();
    for (player, finished) in drained {
        let target = finished.position();
        if context.actor_queries.clients.get(player).is_ok() {
            if let Err(reason) = validate_lingtian_interaction(
                player,
                target,
                &context.actor_queries.positions,
                &context.actor_queries.dimensions,
            ) {
                log_lingtian_interaction_denial("completion", player, target, reason);
                continue;
            }
        } else if context.actor_queries.npcs.get(player).is_err() {
            tracing::debug!(
                "[bong][lingtian] completed session discarded for unknown actor={:?}",
                player,
            );
            continue;
        }

        match finished {
            ActiveSession::Till(s) => {
                if existing_plot_positions.contains(&s.pos)
                    || !completion_till_positions.insert(s.pos)
                {
                    tracing::warn!(
                        "[bong][lingtian] duplicate Till completion suppressed at {:?}",
                        s.pos
                    );
                    continue;
                }
                if let Ok(mut inv) = inventories.get_mut(player) {
                    wear_main_hand_hoe(&mut inv, s.hoe, s.hoe_instance_id);
                }
                let mut plot = LingtianPlot::new(s.pos, Some(player));
                plot.plot_qi_cap = compute_plot_qi_cap(&s.environment);
                commands.spawn(plot);
                // plan §1.2.2 步骤 3 — 放一块 Farmland 让玩家视觉上看到 plot。
                if let Ok(mut layer) = layers.get_single_mut() {
                    layer.set_block(s.pos, BlockState::FARMLAND);
                }
                writers.till.send(TillCompleted {
                    player,
                    pos: s.pos,
                    hoe: s.hoe,
                    hoe_instance_id: s.hoe_instance_id,
                });
                emit_lingtian_vfx(
                    writers.vfx_events.as_deref_mut(),
                    gameplay_vfx::LINGTIAN_TILL,
                    s.pos,
                    "#8B5A2B",
                    0.6,
                    8,
                    24,
                );
                emit_lingtian_skill_xp(&mut skill_xp_events, player, 1, "till");
            }
            ActiveSession::Renew(s) => {
                if let Ok(mut inv) = inventories.get_mut(player) {
                    wear_main_hand_hoe(&mut inv, s.hoe, s.hoe_instance_id);
                }
                if let Some((_e, mut plot)) = plots.iter_mut().find(|(_, p)| p.pos == s.pos) {
                    plot.renew();
                    // 翻新后从"贫瘠"（CoarseDirt）回到 Farmland 可耕状态。
                    if let Ok(mut layer) = layers.get_single_mut() {
                        layer.set_block(s.pos, BlockState::FARMLAND);
                    }
                    writers.renew.send(RenewCompleted {
                        player,
                        pos: s.pos,
                        hoe: s.hoe,
                        hoe_instance_id: s.hoe_instance_id,
                    });
                    emit_lingtian_vfx(
                        writers.vfx_events.as_deref_mut(),
                        gameplay_vfx::LINGTIAN_TILL,
                        s.pos,
                        "#8B5A2B",
                        0.7,
                        8,
                        24,
                    );
                    emit_lingtian_skill_xp(&mut skill_xp_events, player, 2, "renew");
                } else {
                    tracing::warn!(
                        "[bong][lingtian] RenewSession finished but plot at {:?} vanished",
                        s.pos
                    );
                }
            }
            ActiveSession::Planting(s) => {
                let planted = apply_planting_completion(
                    player,
                    &s.pos,
                    &s.plant_id,
                    &mut inventories,
                    &mut plots,
                    &seeds,
                    &mut writers.planting,
                    &mut skill_xp_events,
                );
                if planted {
                    emit_lingtian_vfx(
                        writers.vfx_events.as_deref_mut(),
                        gameplay_vfx::LINGTIAN_PLANT,
                        s.pos,
                        "#44AA44",
                        0.75,
                        6,
                        30,
                    );
                }
            }
            ActiveSession::Harvest(s) => {
                apply_harvest_completion(
                    player,
                    &s.pos,
                    &s.plant_id,
                    &mut inventories,
                    &mut plots,
                    &mut life_records,
                    &plant_registry,
                    &item_registry,
                    &mut allocator,
                    &mut harvest_rng,
                    context.time.lingtian_tick(),
                    &mut writers.harvest,
                    &mut skill_xp_events,
                    s.mode,
                );
                // plan §1.6 — 收获若使 plot 贫瘠，外观改 CoarseDirt 以示灰化。
                if plots.iter().any(|(_, p)| p.pos == s.pos && p.is_barren()) {
                    if let Ok(mut layer) = layers.get_single_mut() {
                        layer.set_block(s.pos, BlockState::COARSE_DIRT);
                    }
                }
            }
            ActiveSession::Replenish(s) => {
                let residue_tick = context.time.residue_tick();
                let replenished = apply_replenish_completion(
                    player,
                    &s.pos,
                    s.source,
                    &mut inventories,
                    &mut plots,
                    &mut zone_qi,
                    context.time.lingtian_tick(),
                    residue_tick,
                    &mut harvest_rng,
                    &mut writers.replenish,
                    &mut writers.dye_warning,
                    &mut skill_xp_events,
                );
                if replenished {
                    emit_lingtian_vfx(
                        writers.vfx_events.as_deref_mut(),
                        gameplay_vfx::LINGTIAN_REPLENISH,
                        s.pos,
                        "#66FFCC",
                        0.8,
                        8,
                        30,
                    );
                }
            }
            ActiveSession::DrainQi(s) => {
                apply_drain_qi_completion(
                    player,
                    &s.pos,
                    &mut plots,
                    &mut cultivations,
                    &mut life_records,
                    &mut zone_qi,
                    context.time.lingtian_tick(),
                    &mut writers.drain_qi,
                    &mut writers.qi_transfer,
                );
            }
        }
    }
}

fn emit_lingtian_vfx(
    events: Option<&mut Events<VfxEventRequest>>,
    event_id: &'static str,
    pos: valence::prelude::BlockPos,
    color: &'static str,
    strength: f32,
    count: u32,
    duration_ticks: u32,
) {
    let Some(events) = events else {
        return;
    };
    let origin = gameplay_vfx::block_center([pos.x, pos.y, pos.z]);
    gameplay_vfx::send_spawn(
        events,
        gameplay_vfx::spawn_request(
            event_id,
            origin,
            Some([0.0, 0.8, 0.0]),
            color,
            strength,
            count,
            duration_ticks,
        ),
    );
}

pub fn emit_harvest_inventory_snapshots(
    mut events: EventReader<HarvestCompleted>,
    inventories: Query<&PlayerInventory>,
    player_states: Query<&PlayerState>,
    cultivations: Query<&Cultivation>,
    mut clients: Query<(&Username, &mut Client)>,
) {
    for event in events.read() {
        let Ok(inventory) = inventories.get(event.player) else {
            continue;
        };
        let Ok(player_state) = player_states.get(event.player) else {
            continue;
        };
        let Ok(cultivation) = cultivations.get(event.player) else {
            continue;
        };
        let Ok((username, mut client)) = clients.get_mut(event.player) else {
            continue;
        };

        send_inventory_snapshot_to_client(
            event.player,
            &mut client,
            username.0.as_str(),
            inventory,
            player_state,
            cultivation,
            "lingtian_harvest",
        );
    }
}

pub fn release_lingtian_plot_owner_on_npc_death(
    mut settlements: EventReader<NpcTerminalSettlementSucceeded>,
    mut plots: Query<&mut LingtianPlot>,
) {
    for settlement in settlements.read() {
        for mut plot in &mut plots {
            if plot.owner == Some(settlement.entity) {
                plot.owner = None;
            }
        }
    }
}

#[derive(Debug, Default, Resource)]
pub struct PendingPlotZones {
    entities: HashSet<Entity>,
    /// fix-spec-1901-v2 §7.1 — 上次观察到的 `ZoneRegistry::spatial_revision`。
    /// `None` = 注册表从未被观察到（此前不在场）——任何新插入（哪怕 revision 0
    /// 基线）都必须重试 unresolved set；`Some(seen)` 仅在 revision 真正变化
    /// （zone membership / bounds 变化）时重试。区分"注册表刚插入（revision 0）"
    /// 与"revision 0 已观察过"，避免新插入的注册表因默认同为 0 而永不重试
    /// pending plot；heartbeat qi 的每 tick mutable borrow 仍不触发全量扫描。
    last_seen_spatial_revision: Option<u64>,
}

/// Resolve newly-added plots once and retry unresolved plots only when the zone
/// registry's spatial revision changes.
#[allow(clippy::type_complexity)]
pub fn auto_set_plot_zone(
    mut plot_queries: ParamSet<(Query<Entity, Added<LingtianPlot>>, Query<&mut LingtianPlot>)>,
    zone_registry: Option<Res<ZoneRegistry>>,
    mut pending: ResMut<PendingPlotZones>,
) {
    let new_entities: Vec<Entity> = plot_queries.p0().iter().collect();
    let Some(zr) = zone_registry.as_deref() else {
        pending.last_seen_spatial_revision = None;
        pending.entities.extend(new_entities);
        return;
    };

    // `None`（注册表此前不在场）视为必然变化：新插入的 revision-zero 注册表
    // 必须触发一次 retry，否则默认同为 0 会让 pending plot 永远得不到 zone。
    let revision_changed = match pending.last_seen_spatial_revision {
        Some(seen) => zr.spatial_revision != seen,
        None => true,
    };
    let candidates = if revision_changed {
        pending.last_seen_spatial_revision = Some(zr.spatial_revision);
        pending.entities.extend(new_entities);
        pending.entities.drain().collect::<Vec<_>>()
    } else {
        new_entities
    };

    let mut plots = plot_queries.p1();
    for entity in candidates {
        let Ok(mut plot) = plots.get_mut(entity) else {
            continue;
        };
        if !plot.zone.is_empty() {
            continue;
        }
        // fix-spec-1901-v2 §7.2 — zone backfill 与 collapse lookup 共用方块坐标合同：
        // 水平取中心，Y 取方块整数底面，确保 inclusive upper-Y boundary 身份一致。
        let pos = plot_zone_center(&plot);
        let Some(zone) = zr.find_zone(crate::world::dimension::DimensionKind::Overworld, pos)
        else {
            pending.entities.insert(entity);
            continue;
        };
        plot.zone = zone.name.clone();
    }
}

/// fix-spec-1901-v2 §6.2 — 每 tick 结算 Till target reservation。
///
/// 在 `apply_completed_sessions` 之后运行（deferred `commands.spawn(plot)`
/// 已应用）：已落地 plot 的 reservation 释放，被 gate 拒绝 / spawn 跳过的
/// 悬空 reservation 也一并清理。
pub fn settle_lingtian_plot_reservations(mut sessions: ResMut<ActiveLingtianSessions>) {
    sessions.settle_reservations();
}

#[allow(clippy::too_many_arguments)]
fn apply_planting_completion(
    actor: Entity,
    pos: &valence::prelude::BlockPos,
    plant_id: &PlantId,
    inventories: &mut Query<&mut PlayerInventory>,
    plots: &mut Query<(Entity, &mut LingtianPlot)>,
    seeds: &SeedRegistry,
    planting_completed: &mut EventWriter<PlantingCompleted>,
    skill_xp_events: &mut Option<ResMut<Events<SkillXpGain>>>,
) -> bool {
    let Some(seed_id) = seeds.seed_for_plant(plant_id).cloned() else {
        tracing::warn!(
            "[bong][lingtian] PlantingSession finished but plant_id={} no longer in SeedRegistry",
            plant_id
        );
        return false;
    };
    // 玩家复验种子仍在；NPC 散修没有 PlayerInventory，按自带低阶种子处理。
    let mut inventory = inventories.get_mut(actor).ok();
    let Some((_e, mut plot)) = plots
        .iter_mut()
        .find(|(_, p)| &p.pos == pos && p.is_empty() && !p.is_barren())
    else {
        tracing::warn!(
            "[bong][lingtian] PlantingSession finished but target plot at {pos:?} no longer plantable"
        );
        return false;
    };
    if let Some(inv) = inventory.as_deref_mut() {
        if !consume_one_seed(inv, &seed_id) {
            tracing::warn!(
                "[bong][lingtian] PlantingSession finished but seed `{seed_id}` no longer in inventory"
            );
            return false;
        }
    } else {
        tracing::debug!(
            "[bong][lingtian] PlantingSession actor={actor:?} has no PlayerInventory; treating as NPC self-supplied seed"
        );
    }
    plot.crop = Some(CropInstance::new(plant_id.clone()));
    planting_completed.send(PlantingCompleted {
        player: actor,
        pos: *pos,
        plant_id: plant_id.clone(),
    });
    emit_lingtian_skill_xp(skill_xp_events, actor, 1, "plant");
    true
}

#[allow(clippy::too_many_arguments)]
fn apply_harvest_completion(
    actor: Entity,
    pos: &valence::prelude::BlockPos,
    plant_id: &PlantId,
    inventories: &mut Query<&mut PlayerInventory>,
    plots: &mut Query<(Entity, &mut LingtianPlot)>,
    life_records: &mut Query<&mut LifeRecord>,
    plant_registry: &PlantKindRegistry,
    item_registry: &ItemRegistry,
    allocator: &mut InventoryInstanceIdAllocator,
    rng: &mut LingtianHarvestRng,
    now_lingtian_tick: u64,
    harvest_completed: &mut EventWriter<HarvestCompleted>,
    skill_xp_events: &mut Option<ResMut<Events<SkillXpGain>>>,
    mode: super::session::SessionMode,
) {
    let Some(kind) = plant_registry.get(plant_id) else {
        tracing::warn!(
            "[bong][lingtian] HarvestSession finished but plant_id={plant_id} no longer in registry"
        );
        return;
    };
    let mut inventory = inventories.get_mut(actor).ok();

    // 锁定 owner 在借用 plot 的局部作用域里读出
    let plot_owner = {
        let Some((_e, mut plot)) = plots
            .iter_mut()
            .find(|(_, p)| &p.pos == pos && p.crop.as_ref().map(|c| c.is_ripe()).unwrap_or(false))
        else {
            tracing::warn!(
                "[bong][lingtian] HarvestSession finished but plot at {pos:?} no longer ripe"
            );
            return;
        };
        let owner = plot.owner;

        // 1. 给作物 item（plant_id 同名）
        if item_registry.get(plant_id).is_none() {
            tracing::warn!(
                "[bong][lingtian] no ItemTemplate for plant_id={plant_id} (need entry in herbs.toml)"
            );
            return;
        }
        if let Some(inv) = inventory.as_deref_mut() {
            if let Err(error) = add_item_to_player_inventory(
                inv,
                item_registry,
                allocator,
                plant_id,
                1,
                now_lingtian_tick,
            ) {
                tracing::warn!(
                    "[bong][lingtian] harvest award failed; dropped 1× {plant_id} for actor={actor:?}: {error}"
                );
            }
        } else {
            tracing::debug!(
                "[bong][lingtian] HarvestSession actor={actor:?} has no PlayerInventory; NPC consumes harvest offscreen"
            );
        }

        // 2. 按 PlantRarity::seed_drop_rate 概率发种子
        let drop_rate = kind.rarity.seed_drop_rate();
        let roll = rng.next_f32();
        let seed_dropped = if roll < drop_rate {
            let seed_id = seed_id_for(plant_id);
            if let Some(inv) = inventory.as_deref_mut() {
                if item_registry.get(&seed_id).is_none() {
                    tracing::warn!(
                        "[bong][lingtian] no ItemTemplate for seed `{seed_id}` (need entry in seeds.toml)"
                    );
                    false
                } else {
                    if let Err(error) = add_item_to_player_inventory(
                        inv,
                        item_registry,
                        allocator,
                        &seed_id,
                        1,
                        now_lingtian_tick,
                    ) {
                        tracing::warn!(
                            "[bong][lingtian] harvest seed award failed; dropped 1× {seed_id} for actor={actor:?}: {error}"
                        );
                    }
                    true
                }
            } else {
                tracing::debug!(
                    "[bong][lingtian] HarvestSession actor={actor:?} has no PlayerInventory; seed drop is consumed offscreen"
                );
                false
            }
        } else {
            false
        };

        // 3. plot 转为空田 + harvest_count++
        plot.crop = None;
        plot.harvest_count = plot.harvest_count.saturating_add(1);

        harvest_completed.send(HarvestCompleted {
            player: actor,
            pos: *pos,
            plant_id: plant_id.clone(),
            seed_dropped,
        });
        let (amount, action) = match mode {
            super::session::SessionMode::Manual => (2, "harvest_manual"),
            super::session::SessionMode::Auto => (5, "harvest_auto"),
        };
        emit_lingtian_skill_xp(skill_xp_events, actor, amount, action);

        owner
    };

    // 4. 偷菜匿名记账（plan §1.7）：owner != actor 时双方各记一条
    if let Some(owner) = plot_owner {
        if owner != actor {
            let pos_arr = [pos.x, pos.y, pos.z];
            if let Ok(mut owner_lr) = life_records.get_mut(owner) {
                owner_lr.push(BiographyEntry::PlotHarvestedByOther {
                    plot_pos: pos_arr,
                    plant_id: plant_id.clone(),
                    tick: now_lingtian_tick,
                });
            }
            if let Ok(mut actor_lr) = life_records.get_mut(actor) {
                actor_lr.push(BiographyEntry::PlotHarvestedFromOther {
                    plot_pos: pos_arr,
                    plant_id: plant_id.clone(),
                    tick: now_lingtian_tick,
                });
            }
        }
    }
}

fn bump_revision(inv: &mut PlayerInventory) {
    inv.revision = crate::inventory::InventoryRevision(inv.revision.0.saturating_add(1));
}

#[allow(clippy::too_many_arguments)]
fn apply_drain_qi_completion(
    player: Entity,
    pos: &valence::prelude::BlockPos,
    plots: &mut Query<(Entity, &mut LingtianPlot)>,
    cultivations: &mut Query<&mut Cultivation>,
    life_records: &mut Query<&mut LifeRecord>,
    zone_qi: &mut ZoneQiAccount,
    now_lingtian_tick: u64,
    drain_completed: &mut EventWriter<DrainQiCompleted>,
    qi_transfers: &mut EventWriter<QiTransfer>,
) {
    let (plot_owner, drained, to_player, to_zone, zone_key) = {
        let Some((_e, mut plot)) = plots.iter_mut().find(|(_, p)| &p.pos == pos) else {
            tracing::warn!("[bong][lingtian] DrainQiSession finished but plot at {pos:?} vanished");
            return;
        };
        let zone_key = plot_zone_key(&plot).to_string();
        let drained = plot.plot_qi;
        if drained <= 0.0 {
            tracing::warn!(
                "[bong][lingtian] DrainQiSession finished but plot at {pos:?} now empty"
            );
            return;
        }
        let owner = plot.owner;
        plot.plot_qi = 0.0;
        let to_player = drained * DRAIN_QI_TO_PLAYER_RATIO;
        let to_zone = drained * DRAIN_QI_TO_ZONE_RATIO;
        (owner, drained, to_player, to_zone, zone_key)
    };

    let player_account = qi_player_account_id(player, life_records);
    // 注入操作者 cultivation.qi_current（cap at qi_max）；未入账份额回流 zone。
    let actual_to_player = if let Ok(mut cult) = cultivations.get_mut(player) {
        let room = (cult.qi_max - cult.qi_current).max(0.0);
        let credited = (to_player as f64).min(room);
        cult.qi_current += credited;
        credited as f32
    } else {
        0.0
    };
    let actual_to_zone = to_zone + (to_player - actual_to_player).max(0.0);

    // 散逸 zone qi
    *zone_qi.get_mut(&zone_key) += actual_to_zone;
    emit_drain_qi_transfers(
        player_account,
        pos,
        &zone_key,
        actual_to_player,
        actual_to_zone,
        qi_transfers,
    );

    // 双方 LifeRecord 记账（仅 owner != player）
    if let Some(owner) = plot_owner {
        if owner != player {
            let pos_arr = [pos.x, pos.y, pos.z];
            if let Ok(mut owner_lr) = life_records.get_mut(owner) {
                owner_lr.push(BiographyEntry::PlotQiDrainedByOther {
                    plot_pos: pos_arr,
                    amount_drained: drained,
                    tick: now_lingtian_tick,
                });
            }
            if let Ok(mut player_lr) = life_records.get_mut(player) {
                player_lr.push(BiographyEntry::PlotQiDrainedFromOther {
                    plot_pos: pos_arr,
                    amount_drained: drained,
                    tick: now_lingtian_tick,
                });
            }
        }
    }

    drain_completed.send(DrainQiCompleted {
        player,
        pos: *pos,
        plot_qi_drained: drained,
        qi_to_player: actual_to_player,
        qi_to_zone: actual_to_zone,
    });
}

fn emit_drain_qi_transfers(
    player_account: Option<QiAccountId>,
    pos: &valence::prelude::BlockPos,
    zone: &str,
    to_player: f32,
    to_zone: f32,
    qi_transfers: &mut EventWriter<QiTransfer>,
) {
    let plot_account =
        QiAccountId::container(format!("lingtian_plot:{},{},{}", pos.x, pos.y, pos.z));
    if to_player > 0.0 {
        if let Some(player_account) = player_account {
            send_qi_transfer(
                qi_transfers,
                plot_account.clone(),
                player_account,
                to_player as f64,
                QiTransferReason::Channeling,
            );
        } else {
            tracing::warn!(
                "[bong][lingtian] skip player qi transfer without stable account at {pos:?}"
            );
        }
    }
    if to_zone > 0.0 {
        send_qi_transfer(
            qi_transfers,
            plot_account,
            QiAccountId::zone(zone),
            to_zone as f64,
            QiTransferReason::ReleaseToZone,
        );
    }
}

fn qi_player_account_id(
    player: Entity,
    life_records: &Query<&mut LifeRecord>,
) -> Option<QiAccountId> {
    if let Ok(life_record) = life_records.get(player) {
        if !life_record.character_id.trim().is_empty() {
            return Some(QiAccountId::player(life_record.character_id.clone()));
        }
    }
    tracing::warn!("[bong][lingtian] DrainQiSession has no stable ledger account for {player:?}");
    None
}

fn send_qi_transfer(
    qi_transfers: &mut EventWriter<QiTransfer>,
    from: QiAccountId,
    to: QiAccountId,
    amount: f64,
    reason: QiTransferReason,
) {
    match QiTransfer::new(from, to, amount, reason) {
        Ok(transfer) => {
            qi_transfers.send(transfer);
        }
        Err(error) => {
            tracing::warn!(?error, "[bong][lingtian] drop invalid qi transfer");
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn apply_replenish_completion(
    player: Entity,
    pos: &valence::prelude::BlockPos,
    source: ReplenishSource,
    inventories: &mut Query<&mut PlayerInventory>,
    plots: &mut Query<(Entity, &mut LingtianPlot)>,
    zone_qi: &mut ZoneQiAccount,
    now_lingtian_tick: u64,
    residue_now_tick: u64,
    rng: &mut LingtianHarvestRng,
    replenish_completed: &mut EventWriter<ReplenishCompleted>,
    dye_warning_events: &mut EventWriter<DyeContaminationWarning>,
    skill_xp_events: &mut Option<ResMut<Events<SkillXpGain>>>,
) -> bool {
    let Some((_e, mut plot)) = plots.iter_mut().find(|(_, p)| &p.pos == pos) else {
        tracing::warn!("[bong][lingtian] ReplenishSession finished but plot at {pos:?} vanished");
        return false;
    };
    let zone_key = plot_zone_key(&plot).to_string();

    // 复验 / 扣材料：plan §1.4 来源材料**不退**，若 session 期间被消耗也照付
    let amount = source.plot_qi_amount();
    let mut paid = true;
    match source {
        ReplenishSource::Zone => {
            // plan-zone-qi-economy-v1 P2：复验时同样要求地板以上余量覆盖 amount，
            // 防止 session 期间 zone 被其它路径抽到贴近地板后仍照付出穿地板。
            let z = zone_qi.get_mut(&zone_key);
            if *z - amount >= QI_NPC_ABSORB_FLOOR as f32 {
                *z -= amount;
            } else {
                paid = false;
            }
        }
        ReplenishSource::BoneCoin => {
            if let Ok(mut inv) = inventories.get_mut(player) {
                if inv.bone_coins >= 1 {
                    inv.bone_coins -= 1;
                    bump_revision(&mut inv);
                } else {
                    paid = false;
                }
            } else {
                paid = false;
            }
        }
        ReplenishSource::BeastCore => {
            if let Ok(mut inv) = inventories.get_mut(player) {
                if !consume_one_seed(&mut inv, BEAST_CORE_ITEM_ID) {
                    paid = false;
                }
            } else {
                paid = false;
            }
        }
        ReplenishSource::LingShui => {
            if let Ok(mut inv) = inventories.get_mut(player) {
                if !consume_one_seed(&mut inv, LING_SHUI_ITEM_ID) {
                    paid = false;
                }
            } else {
                paid = false;
            }
        }
        ReplenishSource::PillResidue { residue_kind } => {
            if let Ok(mut inv) = inventories.get_mut(player) {
                if !consume_one_residue(&mut inv, residue_kind, residue_now_tick) {
                    paid = false;
                }
            } else {
                paid = false;
            }
        }
    }

    if !paid {
        tracing::warn!(
            "[bong][lingtian] ReplenishSession finished but material vanished mid-session (source={source:?}); aborted"
        );
        return false;
    }

    // 注入 plot_qi，溢出回馈 zone（plan §1.4）
    let cap_room = (plot.plot_qi_cap - plot.plot_qi).max(0.0);
    let added = amount.min(cap_room);
    let overflow = amount - added;
    plot.plot_qi += added;
    if overflow > 0.0 {
        // 溢出回馈：Zone source 自身的 overflow 也回馈（plan 没明说 zone 来源
        // 是否例外，本切片按"统一回馈环境"处理）
        let z = zone_qi.get_mut(&zone_key);
        *z += overflow;
    }
    let had_dye_warning = plot.has_dye_contamination_warning();
    let contamination_added =
        apply_dye_contamination_on_replenish(&mut plot, source, rng.next_f32());
    if contamination_added > 0.0 {
        tracing::info!(
            "[bong][lingtian] residue replenish added dye_contamination={contamination_added:.3} source={source:?} at {pos:?}"
        );
    }
    if !had_dye_warning && plot.has_dye_contamination_warning() {
        dye_warning_events.send(DyeContaminationWarning {
            player,
            pos: *pos,
            source,
            dye_contamination: plot.dye_contamination,
            added: contamination_added,
        });
    }
    plot.last_replenish_at = now_lingtian_tick.max(1);

    replenish_completed.send(ReplenishCompleted {
        player,
        pos: *pos,
        source,
        plot_qi_added: added,
        overflow_to_zone: overflow,
    });
    emit_lingtian_skill_xp(skill_xp_events, player, 1, "replenish");
    true
}

fn residue_now_tick(combat_clock: Option<&CombatClock>, lingtian_clock: &LingtianClock) -> u64 {
    if let Some(clock) = combat_clock {
        return clock.tick;
    }
    lingtian_clock
        .lingtian_tick
        .saturating_mul(u64::from(BEVY_TICKS_PER_LINGTIAN_TICK))
}

pub fn record_dye_contamination_warning_recent_events(
    mut events: EventReader<DyeContaminationWarning>,
    mut active_events: Option<ResMut<ActiveEventsResource>>,
    clock: Res<CombatClock>,
    usernames: Query<&Username>,
    plots: Query<&LingtianPlot>,
) {
    let Some(active_events) = active_events.as_deref_mut() else {
        for _ in events.read() {}
        return;
    };

    for event in events.read() {
        let zone = plot_zone_key_at(&plots, &event.pos);
        let mut details = HashMap::new();
        details.insert(
            "pos".to_string(),
            serde_json::json!([event.pos.x, event.pos.y, event.pos.z]),
        );
        details.insert(
            "source".to_string(),
            serde_json::json!(replenish_source_wire(event.source)),
        );
        details.insert(
            "dye_contamination".to_string(),
            serde_json::json!(event.dye_contamination),
        );
        details.insert("added".to_string(), serde_json::json!(event.added));

        active_events.record_recent_event(GameEvent {
            event_type: GameEventType::EventTriggered,
            tick: clock.tick,
            player: usernames
                .get(event.player)
                .ok()
                .map(|username| canonical_player_id(username.0.as_str())),
            target: Some("lingtian_plot_dye_contamination_warning".to_string()),
            zone: Some(zone),
            details: Some(details),
        });
    }
}

fn emit_lingtian_skill_xp(
    skill_xp_events: &mut Option<ResMut<Events<SkillXpGain>>>,
    player: Entity,
    amount: u32,
    action: &'static str,
) {
    if let Some(skill_xp_events) = skill_xp_events.as_deref_mut() {
        skill_xp_events.send(SkillXpGain {
            char_entity: player,
            skill: SkillId::Herbalism,
            amount,
            source: XpGainSource::Action {
                plan_id: "lingtian",
                action,
            },
        });
    }
}

/// plan §1.2.1 / §1.6 — 主手锄扣 1 次耐久。归一化 [0, 1]。归零移除装备。
///
/// `expected_instance_id` 锁定 session 起手时的具体锄实物：若玩家在 session
/// 期间换了把锄（甚至同档不同实物），不应错扣给替换上去的那把。
fn wear_main_hand_hoe(
    inventory: &mut PlayerInventory,
    expected: HoeKind,
    expected_instance_id: u64,
) {
    let cost = expected.use_durability_cost();
    // plan-layered-equip-v1 P0.2（桶①）— 锄在 main_hand held。
    let Some(item) = inventory
        .equipped
        .get_mut(MAIN_HAND_SLOT)
        .and_then(|s| s.held.as_mut())
    else {
        return;
    };
    if item.instance_id != expected_instance_id {
        tracing::warn!(
            "[bong][lingtian] wear_main_hand_hoe: main hand instance changed during session (expected={}, found={})",
            expected_instance_id,
            item.instance_id
        );
        return;
    }
    if HoeKind::from_item_id(&item.template_id) != Some(expected) {
        return;
    }
    item.durability = (item.durability - cost).max(0.0);
    if item.durability <= 0.0 {
        // 耐久归零：清 held；若该槽 SlotContents 随之全空则移除空槽（保持 contains_key 反映槽空）。
        if let Some(contents) = inventory.equipped.get_mut(MAIN_HAND_SLOT) {
            contents.held = None;
            if contents.is_empty() {
                inventory.equipped.remove(MAIN_HAND_SLOT);
            }
        }
    }
}

/// 取消某 actor 的 session（外部如 quit / 离线 / 主动取消调用）。
#[allow(dead_code)]
pub fn cancel_actor_session(
    sessions: &mut ActiveLingtianSessions,
    actor: Entity,
) -> Option<ActiveSession> {
    sessions.clear(actor)
}

// ============================================================================
// 生长 tick（plan §1.3 / §4 LingtianTick）
// ============================================================================

/// 每 Bevy tick 累一次；满 1200 触发一 lingtian-tick：迭代所有 plot，按
/// `botany::PlantKindRegistry` 查 PlantKind，调 `advance_one_lingtian_tick`
/// 推进 growth + plot_qi + zone qi。
///
/// zone 解析以 `plot.zone` 为准，空 zone 回退 `DEFAULT_ZONE`；若当前 world zone 已域崩，
/// 仅阻断灵田自身灵气功能，不移除 plot 实体。
pub fn lingtian_growth_tick(
    mut accumulator: ResMut<LingtianTickAccumulator>,
    mut clock: ResMut<LingtianClock>,
    mut zone_qi: ResMut<ZoneQiAccount>,
    registry: Res<PlantKindRegistry>,
    mut plots: Query<&mut LingtianPlot>,
    zone_registry: Option<Res<ZoneRegistry>>,
    mut layers: Query<&mut ChunkLayer, With<crate::world::dimension::OverworldLayer>>,
) {
    if !accumulator.step() {
        return;
    }
    clock.lingtian_tick = clock.lingtian_tick.saturating_add(1);
    let zone_registry = zone_registry.as_deref();
    for mut plot in plots.iter_mut() {
        dye_contamination_decay_tick(&mut plot);
        advance_plot_one_lingtian_tick_in_zone(&mut plot, &registry, &mut zone_qi, zone_registry);
    }
    // plan §1.5 — 作物成熟在 plot 顶部放 HayBlock 作"熟"标记，空 / 未熟时 Air。
    if let Ok(mut layer) = layers.get_single_mut() {
        for plot in plots.iter() {
            let top = valence::prelude::BlockPos::new(plot.pos.x, plot.pos.y + 1, plot.pos.z);
            let ripe = plot.crop.as_ref().map(|c| c.is_ripe()).unwrap_or(false);
            let desired = if ripe {
                BlockState::HAY_BLOCK
            } else {
                BlockState::AIR
            };
            if layer.block(top).map(|b| b.state) != Some(desired) {
                layer.set_block(top, desired);
            }
        }
    }
}

/// plan §5.1 — 收到 `ReplenishCompleted` 就把 `plot_qi_added + overflow_to_zone`
/// 记到 `ZonePressureTracker`（因为代价已付，全量计入"补灵贡献"）。
pub fn record_replenish_to_pressure(
    mut events: EventReader<ReplenishCompleted>,
    clock: Res<LingtianClock>,
    mut tracker: ResMut<ZonePressureTracker>,
    plots: Query<&LingtianPlot>,
) {
    for e in events.read() {
        let zone = plot_zone_key_at(&plots, &e.pos);
        let total = e.plot_qi_added + e.overflow_to_zone;
        tracker
            .state_mut(&zone)
            .record_replenish(clock.lingtian_tick, total);
    }
}

/// plan §5.1 + plan-lingtian-weather-v1 §2 / §3 — 每 lingtian-tick 后（通过
/// 读 `LingtianTickAccumulator` 刚归零）重算 zone pressure、prune 7d 窗口、
/// 跨档上升时发 `ZonePressureCrossed` 事件；HIGH 进入时清 zone 所有 plot_qi
/// （道伥 spawn 由下游 npc 系统接）。
///
/// 季节修饰从 `WorldSeasonState.current.season` 取（jiezeq-v1 全服同步）；
/// 天气事件从 `ActiveWeather` Resource 取（P2 落地 weather_generator_system
/// 后自动填）。
#[allow(clippy::too_many_arguments)]
pub fn compute_zone_pressure_system(
    accumulator: Res<LingtianTickAccumulator>,
    clock: Res<LingtianClock>,
    mut tracker: ResMut<ZonePressureTracker>,
    registry: Res<PlantKindRegistry>,
    season_state: Option<Res<crate::world::season::WorldSeasonState>>,
    active_weather: Option<Res<crate::lingtian::weather::ActiveWeather>>,
    mut plots: Query<&mut LingtianPlot>,
    mut events: EventWriter<ZonePressureCrossed>,
) {
    // 与 lingtian_growth_tick 同节拍：accumulator 刚在同一 Update 归零
    // → 本 tick 刚跑过一 lingtian-tick，现在是对齐点。
    if accumulator.raw() != 0 {
        return;
    }
    let now = clock.lingtian_tick;

    let season = season_state
        .as_deref()
        .map(|s| s.current.season)
        .unwrap_or_default();

    let mut zones: Vec<String> = plots
        .iter()
        .map(|plot| plot_zone_key(plot).to_string())
        .collect();
    zones.extend(tracker.zones().cloned());
    if zones.is_empty() {
        zones.push(DEFAULT_ZONE.to_string());
    }
    zones.sort();
    zones.dedup();

    for zone in zones {
        tracker.state_mut(&zone).prune(now);

        // 汐转 jitter：用 (zone_hash, lingtian_tick / day_ticks) 派生稳定 unit float
        // 避免每 tick 抖动；非汐转季节 amplitude=0 → 结果与 jitter 无关。
        let jitter_unit = derive_supply_jitter(&zone, now);
        let weather = active_weather.as_deref().and_then(|aw| aw.current(&zone));

        // 借用拆分：读出 pressure 先丢作用域，再改 state
        let pressure = {
            let plots_iter = plots.iter().filter_map(|m| {
                let plot: &LingtianPlot = m;
                (plot_zone_key(plot) == zone).then_some(plot)
            });
            compute_zone_pressure(
                &zone,
                plots_iter,
                &registry,
                &tracker,
                season,
                jitter_unit,
                weather,
            )
        };
        // plan-lingtian-weather-v1 §5 / worldview §七 — 阴霾期间天道注视减弱，
        // 阈值降 1 档（HeavyHaze.pressure_threshold_relax_steps()=1）。其他事件返回 0。
        let relax_steps = weather
            .map(|w| w.pressure_threshold_relax_steps())
            .unwrap_or(0);
        let new_level = PressureLevel::classify_with_relax(pressure, relax_steps);
        let old_level = tracker
            .state(&zone)
            .map(|s| s.last_level)
            .unwrap_or(PressureLevel::None);

        {
            let state = tracker.state_mut(&zone);
            state.last_pressure = pressure;
            state.last_level = new_level;
        }

        if new_level.is_higher_than(old_level) {
            events.send(ZonePressureCrossed {
                zone: zone.clone(),
                level: new_level,
                raw_pressure: pressure,
            });
            if matches!(new_level, PressureLevel::High) {
                // plan §5.1 — HIGH 触发该 zone plot_qi 瞬时清零
                for mut plot in plots.iter_mut() {
                    if plot_zone_key(&plot) == zone {
                        plot.plot_qi = 0.0;
                    }
                }
                tracing::warn!(
                    "[bong][lingtian] zone `{zone}` pressure HIGH (raw={pressure:.3}); cleared plot_qi"
                );
            }
        }
    }
}

/// 推一个 plot 一步：查 `PlantKind`、按 plot zone 取 zone qi、调 growth 公式。
///
/// 把"找 kind / 找 zone / 调用 advance"封装在一处，便于：
///   * `lingtian_growth_tick` system 在 Query 迭代里调
///   * 测试代码绕开 1200 个 Bevy tick 直推
pub fn advance_plot_one_lingtian_tick(
    plot: &mut LingtianPlot,
    registry: &PlantKindRegistry,
    zone_qi: &mut ZoneQiAccount,
) {
    advance_plot_one_lingtian_tick_in_zone(plot, registry, zone_qi, None);
}

fn advance_plot_one_lingtian_tick_in_zone(
    plot: &mut LingtianPlot,
    registry: &PlantKindRegistry,
    zone_qi: &mut ZoneQiAccount,
    zone_registry: Option<&ZoneRegistry>,
) {
    if plot_zone_is_collapsed(plot, zone_registry) {
        plot.plot_qi = 0.0;
        return;
    }

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
    let zone = plot_zone_key(plot).to_string();
    let zone_qi_ref = zone_qi.get_mut(&zone);
    advance_one_lingtian_tick(plot, kind, zone_qi_ref);
}

fn plot_zone_center(plot: &LingtianPlot) -> DVec3 {
    DVec3::new(
        plot.pos.x as f64 + 0.5,
        plot.pos.y as f64,
        plot.pos.z as f64 + 0.5,
    )
}

fn plot_zone_is_collapsed(plot: &LingtianPlot, zone_registry: Option<&ZoneRegistry>) -> bool {
    let Some(zone_registry) = zone_registry else {
        return false;
    };
    zone_registry
        .find_zone(
            crate::world::dimension::DimensionKind::Overworld,
            plot_zone_center(plot),
        )
        .is_some_and(|zone| {
            zone.active_events
                .iter()
                .any(|event| event == EVENT_REALM_COLLAPSE)
        })
}

// ============================================================================
// 端到端集成测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::{
        InventoryRevision, ItemInstance, ItemRarity, ItemTemplate, PlayerInventory,
        MAIN_PACK_CONTAINER_ID,
    };
    use crate::npc::spawn::NpcMarker;
    use crate::world::dimension::{DimensionKind, DimensionLayers, OverworldLayer, TsyLayer};
    use crate::world::dimension_transfer::{
        apply_dimension_transfers, DimensionTransferRequest, DimensionTransferSet,
    };
    use std::collections::HashMap;
    use valence::prelude::{
        App, BlockPos, DVec3, EntityLayerId, IntoSystemConfigs, Update, VisibleChunkLayer,
        VisibleEntityLayers,
    };

    use super::super::events::{
        DrainQiCompleted, DyeContaminationWarning, HarvestCompleted, PlantingCompleted,
        RenewCompleted, ReplenishCompleted, StartDrainQiRequest, StartHarvestRequest,
        StartPlantingRequest, StartRenewRequest, StartReplenishRequest, StartTillRequest,
        TillCompleted,
    };
    use super::super::session::{
        ReplenishSource, SessionMode, DRAIN_QI_TICKS, HARVEST_MANUAL_TICKS, PLANTING_TICKS,
        RENEW_TICKS, REPLENISH_COOLDOWN_LINGTIAN_TICKS, TILL_MANUAL_TICKS,
    };
    use super::super::terrain::TerrainKind;
    use crate::skill::events::XpGainSource;

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

    fn make_inventory_with_hoe(kind: HoeKind, durability: f64) -> PlayerInventory {
        let mut equipped = HashMap::new();
        equipped.insert(
            MAIN_HAND_SLOT.to_string(),
            crate::inventory::SlotContents::held_single(make_hoe_instance(kind, durability)),
        );
        PlayerInventory {
            triggered_treasures: Vec::new(),
            revision: InventoryRevision(0),
            containers: vec![],
            equipped,
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 45.0,
        }
    }

    fn spawn_test_player<T: bevy_ecs::bundle::Bundle>(app: &mut App, components: T) -> Entity {
        let (client_bundle, _helper) = valence::testing::create_mock_client("LingtianTest");
        let player = app.world_mut().spawn(client_bundle).id();
        app.world_mut().entity_mut(player).insert((
            components,
            Position(DVec3::new(0.5, 64.5, 0.5)),
            CurrentDimension(crate::world::dimension::DimensionKind::Overworld),
        ));
        player
    }

    fn set_test_player_position(app: &mut App, player: Entity, target: BlockPos) {
        app.world_mut()
            .entity_mut(player)
            .insert(Position(DVec3::new(
                f64::from(target.x) + 0.5,
                f64::from(target.y) + 0.5,
                f64::from(target.z) + 0.5,
            )));
    }

    fn valid_test_player<T: bevy_ecs::bundle::Bundle>(
        app: &mut App,
        components: T,
        target: BlockPos,
    ) -> Entity {
        let player = spawn_test_player(app, components);
        set_test_player_position(app, player, target);
        player
    }

    fn build_app() -> App {
        let mut app = App::new();
        app.insert_resource(ActiveLingtianSessions::new())
            .insert_resource(SeedRegistry::new())
            .insert_resource(PlantKindRegistry::new())
            .insert_resource(ItemRegistry::default())
            .insert_resource(InventoryInstanceIdAllocator::default())
            .insert_resource(LingtianHarvestRng::default())
            .insert_resource(ZoneQiAccount::new())
            .insert_resource(LingtianClock::default())
            .insert_resource(CombatClock::default())
            .insert_resource(ActiveEventsResource::default())
            // fix-spec-1901-v2 §4.1 — C2S 请求持久队列（测试 app 也要 init）。
            .init_resource::<crate::lingtian::requests::PendingLingtianRequests>()
            .add_event::<StartTillRequest>()
            .add_event::<TillCompleted>()
            .add_event::<StartRenewRequest>()
            .add_event::<RenewCompleted>()
            .add_event::<StartPlantingRequest>()
            .add_event::<PlantingCompleted>()
            .add_event::<StartHarvestRequest>()
            .add_event::<HarvestCompleted>()
            .add_event::<StartReplenishRequest>()
            .add_event::<ReplenishCompleted>()
            .add_event::<DyeContaminationWarning>()
            .add_event::<StartDrainQiRequest>()
            .add_event::<DrainQiCompleted>()
            .add_event::<QiTransfer>()
            .add_event::<SkillXpGain>()
            .add_systems(
                Update,
                (
                    validate_and_dispatch_lingtian_requests,
                    handle_start_till,
                    handle_start_renew,
                    handle_start_harvest,
                    handle_start_replenish,
                    handle_start_drain_qi,
                    tick_lingtian_sessions,
                    apply_completed_sessions,
                    record_dye_contamination_warning_recent_events,
                )
                    .chain()
                    .after(crate::world::movement_commit::AuthoritativePositionCommitSet),
            );
        app
    }

    #[test]
    fn start_plot_index_preserves_all_plots_at_duplicate_position() {
        let pos = BlockPos::new(7, 64, -3);
        let mut first = LingtianPlot::new(pos, None);
        first.plot_qi = 0.25;
        let mut second = LingtianPlot::new(pos, None);
        second.plot_qi = 0.75;
        let plots = vec![first, second];

        let index = build_start_plot_index(plots.iter(), None);
        let candidates = index
            .get(&pos)
            .expect("duplicate-position fixture must be indexed");
        assert_eq!(candidates.len(), 2, "同一位置的 plot 候选不能在索引中丢失");
        let selected = candidates
            .first()
            .copied()
            .expect("duplicate-position fixture must preserve the first plot");

        assert!(
            std::ptr::eq(selected, &plots[0]),
            "duplicate BlockPos must preserve the first plot, matching the previous find semantics"
        );
        assert_eq!(selected.plot_qi, 0.25);
    }

    #[test]
    fn renew_accepts_later_barren_duplicate_plot() {
        let mut app = build_app();
        let pos = BlockPos::new(8, 64, -3);
        let player = valid_test_player(
            &mut app,
            make_inventory_with_hoe(HoeKind::Xuantie, 1.0),
            pos,
        );
        app.world_mut().spawn(LingtianPlot::new(pos, Some(player)));
        let mut barren = LingtianPlot::new(pos, Some(player));
        barren.harvest_count = super::super::plot::N_RENEW;
        app.world_mut().spawn(barren);

        app.world_mut().send_event(StartRenewRequest {
            player,
            pos,
            hoe_instance_id: 1,
        });
        app.update();

        assert_eq!(
            app.world().resource::<ActiveLingtianSessions>().len(),
            1,
            "后续重复位置 plot 满足贫瘠条件时，Renew 不应被首个 plot 拒绝"
        );
    }

    #[test]
    fn planting_accepts_later_empty_duplicate_plot() {
        let mut app = build_planting_app();
        let pos = BlockPos::new(9, 64, -3);
        let player = valid_test_player(
            &mut app,
            make_inventory_with_seed("ci_she_hao_seed", 2),
            pos,
        );
        let mut blocked = LingtianPlot::new(pos, Some(player));
        blocked.crop = Some(CropInstance::new("ning_mai_cao".into()));
        app.world_mut().spawn(blocked);
        app.world_mut().spawn(LingtianPlot::new(pos, Some(player)));

        app.world_mut().send_event(StartPlantingRequest {
            player,
            pos,
            plant_id: "ci_she_hao".into(),
        });
        app.update();

        assert_eq!(
            app.world().resource::<ActiveLingtianSessions>().len(),
            1,
            "后续重复位置 plot 满足空且不贫瘠时，Planting 不应被首个 plot 拒绝"
        );
    }

    #[test]
    fn drain_qi_accepts_later_nonzero_duplicate_plot() {
        let mut app = build_app();
        let pos = BlockPos::new(10, 64, -3);
        let player = valid_test_player(
            &mut app,
            (empty_inventory_8x8(), LifeRecord::new("duplicate-drain")),
            pos,
        );
        app.world_mut().spawn(LingtianPlot::new(pos, None));
        let mut charged = LingtianPlot::new(pos, None);
        charged.plot_qi = 0.5;
        app.world_mut().spawn(charged);

        app.world_mut()
            .send_event(StartDrainQiRequest { player, pos });
        app.update();

        assert_eq!(
            app.world().resource::<ActiveLingtianSessions>().len(),
            1,
            "后续重复位置 plot 有真元时，DrainQi 不应被首个空 plot 拒绝"
        );
    }

    #[test]
    fn till_e2e_spawns_plot_and_decrements_durability() {
        let mut app = build_app();
        let pos = BlockPos::new(10, 64, 10);
        let player = spawn_test_player(&mut app, make_inventory_with_hoe(HoeKind::Iron, 1.0));
        set_test_player_position(&mut app, player, pos);
        app.world_mut().send_event(StartTillRequest {
            player,
            pos,
            hoe_instance_id: 1,
            mode: SessionMode::Manual,
            terrain: TerrainKind::Grass,
            environment: PlotEnvironment::base(),
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
        let dur = inv
            .equipped
            .get(MAIN_HAND_SLOT)
            .unwrap()
            .held
            .as_ref()
            .unwrap()
            .durability;
        assert!((dur - 0.95).abs() < 1e-9, "Iron 锄一次扣 0.05；实得 {dur}");
    }

    #[test]
    fn till_emits_vfx() {
        let mut app = build_app();
        app.add_event::<VfxEventRequest>();
        let pos = BlockPos::new(10, 64, 10);
        let player = spawn_test_player(&mut app, make_inventory_with_hoe(HoeKind::Iron, 1.0));
        set_test_player_position(&mut app, player, pos);
        app.world_mut().send_event(StartTillRequest {
            player,
            pos,
            hoe_instance_id: 1,
            mode: SessionMode::Manual,
            terrain: TerrainKind::Grass,
            environment: PlotEnvironment::base(),
        });
        for _ in 0..TILL_MANUAL_TICKS {
            app.update();
        }

        let events = app.world().resource::<Events<VfxEventRequest>>();
        let emitted = events
            .iter_current_update_events()
            .next()
            .expect("finished till session should emit vfx");
        match &emitted.payload {
            crate::schema::vfx_event::VfxEventPayloadV1::SpawnParticle { event_id, .. } => {
                assert_eq!(event_id, gameplay_vfx::LINGTIAN_TILL);
            }
            other => panic!("expected SpawnParticle, got {other:?}"),
        }
    }

    #[test]
    fn till_rejected_when_not_holding_hoe() {
        let mut app = build_app();
        // 玩家手里啥都没有
        let player = spawn_test_player(
            &mut app,
            PlayerInventory {
                triggered_treasures: Vec::new(),
                revision: InventoryRevision(0),
                containers: vec![],
                equipped: HashMap::new(),
                hotbar: Default::default(),
                bone_coins: 0,
                max_weight: 45.0,
            },
        );
        app.world_mut().send_event(StartTillRequest {
            player,
            pos: BlockPos::new(0, 64, 0),
            hoe_instance_id: 1,
            mode: SessionMode::Manual,
            terrain: TerrainKind::Grass,
            environment: PlotEnvironment::base(),
        });
        app.update();
        assert!(app.world().resource::<ActiveLingtianSessions>().is_empty());
    }

    #[test]
    fn till_rejected_on_blocked_terrain() {
        let mut app = build_app();
        let player = spawn_test_player(&mut app, make_inventory_with_hoe(HoeKind::Iron, 1.0));
        app.world_mut().send_event(StartTillRequest {
            player,
            pos: BlockPos::new(0, 64, 0),
            hoe_instance_id: 1,
            mode: SessionMode::Manual,
            terrain: TerrainKind::Stone,
            environment: PlotEnvironment::base(),
        });
        app.update();
        assert!(app.world().resource::<ActiveLingtianSessions>().is_empty());
    }

    #[test]
    fn equipped_main_hand_hoe_returns_kind_and_instance_id() {
        let inv = make_inventory_with_hoe(HoeKind::Lingtie, 0.5);
        let (kind, id) = equipped_main_hand_hoe(&inv).expect("should resolve");
        assert_eq!(kind, HoeKind::Lingtie);
        assert_eq!(id, 1, "make_hoe_instance 默认 instance_id=1");
    }

    #[test]
    fn equipped_main_hand_hoe_returns_none_for_non_hoe() {
        let mut equipped = HashMap::new();
        equipped.insert(
            MAIN_HAND_SLOT.to_string(),
            crate::inventory::SlotContents::held_single(ItemInstance {
                instance_id: 99,
                template_id: "rusted_blade".into(),
                display_name: "rusted_blade".into(),
                grid_w: 1,
                grid_h: 2,
                weight: 1.8,
                rarity: ItemRarity::Common,
                description: String::new(),
                stack_count: 1,
                spirit_quality: 0.8,
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
            }),
        );
        let inv = PlayerInventory {
            triggered_treasures: Vec::new(),
            revision: InventoryRevision(0),
            containers: vec![],
            equipped,
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 45.0,
        };
        assert!(equipped_main_hand_hoe(&inv).is_none());
    }

    fn terminal_settlement(entity: Entity) -> NpcTerminalSettlementSucceeded {
        let life_record = LifeRecord::new(format!("npc:lingtian-test:{}", entity.to_bits()));
        NpcTerminalSettlementSucceeded {
            entity,
            at_tick: 1,
            cause: crate::npc::lifecycle::NpcDeathReason::Combat
                .as_str()
                .to_string(),
            reason: crate::npc::lifecycle::NpcDeathReason::Combat,
            attacker: None,
            attacker_player_id: None,
            authorize_loot: true,
            actor_qi_identity: crate::cultivation::components::ActorQiIdentity::from_life_record(
                &life_record,
                crate::cultivation::components::ActorQiKind::Npc,
            )
            .expect("lingtian terminal fixture must have canonical identity"),
        }
    }

    #[test]
    fn release_lingtian_plot_owner_on_npc_death_clears_npc_owner() {
        let mut app = App::new();
        app.add_event::<NpcTerminalSettlementSucceeded>();
        app.add_systems(Update, release_lingtian_plot_owner_on_npc_death);

        let owner = app.world_mut().spawn(NpcMarker).id();
        let plot = app
            .world_mut()
            .spawn(LingtianPlot::new(BlockPos::new(1, 64, 1), Some(owner)))
            .id();

        app.world_mut().send_event(terminal_settlement(owner));
        app.update();

        assert_eq!(app.world().get::<LingtianPlot>(plot).unwrap().owner, None);
    }

    #[test]
    fn release_lingtian_plot_owner_ignores_unrelated_settlement() {
        let mut app = App::new();
        app.add_event::<NpcTerminalSettlementSucceeded>();
        app.add_systems(Update, release_lingtian_plot_owner_on_npc_death);

        let owner = app.world_mut().spawn(NpcMarker).id();
        let other_npc = app.world_mut().spawn(NpcMarker).id();
        let plot = app
            .world_mut()
            .spawn(LingtianPlot::new(BlockPos::new(1, 64, 1), Some(owner)))
            .id();

        app.world_mut().send_event(terminal_settlement(other_npc));
        app.update();

        assert_eq!(
            app.world().get::<LingtianPlot>(plot).unwrap().owner,
            Some(owner)
        );
    }

    #[test]
    fn till_rejected_when_request_instance_id_mismatches_main_hand() {
        let mut app = build_app();
        let player = spawn_test_player(&mut app, make_inventory_with_hoe(HoeKind::Iron, 1.0));
        // 主手 instance_id=1，但请求声 instance_id=2 → 应被拒
        app.world_mut().send_event(StartTillRequest {
            player,
            pos: BlockPos::new(0, 64, 0),
            hoe_instance_id: 2,
            mode: SessionMode::Manual,
            terrain: TerrainKind::Grass,
            environment: PlotEnvironment::base(),
        });
        app.update();
        assert!(app.world().resource::<ActiveLingtianSessions>().is_empty());
    }

    #[test]
    fn second_till_during_active_session_is_rejected() {
        let mut app = build_app();
        let player = spawn_test_player(&mut app, make_inventory_with_hoe(HoeKind::Iron, 1.0));
        app.world_mut().send_event(StartTillRequest {
            player,
            pos: BlockPos::new(0, 64, 0),
            hoe_instance_id: 1,
            mode: SessionMode::Manual,
            terrain: TerrainKind::Grass,
            environment: PlotEnvironment::base(),
        });
        app.update();
        assert_eq!(app.world().resource::<ActiveLingtianSessions>().len(), 1);
        // 第二请求应被拒
        app.world_mut().send_event(StartTillRequest {
            player,
            pos: BlockPos::new(1, 64, 0),
            hoe_instance_id: 1,
            mode: SessionMode::Manual,
            terrain: TerrainKind::Dirt,
            environment: PlotEnvironment::base(),
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
        let pos = BlockPos::new(5, 64, 5);
        let player = valid_test_player(
            &mut app,
            make_inventory_with_hoe(HoeKind::Xuantie, 1.0),
            pos,
        );
        // 直接 spawn 一个贫瘠 plot
        let mut plot = LingtianPlot::new(pos, Some(player));
        plot.harvest_count = super::super::plot::N_RENEW;
        app.world_mut().spawn(plot);

        app.world_mut().send_event(StartRenewRequest {
            player,
            pos,
            hoe_instance_id: 1,
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
        let dur = inv
            .equipped
            .get(MAIN_HAND_SLOT)
            .unwrap()
            .held
            .as_ref()
            .unwrap()
            .durability;
        assert!((dur - 0.99).abs() < 1e-9, "Xuantie 一次扣 0.01；实得 {dur}");
    }

    #[test]
    fn renew_rejected_when_plot_not_barren() {
        let mut app = build_app();
        let player = spawn_test_player(&mut app, make_inventory_with_hoe(HoeKind::Iron, 1.0));
        let pos = BlockPos::new(0, 64, 0);
        // 新 plot，未贫瘠
        app.world_mut().spawn(LingtianPlot::new(pos, None));
        app.world_mut().send_event(StartRenewRequest {
            player,
            pos,
            hoe_instance_id: 1,
        });
        app.update();
        assert!(app.world().resource::<ActiveLingtianSessions>().is_empty());
    }

    #[test]
    fn hoe_breaks_at_zero_durability() {
        let mut app = build_app();
        // Iron 锄剩 0.05 → 一次操作就归零（uses_max=20，cost=0.05）
        let pos = BlockPos::new(0, 64, 0);
        let player = spawn_test_player(&mut app, make_inventory_with_hoe(HoeKind::Iron, 0.05));
        app.world_mut().send_event(StartTillRequest {
            player,
            pos,
            hoe_instance_id: 1,
            mode: SessionMode::Manual,
            terrain: TerrainKind::Grass,
            environment: PlotEnvironment::base(),
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
    use crate::lingtian::environment::{PlotBiome, PlotEnvironment, PlotLingjuTier};
    use crate::lingtian::plot::CropInstance;
    use crate::lingtian::qi_account::BEVY_TICKS_PER_LINGTIAN_TICK;
    use crate::world::season::Season;

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
            .insert_resource(LingtianClock::default())
            .insert_resource(acc)
            .insert_resource(registry_with(ci_she_hao_kind()))
            .add_systems(Update, lingtian_growth_tick);
        app
    }

    fn build_collapsed_growth_app(zone_qi: f32) -> App {
        let mut app = build_growth_app(zone_qi);
        app.insert_resource(ZoneRegistry {
            zones: vec![crate::world::zone::Zone {
                name: "collapsed_test".to_string(),
                dimension: crate::world::dimension::DimensionKind::Overworld,
                bounds: (DVec3::new(-16.0, 0.0, -16.0), DVec3::new(16.0, 256.0, 16.0)),
                spirit_qi: 0.0,
                danger_level: 5,
                active_events: vec![EVENT_REALM_COLLAPSE.to_string()],
                patrol_anchors: Vec::new(),
                blocked_tiles: Vec::new(),
                qi_equilibrium: 0.0,
                qi_inflow_per_min: 0.0,
            }],
            spatial_revision: 0,
        });
        app
    }

    fn spawn_planted_plot(app: &mut App, plot_qi: f32) -> Entity {
        spawn_planted_plot_in_zone(app, plot_qi, "")
    }

    fn spawn_planted_plot_in_zone(app: &mut App, plot_qi: f32, zone: &str) -> Entity {
        let mut p = LingtianPlot::new(BlockPos::new(0, 64, 0), None);
        p.zone = zone.to_string();
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
            let zone_registry = world.get_resource::<ZoneRegistry>().cloned();
            let mut state = world.query::<&mut LingtianPlot>();
            for mut plot in state.iter_mut(world) {
                advance_plot_one_lingtian_tick_in_zone(
                    &mut plot,
                    &registry,
                    &mut zone_qi,
                    zone_registry.as_ref(),
                );
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
    fn zone_leak_path_uses_plot_zone_when_non_default() {
        let mut app = build_growth_app(0.0);
        app.world_mut()
            .resource_mut::<ZoneQiAccount>()
            .set("blood_valley", 2.0);
        let plot = spawn_planted_plot_in_zone(&mut app, 0.0, "blood_valley");

        advance_n_lingtian_ticks_direct(&mut app, 10);

        let p = app.world().get::<LingtianPlot>(plot).unwrap();
        let g = p.crop.as_ref().unwrap().growth;
        let expected = 10.0 * (1.0_f32 / 480.0) * 0.3;
        assert!(
            (g - expected).abs() < 1e-5,
            "非默认区 plot 应从 blood_valley 漏吸生长，growth={g}, expected≈{expected}"
        );
        let accounts = app.world().resource::<ZoneQiAccount>();
        let remote_left = accounts.get("blood_valley");
        let default_left = accounts.get(DEFAULT_ZONE);
        let zone_consumed = 10.0 * 0.002 * 0.2;
        assert!(
            (remote_left - (2.0 - zone_consumed)).abs() < 1e-5,
            "blood_valley 应被扣漏吸量，实际 {remote_left}"
        );
        assert_eq!(default_left, 0.0, "非默认区生长不应触碰 default zone");
    }

    #[test]
    fn collapsed_zone_clears_plot_qi_and_stops_growth() {
        let mut app = build_collapsed_growth_app(2.0);
        let plot = spawn_planted_plot(&mut app, 1.0);

        advance_n_lingtian_ticks_direct(&mut app, 1);
        let p = app.world().get::<LingtianPlot>(plot).unwrap();

        assert_eq!(p.plot_qi, 0.0);
        assert_eq!(p.crop.as_ref().unwrap().growth, 0.0);
        assert_eq!(
            app.world().resource::<ZoneQiAccount>().get(DEFAULT_ZONE),
            2.0
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

    // ------------------------------------------------------------------------
    // P3 种植 e2e
    // ------------------------------------------------------------------------

    use crate::inventory::{ContainerState, PlacedItemState};

    fn registry_with_three_test_plants() -> PlantKindRegistry {
        let mut r = PlantKindRegistry::new();
        for id in ["ci_she_hao", "ning_mai_cao", "ling_mu_miao"] {
            r.insert(PlantKind {
                id: id.into(),
                display_name: id.into(),
                cultivable: true,
                growth_cost: GrowthCost::Low,
                growth_duration_ticks: 480,
                rarity: PlantRarity::Common,
                description: String::new(),
            })
            .unwrap();
        }
        r
    }

    fn make_seed_instance(template_id: &str, stack: u32) -> ItemInstance {
        ItemInstance {
            instance_id: 100,
            template_id: template_id.into(),
            display_name: template_id.into(),
            grid_w: 1,
            grid_h: 1,
            weight: 0.05,
            rarity: ItemRarity::Common,
            description: String::new(),
            stack_count: stack,
            spirit_quality: 1.0,
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

    fn make_inventory_with_seed(template_id: &str, stack: u32) -> PlayerInventory {
        let container = ContainerState {
            quick_access: false,
            id: "main_pack".into(),
            name: "main_pack".into(),
            rows: 4,
            cols: 4,
            items: vec![PlacedItemState {
                row: 0,
                col: 0,
                instance: make_seed_instance(template_id, stack),
            }],

            owner_instance_id: None,
        };
        PlayerInventory {
            triggered_treasures: Vec::new(),
            revision: InventoryRevision(0),
            containers: vec![container],
            equipped: HashMap::new(),
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 45.0,
        }
    }

    fn build_planting_app() -> App {
        let mut app = App::new();
        let registry = registry_with_three_test_plants();
        let seeds = SeedRegistry::from_plant_registry(&registry);
        app.insert_resource(ActiveLingtianSessions::new())
            .insert_resource(registry)
            .insert_resource(seeds)
            .insert_resource(ItemRegistry::default())
            .insert_resource(InventoryInstanceIdAllocator::default())
            .insert_resource(LingtianHarvestRng::default())
            .insert_resource(ZoneQiAccount::new())
            .insert_resource(LingtianClock::default())
            .add_event::<StartPlantingRequest>()
            .add_event::<PlantingCompleted>()
            .add_event::<StartTillRequest>()
            .add_event::<TillCompleted>()
            .add_event::<StartRenewRequest>()
            .add_event::<RenewCompleted>()
            .add_event::<StartHarvestRequest>()
            .add_event::<HarvestCompleted>()
            .add_event::<StartReplenishRequest>()
            .add_event::<ReplenishCompleted>()
            .add_event::<DyeContaminationWarning>()
            .add_event::<StartDrainQiRequest>()
            .add_event::<DrainQiCompleted>()
            .add_event::<QiTransfer>()
            .add_event::<SkillXpGain>()
            // fix-spec-1901-v2 §4.1 — C2S 请求持久队列。
            .init_resource::<crate::lingtian::requests::PendingLingtianRequests>()
            .add_systems(
                Update,
                (
                    validate_and_dispatch_lingtian_requests,
                    handle_start_till,
                    handle_start_renew,
                    handle_start_planting,
                    handle_start_harvest,
                    handle_start_replenish,
                    handle_start_drain_qi,
                    tick_lingtian_sessions,
                    apply_completed_sessions,
                )
                    .chain()
                    .after(crate::world::movement_commit::AuthoritativePositionCommitSet),
            );
        app
    }

    #[test]
    fn planting_e2e_spawns_crop_and_consumes_seed() {
        let mut app = build_planting_app();
        let pos = BlockPos::new(0, 64, 0);
        let player = valid_test_player(
            &mut app,
            make_inventory_with_seed("ci_she_hao_seed", 5),
            pos,
        );
        // 已开垦的空 plot
        app.world_mut().spawn(LingtianPlot::new(pos, Some(player)));
        app.world_mut().send_event(StartPlantingRequest {
            player,
            pos,
            plant_id: "ci_she_hao".into(),
        });
        for _ in 0..PLANTING_TICKS {
            app.update();
        }
        // session 应已结算
        assert!(app.world().resource::<ActiveLingtianSessions>().is_empty());
        // plot 应有 crop = ci_she_hao
        let plot = app
            .world_mut()
            .query::<&LingtianPlot>()
            .iter(app.world())
            .next()
            .unwrap();
        assert_eq!(
            plot.crop.as_ref().map(|c| c.kind.as_str()),
            Some("ci_she_hao")
        );
        // 种子应 -1（5 → 4）
        let inv = app.world().get::<PlayerInventory>(player).unwrap();
        let stack = inv.containers[0].items[0].instance.stack_count;
        assert_eq!(stack, 4);
    }

    #[test]
    fn planting_consumes_last_seed_then_removes_stack() {
        let mut app = build_planting_app();
        let pos = BlockPos::new(1, 64, 1);
        let player = valid_test_player(
            &mut app,
            make_inventory_with_seed("ning_mai_cao_seed", 1),
            pos,
        );
        app.world_mut().spawn(LingtianPlot::new(pos, Some(player)));
        app.world_mut().send_event(StartPlantingRequest {
            player,
            pos,
            plant_id: "ning_mai_cao".into(),
        });
        for _ in 0..PLANTING_TICKS {
            app.update();
        }
        let inv = app.world().get::<PlayerInventory>(player).unwrap();
        assert!(inv.containers[0].items.is_empty(), "最后 1 颗扣完应空格");
    }

    #[test]
    fn planting_rejected_when_no_seed_in_inventory() {
        let mut app = build_planting_app();
        let pos = BlockPos::new(0, 64, 0);
        let player = valid_test_player(
            &mut app,
            make_inventory_with_seed("ci_she_hao_seed", 1),
            pos,
        );
        app.world_mut().spawn(LingtianPlot::new(pos, Some(player)));
        // 请求种 ling_mu_miao（没种子）
        app.world_mut().send_event(StartPlantingRequest {
            player,
            pos,
            plant_id: "ling_mu_miao".into(),
        });
        app.update();
        assert!(app.world().resource::<ActiveLingtianSessions>().is_empty());
    }

    #[test]
    fn planting_rejected_when_plot_already_has_crop() {
        let mut app = build_planting_app();
        let pos = BlockPos::new(0, 64, 0);
        let player = valid_test_player(
            &mut app,
            make_inventory_with_seed("ci_she_hao_seed", 5),
            pos,
        );
        let mut plot = LingtianPlot::new(pos, Some(player));
        plot.crop = Some(CropInstance::new("ning_mai_cao".into()));
        app.world_mut().spawn(plot);
        app.world_mut().send_event(StartPlantingRequest {
            player,
            pos,
            plant_id: "ci_she_hao".into(),
        });
        app.update();
        assert!(app.world().resource::<ActiveLingtianSessions>().is_empty());
    }

    #[test]
    fn planting_rejected_when_plot_barren() {
        let mut app = build_planting_app();
        let pos = BlockPos::new(0, 64, 0);
        let player = valid_test_player(
            &mut app,
            make_inventory_with_seed("ci_she_hao_seed", 5),
            pos,
        );
        let mut plot = LingtianPlot::new(pos, Some(player));
        plot.harvest_count = crate::lingtian::plot::N_RENEW; // 贫瘠
        app.world_mut().spawn(plot);
        app.world_mut().send_event(StartPlantingRequest {
            player,
            pos,
            plant_id: "ci_she_hao".into(),
        });
        app.update();
        assert!(app.world().resource::<ActiveLingtianSessions>().is_empty());
    }

    #[test]
    fn planting_rejected_when_plant_id_unknown_to_seed_registry() {
        let mut app = build_planting_app();
        let pos = BlockPos::new(0, 64, 0);
        let player = valid_test_player(
            &mut app,
            make_inventory_with_seed("ci_she_hao_seed", 5),
            pos,
        );
        app.world_mut().spawn(LingtianPlot::new(pos, Some(player)));
        // shi_mai_gen 非 cultivable，SeedRegistry 不应有它
        app.world_mut().send_event(StartPlantingRequest {
            player,
            pos,
            plant_id: "shi_mai_gen".into(),
        });
        app.update();
        assert!(app.world().resource::<ActiveLingtianSessions>().is_empty());
    }

    // ------------------------------------------------------------------------
    // P4 收获 e2e
    // ------------------------------------------------------------------------

    use crate::inventory::{ItemCategory, ItemEffect};

    fn herb_template(id: &str, display: &str) -> ItemTemplate {
        ItemTemplate {
            id: id.into(),
            display_name: display.into(),
            category: ItemCategory::Herb,
            placeable: None,
            max_stack_count: 64,
            grid_w: 1,
            grid_h: 1,
            base_weight: 0.1,
            rarity: ItemRarity::Common,
            spirit_quality_initial: 0.85,
            description: String::new(),
            effect: None as Option<ItemEffect>,
            cast_duration_ms: 1500,
            cooldown_ms: 1500,
            weapon_spec: None,
            forge_station_spec: None,
            blueprint_scroll_spec: None,
            inscription_scroll_spec: None,
            technique_scroll_spec: None,
            readable_scroll_spec: None,
            recipe_fragment_spec: None,
            container_spec: None,
            shelflife_profile: None,
            shield_spec: None,
            shelflife_track: None,
            wearer_race: crate::body_plan::types::RaceGateOwned::default(),
        }
    }

    fn seed_template(id: &str) -> ItemTemplate {
        ItemTemplate {
            id: id.into(),
            display_name: id.into(),
            category: ItemCategory::Misc,
            placeable: None,
            max_stack_count: 1,
            grid_w: 1,
            grid_h: 1,
            base_weight: 0.05,
            rarity: ItemRarity::Common,
            spirit_quality_initial: 0.7,
            description: String::new(),
            effect: None as Option<ItemEffect>,
            cast_duration_ms: 1500,
            cooldown_ms: 1500,
            weapon_spec: None,
            forge_station_spec: None,
            blueprint_scroll_spec: None,
            inscription_scroll_spec: None,
            technique_scroll_spec: None,
            readable_scroll_spec: None,
            recipe_fragment_spec: None,
            container_spec: None,
            shelflife_profile: None,
            shield_spec: None,
            shelflife_track: None,
            wearer_race: crate::body_plan::types::RaceGateOwned::default(),
        }
    }

    fn registry_with_herb_and_seed_templates() -> ItemRegistry {
        let mut m = HashMap::new();
        for id in ["ci_she_hao", "ning_mai_cao", "ling_mu_miao"] {
            m.insert(id.to_string(), herb_template(id, id));
        }
        for id in ["ci_she_hao_seed", "ning_mai_cao_seed", "ling_mu_miao_seed"] {
            m.insert(id.to_string(), seed_template(id));
        }
        ItemRegistry::from_map(m)
    }

    fn build_harvest_app_with_item_registry(item_registry: ItemRegistry) -> App {
        let mut app = App::new();
        let plant_registry = registry_with_three_test_plants();
        let seeds = SeedRegistry::from_plant_registry(&plant_registry);
        app.insert_resource(ActiveLingtianSessions::new())
            .insert_resource(plant_registry)
            .insert_resource(seeds)
            .insert_resource(item_registry)
            .insert_resource(InventoryInstanceIdAllocator::default())
            .insert_resource(LingtianHarvestRng::new(0xDEAD_BEEF))
            .insert_resource(ZoneQiAccount::new())
            .insert_resource(LingtianClock::default())
            .add_event::<StartHarvestRequest>()
            .add_event::<HarvestCompleted>()
            .add_event::<StartTillRequest>()
            .add_event::<TillCompleted>()
            .add_event::<StartRenewRequest>()
            .add_event::<RenewCompleted>()
            .add_event::<StartPlantingRequest>()
            .add_event::<PlantingCompleted>()
            .add_event::<StartReplenishRequest>()
            .add_event::<ReplenishCompleted>()
            .add_event::<DyeContaminationWarning>()
            .add_event::<StartDrainQiRequest>()
            .add_event::<DrainQiCompleted>()
            .add_event::<QiTransfer>()
            .add_event::<SkillXpGain>()
            // fix-spec-1901-v2 §4.1 — C2S 请求持久队列。
            .init_resource::<crate::lingtian::requests::PendingLingtianRequests>()
            .add_systems(
                Update,
                (
                    validate_and_dispatch_lingtian_requests,
                    handle_start_till,
                    handle_start_renew,
                    handle_start_planting,
                    handle_start_harvest,
                    handle_start_replenish,
                    handle_start_drain_qi,
                    tick_lingtian_sessions,
                    apply_completed_sessions,
                )
                    .chain()
                    .after(crate::world::movement_commit::AuthoritativePositionCommitSet),
            );
        app
    }

    fn build_harvest_app() -> App {
        build_harvest_app_with_item_registry(registry_with_herb_and_seed_templates())
    }

    fn empty_inventory_8x8() -> PlayerInventory {
        let main_pack = ContainerState {
            quick_access: false,
            id: MAIN_PACK_CONTAINER_ID.into(),
            name: "main".into(),
            rows: 8,
            cols: 8,
            items: vec![],
            owner_instance_id: None,
        };
        PlayerInventory {
            triggered_treasures: Vec::new(),
            revision: InventoryRevision(0),
            containers: vec![main_pack],
            equipped: HashMap::new(),
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 999.0,
        }
    }

    fn spawn_ripe_plot(app: &mut App, plant_id: &str, pos: BlockPos) -> Entity {
        let mut p = LingtianPlot::new(pos, None);
        let mut crop = CropInstance::new(plant_id.into());
        crop.growth = 1.0;
        p.crop = Some(crop);
        app.world_mut().spawn(p).id()
    }

    fn count_in_main_pack(inv: &PlayerInventory, template_id: &str) -> u32 {
        inv.containers
            .iter()
            .find(|c| c.id == MAIN_PACK_CONTAINER_ID)
            .map(|c| {
                c.items
                    .iter()
                    .filter(|p| p.instance.template_id == template_id)
                    .map(|p| p.instance.stack_count)
                    .sum::<u32>()
            })
            .unwrap_or(0)
    }

    fn count_in_all_containers(inv: &PlayerInventory, template_id: &str) -> u32 {
        inv.containers
            .iter()
            .flat_map(|container| container.items.iter())
            .filter(|placed| placed.instance.template_id == template_id)
            .map(|placed| placed.instance.stack_count)
            .sum()
    }

    #[test]
    fn harvest_e2e_drops_plant_and_clears_plot() {
        let mut app = build_harvest_app();
        let pos = BlockPos::new(2, 64, 2);
        let player = valid_test_player(&mut app, empty_inventory_8x8(), pos);
        let plot = spawn_ripe_plot(&mut app, "ci_she_hao", pos);
        app.world_mut().send_event(StartHarvestRequest {
            player,
            pos,
            mode: SessionMode::Manual,
        });
        for _ in 0..HARVEST_MANUAL_TICKS {
            app.update();
        }
        assert!(app.world().resource::<ActiveLingtianSessions>().is_empty());
        let p = app.world().get::<LingtianPlot>(plot).unwrap();
        assert!(p.crop.is_none(), "plot 应空");
        assert_eq!(p.harvest_count, 1, "harvest_count 应 +1");
        let inv = app.world().get::<PlayerInventory>(player).unwrap();
        assert_eq!(count_in_main_pack(inv, "ci_she_hao"), 1, "应得 1 株作物");
    }

    #[test]
    fn harvest_with_default_runtime_pack_without_main_pack_is_not_lost() {
        let item_registry =
            crate::inventory::load_item_registry().expect("item registry should load");
        let loadout =
            crate::inventory::load_default_loadout(&item_registry).expect("default loadout loads");
        let mut loadout_allocator = InventoryInstanceIdAllocator::new(3000);
        let inventory = crate::inventory::instantiate_inventory_from_loadout(
            &loadout,
            &mut loadout_allocator,
            &item_registry,
        )
        .expect("default loadout should instantiate");
        let runtime_pack_id = inventory
            .containers
            .iter()
            .find_map(|container| {
                container
                    .id
                    .strip_prefix("pack_")
                    .map(|_| container.id.clone())
            })
            .expect("default loadout should derive a runtime pack_<instance_id> container");
        assert!(
            inventory
                .containers
                .iter()
                .all(|container| container.id != MAIN_PACK_CONTAINER_ID),
            "default loadout no longer creates `{MAIN_PACK_CONTAINER_ID}`; ids={:?}",
            inventory
                .containers
                .iter()
                .map(|container| container.id.as_str())
                .collect::<Vec<_>>()
        );

        let mut app = build_harvest_app_with_item_registry(item_registry);
        let pos = BlockPos::new(4, 64, 4);
        let player = valid_test_player(&mut app, inventory, pos);
        let plot = spawn_ripe_plot(&mut app, "ci_she_hao", pos);
        app.world_mut().send_event(StartHarvestRequest {
            player,
            pos,
            mode: SessionMode::Manual,
        });

        for _ in 0..HARVEST_MANUAL_TICKS {
            app.update();
        }

        let plot = app.world().get::<LingtianPlot>(plot).unwrap();
        assert!(plot.crop.is_none(), "收获完成后 plot 应清空");
        let inv = app.world().get::<PlayerInventory>(player).unwrap();
        assert_eq!(
            count_in_all_containers(inv, "ci_she_hao"),
            1,
            "灵田收获奖励必须进入随身容器，不能因缺 `{MAIN_PACK_CONTAINER_ID}` 静默丢失；\
             runtime_pack={runtime_pack_id}, containers={:?}",
            inv.containers
                .iter()
                .map(|container| {
                    (
                        container.id.as_str(),
                        container
                            .items
                            .iter()
                            .map(|placed| placed.instance.template_id.as_str())
                            .collect::<Vec<_>>(),
                    )
                })
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn harvest_rejected_when_crop_not_ripe() {
        let mut app = build_harvest_app();
        let pos = BlockPos::new(0, 64, 0);
        let player = valid_test_player(&mut app, empty_inventory_8x8(), pos);
        let mut p = LingtianPlot::new(pos, None);
        let mut crop = CropInstance::new("ci_she_hao".into());
        crop.growth = 0.5;
        p.crop = Some(crop);
        app.world_mut().spawn(p);
        app.world_mut().send_event(StartHarvestRequest {
            player,
            pos,
            mode: SessionMode::Manual,
        });
        app.update();
        assert!(app.world().resource::<ActiveLingtianSessions>().is_empty());
    }

    #[test]
    fn harvest_rejected_when_no_crop() {
        let mut app = build_harvest_app();
        let pos = BlockPos::new(0, 64, 0);
        let player = valid_test_player(&mut app, empty_inventory_8x8(), pos);
        app.world_mut().spawn(LingtianPlot::new(pos, None));
        app.world_mut().send_event(StartHarvestRequest {
            player,
            pos,
            mode: SessionMode::Manual,
        });
        app.update();
        assert!(app.world().resource::<ActiveLingtianSessions>().is_empty());
    }

    // ───────────────────────── F23 — Auto 采集服务端等级门禁 ─────────────────────────

    fn skill_set_with_herbalism_lv(lv: u8) -> SkillSet {
        let mut skills = HashMap::new();
        skills.insert(
            SkillId::Herbalism,
            crate::skill::components::SkillEntry {
                lv,
                ..Default::default()
            },
        );
        SkillSet {
            skills,
            consumed_scrolls: Default::default(),
        }
    }

    #[test]
    fn harvest_auto_rejected_when_herbalism_below_unlock_level() {
        let mut app = build_harvest_app();
        let pos = BlockPos::new(0, 64, 0);
        let player = valid_test_player(&mut app, empty_inventory_8x8(), pos);
        // auto_unlock_level 默认 3；lv=2 明确不足。
        app.world_mut()
            .entity_mut(player)
            .insert(skill_set_with_herbalism_lv(2));
        spawn_ripe_plot(&mut app, "ci_she_hao", pos);
        app.world_mut().send_event(StartHarvestRequest {
            player,
            pos,
            mode: SessionMode::Auto,
        });
        app.update();

        assert!(
            app.world().resource::<ActiveLingtianSessions>().is_empty(),
            "F23: herbalism lv=2 < auto_unlock_level=3 必须被服务端拒绝，不能靠 client UI \
             单层 gating（协议可绕过）"
        );
    }

    #[test]
    fn harvest_auto_rejected_when_herbalism_missing_entirely() {
        // 完全没有 SkillSet/Cultivation 组件 —— 等价 herbalism lv=0，同样必须被拒。
        let mut app = build_harvest_app();
        let pos = BlockPos::new(0, 64, 0);
        let player = valid_test_player(&mut app, empty_inventory_8x8(), pos);
        spawn_ripe_plot(&mut app, "ci_she_hao", pos);
        app.world_mut().send_event(StartHarvestRequest {
            player,
            pos,
            mode: SessionMode::Auto,
        });
        app.update();

        assert!(
            app.world().resource::<ActiveLingtianSessions>().is_empty(),
            "F23: 缺 SkillSet 组件时 herbalism_effective_lv 应回落到 0，同样触发门禁拒绝"
        );
    }

    #[test]
    fn harvest_auto_allowed_when_herbalism_meets_unlock_level() {
        let mut app = build_harvest_app();
        let pos = BlockPos::new(0, 64, 0);
        let player = valid_test_player(&mut app, empty_inventory_8x8(), pos);
        // 刚好等于 auto_unlock_level=3 —— 边界值必须放行（>= 不是 >）。
        app.world_mut()
            .entity_mut(player)
            .insert(skill_set_with_herbalism_lv(3));
        spawn_ripe_plot(&mut app, "ci_she_hao", pos);
        app.world_mut().send_event(StartHarvestRequest {
            player,
            pos,
            mode: SessionMode::Auto,
        });
        app.update();

        assert!(
            !app.world().resource::<ActiveLingtianSessions>().is_empty(),
            "F23: herbalism lv=3 == auto_unlock_level 边界应放行，不应被拒"
        );
    }

    #[test]
    fn harvest_auto_allowed_when_herbalism_well_above_unlock_level() {
        let mut app = build_harvest_app();
        let pos = BlockPos::new(0, 64, 0);
        let player = valid_test_player(&mut app, empty_inventory_8x8(), pos);
        app.world_mut()
            .entity_mut(player)
            .insert(skill_set_with_herbalism_lv(10));
        spawn_ripe_plot(&mut app, "ci_she_hao", pos);
        app.world_mut().send_event(StartHarvestRequest {
            player,
            pos,
            mode: SessionMode::Auto,
        });
        app.update();

        assert!(
            !app.world().resource::<ActiveLingtianSessions>().is_empty(),
            "F23: herbalism lv 远高于门禁值时必须放行"
        );
    }

    #[test]
    fn harvest_manual_mode_is_unaffected_by_herbalism_level() {
        // Manual 模式完全不该受门禁影响，即使玩家一点采集技艺都没有。
        let mut app = build_harvest_app();
        let pos = BlockPos::new(0, 64, 0);
        let player = valid_test_player(&mut app, empty_inventory_8x8(), pos);
        spawn_ripe_plot(&mut app, "ci_she_hao", pos);
        app.world_mut().send_event(StartHarvestRequest {
            player,
            pos,
            mode: SessionMode::Manual,
        });
        app.update();

        assert!(
            !app.world().resource::<ActiveLingtianSessions>().is_empty(),
            "F23: Manual 模式不应被 herbalism 等级门禁拦截"
        );
    }

    #[test]
    fn five_harvests_make_plot_barren() {
        let mut app = build_harvest_app();
        let pos = BlockPos::new(3, 64, 3);
        let player = valid_test_player(&mut app, empty_inventory_8x8(), pos);
        // 收 N_RENEW 次：每次都重新种熟（手动设 growth=1）
        let plot = spawn_ripe_plot(&mut app, "ci_she_hao", pos);
        for i in 0..crate::lingtian::plot::N_RENEW {
            app.world_mut().send_event(StartHarvestRequest {
                player,
                pos,
                mode: SessionMode::Manual,
            });
            for _ in 0..HARVEST_MANUAL_TICKS {
                app.update();
            }
            // 复种（绕过 PlantingSession，直接重熟）
            let mut p = app.world_mut().get_mut::<LingtianPlot>(plot).unwrap();
            assert_eq!(p.harvest_count, i + 1);
            if i + 1 < crate::lingtian::plot::N_RENEW {
                let mut crop = CropInstance::new("ci_she_hao".into());
                crop.growth = 1.0;
                p.crop = Some(crop);
            }
        }
        let p = app.world().get::<LingtianPlot>(plot).unwrap();
        assert!(p.is_barren(), "5 次收获后应贫瘠");
    }

    #[test]
    fn harvest_stack_increments_existing_stack() {
        let mut app = build_harvest_app();
        // 玩家先有一摞 ci_she_hao = 3
        let mut inv = empty_inventory_8x8();
        inv.containers[0]
            .items
            .push(crate::inventory::PlacedItemState {
                row: 0,
                col: 0,
                instance: ItemInstance {
                    instance_id: 999,
                    template_id: "ci_she_hao".into(),
                    display_name: "ci_she_hao".into(),
                    grid_w: 1,
                    grid_h: 1,
                    weight: 0.1,
                    rarity: ItemRarity::Common,
                    description: String::new(),
                    stack_count: 3,
                    spirit_quality: 0.85,
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
                },
            });
        let pos = BlockPos::new(0, 64, 0);
        let player = valid_test_player(&mut app, inv, pos);
        spawn_ripe_plot(&mut app, "ci_she_hao", pos);
        app.world_mut().send_event(StartHarvestRequest {
            player,
            pos,
            mode: SessionMode::Manual,
        });
        for _ in 0..HARVEST_MANUAL_TICKS {
            app.update();
        }
        let inv = app.world().get::<PlayerInventory>(player).unwrap();
        assert_eq!(count_in_main_pack(inv, "ci_she_hao"), 4, "原 3 → 4");
        // 校验"叠到原摞而非新建" — 数 ci_she_hao 的 PlacedItemState 数量
        // （种子可能另起一摞，所以不能数总 items.len）
        let ci_she_hao_stacks = inv.containers[0]
            .items
            .iter()
            .filter(|p| p.instance.template_id == "ci_she_hao")
            .count();
        assert_eq!(ci_she_hao_stacks, 1, "应叠到原 ci_she_hao 摞");
    }

    #[test]
    fn harvest_completion_emits_herbalism_skill_xp() {
        let mut app = build_harvest_app();
        let pos = BlockPos::new(0, 64, 0);
        let player = valid_test_player(&mut app, empty_inventory_8x8(), pos);
        spawn_ripe_plot(&mut app, "ci_she_hao", pos);
        app.world_mut().send_event(StartHarvestRequest {
            player,
            pos,
            mode: SessionMode::Manual,
        });

        for _ in 0..HARVEST_MANUAL_TICKS {
            app.update();
        }

        let xp_events = app.world().resource::<Events<SkillXpGain>>();
        let xp = xp_events
            .iter_current_update_events()
            .next()
            .expect("harvest should emit herbalism xp");
        assert_eq!(xp.char_entity, player);
        assert_eq!(xp.skill, SkillId::Herbalism);
        assert_eq!(xp.amount, 2);
        assert!(matches!(
            &xp.source,
            XpGainSource::Action {
                plan_id: "lingtian",
                action: "harvest_manual",
            }
        ));
    }

    #[test]
    fn harvest_drops_seed_when_rng_under_drop_rate() {
        // 先确认：seed=2 的第一 roll < 0.30（Common 掉率），otherwise 测试无意义
        let mut probe = LingtianHarvestRng::new(2);
        let roll = probe.next_f32();
        assert!(roll < 0.30, "seed 2 第一 roll = {roll} 应 < 0.30");

        let mut app = App::new();
        let plant_registry = registry_with_three_test_plants();
        let seeds = SeedRegistry::from_plant_registry(&plant_registry);
        app.insert_resource(ActiveLingtianSessions::new())
            .insert_resource(plant_registry)
            .insert_resource(seeds)
            .insert_resource(registry_with_herb_and_seed_templates())
            .insert_resource(InventoryInstanceIdAllocator::default())
            .insert_resource(LingtianHarvestRng::new(2))
            .insert_resource(ZoneQiAccount::new())
            .insert_resource(LingtianClock::default())
            .add_event::<StartHarvestRequest>()
            .add_event::<HarvestCompleted>()
            .add_event::<StartTillRequest>()
            .add_event::<TillCompleted>()
            .add_event::<StartRenewRequest>()
            .add_event::<RenewCompleted>()
            .add_event::<StartPlantingRequest>()
            .add_event::<PlantingCompleted>()
            .add_event::<StartReplenishRequest>()
            .add_event::<ReplenishCompleted>()
            .add_event::<DyeContaminationWarning>()
            .add_event::<StartDrainQiRequest>()
            .add_event::<DrainQiCompleted>()
            .add_event::<QiTransfer>()
            .add_event::<SkillXpGain>()
            .add_systems(
                Update,
                (
                    handle_start_harvest,
                    tick_lingtian_sessions,
                    apply_completed_sessions,
                )
                    .chain()
                    .after(crate::world::movement_commit::AuthoritativePositionCommitSet),
            );

        let pos = BlockPos::new(0, 64, 0);
        let player = valid_test_player(&mut app, empty_inventory_8x8(), pos);
        spawn_ripe_plot(&mut app, "ci_she_hao", pos);
        app.world_mut().send_event(StartHarvestRequest {
            player,
            pos,
            mode: SessionMode::Manual,
        });
        for _ in 0..HARVEST_MANUAL_TICKS {
            app.update();
        }
        let inv = app.world().get::<PlayerInventory>(player).unwrap();
        assert_eq!(count_in_main_pack(inv, "ci_she_hao"), 1);
        assert_eq!(
            count_in_main_pack(inv, "ci_she_hao_seed"),
            1,
            "RNG roll < 0.3 应掉种子"
        );
    }

    // ------------------------------------------------------------------------
    // P5 补灵 e2e
    // ------------------------------------------------------------------------

    fn spawn_empty_plot(app: &mut App, pos: BlockPos) -> Entity {
        spawn_empty_plot_in_zone(app, pos, "")
    }

    fn spawn_empty_plot_in_zone(app: &mut App, pos: BlockPos, zone: &str) -> Entity {
        let mut p = LingtianPlot::new(pos, None);
        p.zone = zone.to_string();
        p.plot_qi = 0.0;
        // plot_qi_cap 默认 1.0
        app.world_mut().spawn(p).id()
    }

    fn make_inventory_with_bone_coins(coins: u64) -> PlayerInventory {
        let mut inv = empty_inventory_8x8();
        inv.bone_coins = coins;
        inv
    }

    fn make_inventory_with_misc_stack(template_id: &str, stack: u32) -> PlayerInventory {
        let mut inv = empty_inventory_8x8();
        inv.containers[0]
            .items
            .push(crate::inventory::PlacedItemState {
                row: 0,
                col: 0,
                instance: ItemInstance {
                    instance_id: 5000,
                    template_id: template_id.into(),
                    display_name: template_id.into(),
                    grid_w: 1,
                    grid_h: 1,
                    weight: 0.3,
                    rarity: ItemRarity::Common,
                    description: String::new(),
                    stack_count: stack,
                    spirit_quality: 0.7,
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
                },
            });
        inv
    }

    fn make_inventory_with_residue(
        kind: crate::alchemy::residue::PillResidueKind,
        produced_at_tick: u64,
        stack: u32,
    ) -> PlayerInventory {
        let mut inv = make_inventory_with_misc_stack(kind.spec().template_id, stack);
        inv.containers[0].items[0].instance.alchemy = Some(
            crate::alchemy::residue::residue_alchemy_data(kind, produced_at_tick),
        );
        inv
    }

    #[test]
    fn replenish_zone_drains_zone_qi_and_fills_plot() {
        let mut app = build_app();
        // zone qi 充足
        app.world_mut()
            .resource_mut::<ZoneQiAccount>()
            .set(DEFAULT_ZONE, 5.0);
        let pos = BlockPos::new(0, 64, 0);
        let player = valid_test_player(&mut app, empty_inventory_8x8(), pos);
        let plot = spawn_empty_plot(&mut app, pos);
        app.world_mut().send_event(StartReplenishRequest {
            player,
            pos,
            source: ReplenishSource::Zone,
        });
        for _ in 0..ReplenishSource::Zone.duration_ticks() {
            app.update();
        }
        assert!(app.world().resource::<ActiveLingtianSessions>().is_empty());
        let p = app.world().get::<LingtianPlot>(plot).unwrap();
        assert!((p.plot_qi - 0.5).abs() < 1e-6, "plot_qi 应 +0.5");
        let z = app.world().resource::<ZoneQiAccount>().get(DEFAULT_ZONE);
        assert!((z - 4.5).abs() < 1e-6, "zone qi 应 -0.5");
    }

    #[test]
    fn replenish_zone_uses_non_default_plot_zone_for_precheck_and_debit() {
        let mut app = build_app();
        app.world_mut()
            .resource_mut::<ZoneQiAccount>()
            .set(DEFAULT_ZONE, 0.0);
        app.world_mut()
            .resource_mut::<ZoneQiAccount>()
            .set("blood_valley", 5.0);
        let pos = BlockPos::new(8, 64, 8);
        let player = valid_test_player(&mut app, empty_inventory_8x8(), pos);
        let plot = spawn_empty_plot_in_zone(&mut app, pos, "blood_valley");

        app.world_mut().send_event(StartReplenishRequest {
            player,
            pos,
            source: ReplenishSource::Zone,
        });
        for _ in 0..ReplenishSource::Zone.duration_ticks() {
            app.update();
        }

        assert!(
            app.world().resource::<ActiveLingtianSessions>().is_empty(),
            "blood_valley 余额充足时应允许非默认区补灵，不能被 default=0 拦截"
        );
        let p = app.world().get::<LingtianPlot>(plot).unwrap();
        assert!((p.plot_qi - 0.5).abs() < 1e-6, "plot_qi 应 +0.5");
        let accounts = app.world().resource::<ZoneQiAccount>();
        assert!(
            (accounts.get("blood_valley") - 4.5).abs() < 1e-6,
            "补灵应扣 blood_valley，而不是 default"
        );
        assert_eq!(accounts.get(DEFAULT_ZONE), 0.0, "default zone 不应被扣款");
    }

    /// plan-zone-qi-economy-v1 P2：地板红线——zone qi 不足以支付 `plot_qi_amount()`
    /// 且留住 `QI_NPC_ABSORB_FLOOR` 底仓时，StartReplenishRequest 必须被材料检查直接
    /// 拒绝（不开 session），不能像修 P0 之前那样把 zone 抽穿地板。
    #[test]
    fn replenish_zone_rejected_when_zone_qi_insufficient_to_cover_floor() {
        let mut app = build_app();
        // amount=0.5，floor=0.3 → 需要 >= 0.8 才允许；这里只给 0.79（差一点点）。
        app.world_mut()
            .resource_mut::<ZoneQiAccount>()
            .set(DEFAULT_ZONE, 0.79);
        let pos = BlockPos::new(0, 64, 0);
        let player = valid_test_player(&mut app, empty_inventory_8x8(), pos);
        spawn_empty_plot(&mut app, pos);
        app.world_mut().send_event(StartReplenishRequest {
            player,
            pos,
            source: ReplenishSource::Zone,
        });
        app.update();
        assert!(
            app.world().resource::<ActiveLingtianSessions>().is_empty(),
            "zone_qi=0.79 不足以支付 amount(0.5)+floor(0.3)=0.8，应被材料检查拒绝，不应开 session"
        );
        // zone qi 分毫未动（连 session 都没开）。
        let z = app.world().resource::<ZoneQiAccount>().get(DEFAULT_ZONE);
        assert!((z - 0.79).abs() < 1e-6, "被拒绝的请求不应触碰 zone qi");
    }

    /// 边界回归：zone qi 恰好等于 amount+floor（0.8）时应被允许，补灵后 zone 恰好
    /// 停在地板（0.3），不多不少。
    #[test]
    fn replenish_zone_allowed_exactly_at_floor_boundary() {
        let mut app = build_app();
        app.world_mut()
            .resource_mut::<ZoneQiAccount>()
            .set(DEFAULT_ZONE, 0.8);
        let pos = BlockPos::new(0, 64, 0);
        let player = valid_test_player(&mut app, empty_inventory_8x8(), pos);
        let plot = spawn_empty_plot(&mut app, pos);
        app.world_mut().send_event(StartReplenishRequest {
            player,
            pos,
            source: ReplenishSource::Zone,
        });
        for _ in 0..ReplenishSource::Zone.duration_ticks() {
            app.update();
        }
        assert!(
            app.world().resource::<ActiveLingtianSessions>().is_empty(),
            "zone_qi=0.8 恰好等于 amount+floor，应被允许并正常完成"
        );
        let p = app.world().get::<LingtianPlot>(plot).unwrap();
        assert!((p.plot_qi - 0.5).abs() < 1e-6, "plot_qi 应 +0.5");
        let z = app.world().resource::<ZoneQiAccount>().get(DEFAULT_ZONE);
        assert!(
            (z - 0.3).abs() < 1e-6,
            "zone qi 补灵后应恰好停在地板 0.3，实际 {z}"
        );
    }

    #[test]
    fn replenish_bone_coin_consumes_one_coin_and_adds_0_8() {
        let mut app = build_app();
        let pos = BlockPos::new(0, 64, 0);
        let player = valid_test_player(&mut app, make_inventory_with_bone_coins(3), pos);
        let plot = spawn_empty_plot(&mut app, pos);
        app.world_mut().send_event(StartReplenishRequest {
            player,
            pos,
            source: ReplenishSource::BoneCoin,
        });
        for _ in 0..ReplenishSource::BoneCoin.duration_ticks() {
            app.update();
        }
        let p = app.world().get::<LingtianPlot>(plot).unwrap();
        assert!((p.plot_qi - 0.8).abs() < 1e-6);
        let inv = app.world().get::<PlayerInventory>(player).unwrap();
        assert_eq!(inv.bone_coins, 2);
    }

    #[test]
    fn replenish_beast_core_overflows_to_zone_when_full() {
        let mut app = build_app();
        let pos = BlockPos::new(0, 64, 0);
        let player = valid_test_player(
            &mut app,
            make_inventory_with_misc_stack("mutant_beast_core", 1),
            pos,
        );
        let plot = spawn_empty_plot(&mut app, pos);
        // plot_qi 已经在 0.5/1.0 → 注 2.0 → +0.5 满，溢出 1.5 回 zone
        app.world_mut()
            .get_mut::<LingtianPlot>(plot)
            .unwrap()
            .plot_qi = 0.5;
        let zone_before = app.world().resource::<ZoneQiAccount>().get(DEFAULT_ZONE);
        app.world_mut().send_event(StartReplenishRequest {
            player,
            pos,
            source: ReplenishSource::BeastCore,
        });
        for _ in 0..ReplenishSource::BeastCore.duration_ticks() {
            app.update();
        }
        let p = app.world().get::<LingtianPlot>(plot).unwrap();
        assert!((p.plot_qi - 1.0).abs() < 1e-6, "plot_qi 拉满 1.0");
        let zone_after = app.world().resource::<ZoneQiAccount>().get(DEFAULT_ZONE);
        assert!(
            (zone_after - zone_before - 1.5).abs() < 1e-6,
            "1.5 应回馈 zone"
        );
        // 兽核应被消耗（从背包移除）
        let inv = app.world().get::<PlayerInventory>(player).unwrap();
        assert_eq!(count_in_main_pack(inv, "mutant_beast_core"), 0);
    }

    #[test]
    fn replenish_overflow_returns_to_non_default_plot_zone() {
        let mut app = build_app();
        app.world_mut()
            .resource_mut::<ZoneQiAccount>()
            .set(DEFAULT_ZONE, 0.0);
        app.world_mut()
            .resource_mut::<ZoneQiAccount>()
            .set("north_wastes", 1.0);
        let pos = BlockPos::new(9, 64, 9);
        let player = valid_test_player(
            &mut app,
            make_inventory_with_misc_stack("mutant_beast_core", 1),
            pos,
        );
        let plot = spawn_empty_plot_in_zone(&mut app, pos, "north_wastes");
        app.world_mut()
            .get_mut::<LingtianPlot>(plot)
            .unwrap()
            .plot_qi = 0.5;

        app.world_mut().send_event(StartReplenishRequest {
            player,
            pos,
            source: ReplenishSource::BeastCore,
        });
        for _ in 0..ReplenishSource::BeastCore.duration_ticks() {
            app.update();
        }

        let accounts = app.world().resource::<ZoneQiAccount>();
        assert!(
            (accounts.get("north_wastes") - 2.5).abs() < 1e-6,
            "1.5 overflow 应回流 north_wastes，实际 {}",
            accounts.get("north_wastes")
        );
        assert_eq!(
            accounts.get(DEFAULT_ZONE),
            0.0,
            "非默认区 overflow 不应串到 default"
        );
    }

    #[test]
    fn replenish_ling_shui_consumes_one_bottle() {
        let mut app = build_app();
        let pos = BlockPos::new(0, 64, 0);
        let player = valid_test_player(
            &mut app,
            make_inventory_with_misc_stack("ling_shui", 2),
            pos,
        );
        let plot = spawn_empty_plot(&mut app, pos);
        app.world_mut().send_event(StartReplenishRequest {
            player,
            pos,
            source: ReplenishSource::LingShui,
        });
        for _ in 0..ReplenishSource::LingShui.duration_ticks() {
            app.update();
        }
        let p = app.world().get::<LingtianPlot>(plot).unwrap();
        assert!((p.plot_qi - 0.3).abs() < 1e-6);
        let inv = app.world().get::<PlayerInventory>(player).unwrap();
        assert_eq!(count_in_main_pack(inv, "ling_shui"), 1);
    }

    #[test]
    fn all_pill_residue_kinds_consume_stack_and_apply_spec_effects() {
        for kind in [
            crate::alchemy::residue::PillResidueKind::FailedPill,
            crate::alchemy::residue::PillResidueKind::FlawedPill,
            crate::alchemy::residue::PillResidueKind::ProcessingDregs,
            crate::alchemy::residue::PillResidueKind::AgingScraps,
        ] {
            let mut app = build_app();
            app.world_mut()
                .insert_resource(LingtianHarvestRng::new(343));
            let pos = BlockPos::new(0, 64, 0);
            let player = valid_test_player(&mut app, make_inventory_with_residue(kind, 0, 1), pos);
            let plot = spawn_empty_plot(&mut app, pos);
            app.world_mut().send_event(StartReplenishRequest {
                player,
                pos,
                source: ReplenishSource::PillResidue { residue_kind: kind },
            });
            let duration = (ReplenishSource::PillResidue { residue_kind: kind }).duration_ticks();
            for _ in 0..duration {
                app.update();
            }

            let spec = kind.spec();
            let p = app.world().get::<LingtianPlot>(plot).unwrap();
            assert!(
                (p.plot_qi - spec.plot_qi_amount).abs() < 1e-6,
                "{kind:?} should add plot_qi per spec"
            );
            assert!(
                (p.dye_contamination - spec.contamination_delta).abs() < 1e-6,
                "{kind:?} should add contamination per spec when roll hits"
            );
            let inv = app.world().get::<PlayerInventory>(player).unwrap();
            assert_eq!(count_in_main_pack(inv, spec.template_id), 0);
        }
    }

    #[test]
    fn residue_contamination_warning_records_world_state_event() {
        let mut app = build_app();
        app.world_mut().insert_resource(LingtianHarvestRng::new(2));
        app.world_mut().resource_mut::<CombatClock>().tick = 987;
        let kind = crate::alchemy::residue::PillResidueKind::FailedPill;
        let pos = BlockPos::new(0, 64, 0);
        let player = valid_test_player(
            &mut app,
            (
                Username("Azure".to_string()),
                make_inventory_with_residue(kind, 0, 1),
            ),
            pos,
        );
        let plot = spawn_empty_plot_in_zone(&mut app, pos, "lingquan_marsh");
        app.world_mut()
            .get_mut::<LingtianPlot>(plot)
            .unwrap()
            .dye_contamination = 0.25;

        app.world_mut().send_event(StartReplenishRequest {
            player,
            pos,
            source: ReplenishSource::PillResidue { residue_kind: kind },
        });
        let duration = (ReplenishSource::PillResidue { residue_kind: kind }).duration_ticks();
        for _ in 0..duration {
            app.update();
        }

        let events = app
            .world()
            .resource::<ActiveEventsResource>()
            .recent_events_snapshot();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, GameEventType::EventTriggered);
        assert_eq!(
            events[0].target.as_deref(),
            Some("lingtian_plot_dye_contamination_warning")
        );
        assert_eq!(events[0].zone.as_deref(), Some("lingquan_marsh"));
        assert_eq!(events[0].tick, 987);
        assert_eq!(events[0].player.as_deref(), Some("offline:Azure"));
        assert_eq!(
            events[0]
                .details
                .as_ref()
                .and_then(|details| details.get("source")),
            Some(&serde_json::json!("pill_residue_failed_pill"))
        );
    }

    #[test]
    fn replenish_rejects_expired_residue() {
        let mut app = build_app();
        let kind = crate::alchemy::residue::PillResidueKind::FailedPill;
        let pos = BlockPos::new(0, 64, 0);
        let player = valid_test_player(&mut app, make_inventory_with_residue(kind, 10, 1), pos);
        app.world_mut().resource_mut::<CombatClock>().tick =
            10 + crate::alchemy::residue::PILL_RESIDUE_TTL_TICKS;
        spawn_empty_plot(&mut app, pos);
        app.world_mut().send_event(StartReplenishRequest {
            player,
            pos,
            source: ReplenishSource::PillResidue { residue_kind: kind },
        });
        app.update();

        assert!(app.world().resource::<ActiveLingtianSessions>().is_empty());
        let inv = app.world().get::<PlayerInventory>(player).unwrap();
        assert_eq!(count_in_main_pack(inv, kind.spec().template_id), 1);
    }

    #[test]
    fn residue_now_tick_prefers_combat_clock_over_lingtian_clock() {
        let combat_clock = CombatClock { tick: 123 };
        let lingtian_clock = LingtianClock {
            lingtian_tick: 99_999,
        };

        assert_eq!(residue_now_tick(Some(&combat_clock), &lingtian_clock), 123);
    }

    #[test]
    fn replenish_rejected_when_no_material() {
        let mut app = build_app();
        // bone_coins=0，请求 BoneCoin → 拒
        let pos = BlockPos::new(0, 64, 0);
        let player = valid_test_player(&mut app, empty_inventory_8x8(), pos);
        spawn_empty_plot(&mut app, pos);
        app.world_mut().send_event(StartReplenishRequest {
            player,
            pos,
            source: ReplenishSource::BoneCoin,
        });
        app.update();
        assert!(app.world().resource::<ActiveLingtianSessions>().is_empty());
    }

    #[test]
    fn replenish_rejected_when_in_cooldown() {
        let mut app = build_app();
        let pos = BlockPos::new(0, 64, 0);
        let player = valid_test_player(&mut app, make_inventory_with_bone_coins(2), pos);
        let plot = spawn_empty_plot(&mut app, pos);
        // 模拟"刚补过" — last_replenish_at 设到当前 clock
        app.world_mut()
            .resource_mut::<LingtianClock>()
            .lingtian_tick = 1000;
        app.world_mut()
            .get_mut::<LingtianPlot>(plot)
            .unwrap()
            .last_replenish_at = 1000;

        app.world_mut().send_event(StartReplenishRequest {
            player,
            pos,
            source: ReplenishSource::BoneCoin,
        });
        app.update();
        assert!(app.world().resource::<ActiveLingtianSessions>().is_empty());
        // 骨币没扣
        let inv = app.world().get::<PlayerInventory>(player).unwrap();
        assert_eq!(inv.bone_coins, 2);
    }

    #[test]
    fn replenish_allowed_after_cooldown_expires() {
        let mut app = build_app();
        let pos = BlockPos::new(0, 64, 0);
        let player = valid_test_player(&mut app, make_inventory_with_bone_coins(2), pos);
        let plot = spawn_empty_plot(&mut app, pos);
        app.world_mut()
            .resource_mut::<LingtianClock>()
            .lingtian_tick = REPLENISH_COOLDOWN_LINGTIAN_TICKS + 100;
        app.world_mut()
            .get_mut::<LingtianPlot>(plot)
            .unwrap()
            .last_replenish_at = 50; // 距今 4370 lingtian-tick > 4320 冷却
        app.world_mut().send_event(StartReplenishRequest {
            player,
            pos,
            source: ReplenishSource::BoneCoin,
        });
        for _ in 0..ReplenishSource::BoneCoin.duration_ticks() {
            app.update();
        }
        // 应已结算
        assert!(app.world().resource::<ActiveLingtianSessions>().is_empty());
        let p = app.world().get::<LingtianPlot>(plot).unwrap();
        assert!((p.plot_qi - 0.8).abs() < 1e-6);
    }

    // ------------------------------------------------------------------------
    // P2 plot_qi_cap 修饰 e2e
    // ------------------------------------------------------------------------

    #[test]
    fn till_with_combined_environment_yields_cap_2_8() {
        let mut app = build_app();
        let player = spawn_test_player(&mut app, make_inventory_with_hoe(HoeKind::Iron, 1.0));
        app.world_mut().send_event(StartTillRequest {
            player,
            pos: BlockPos::new(0, 64, 0),
            hoe_instance_id: 1,
            mode: SessionMode::Manual,
            terrain: TerrainKind::Grass,
            // 用 SummerToWinter（汐转期 modifier=0）保留 plan-lingtian-v1 的
            // "三大基础修饰 → cap 2.8" 锁定，避免与 plan-lingtian-weather-v1 §2
            // 夏散 -0.2 / 冬聚 +0.2 的物理修饰交织。
            environment: PlotEnvironment {
                water_adjacent: true,
                biome: PlotBiome::Wetland,
                zhenfa_lingju_tier: PlotLingjuTier::Full,
                season: Season::SummerToWinter,
                active_weather: None,
            },
        });
        for _ in 0..TILL_MANUAL_TICKS {
            app.update();
        }
        let plot = app
            .world_mut()
            .query::<&LingtianPlot>()
            .iter(app.world())
            .next()
            .unwrap();
        // 1.0 + 0.3 + 0.5 + 1.0 = 2.8
        assert!((plot.plot_qi_cap - 2.8).abs() < 1e-6);
    }

    #[test]
    fn till_default_summer_environment_yields_cap_0_8() {
        // plan-lingtian-weather-v1 §2 — `PlotEnvironment::base()` 默认 Summer
        // (-0.2 modifier)，所以裸开垦的 plot_qi_cap = 1.0 - 0.2 = 0.8。
        let mut app = build_app();
        let player = spawn_test_player(&mut app, make_inventory_with_hoe(HoeKind::Iron, 1.0));
        app.world_mut().send_event(StartTillRequest {
            player,
            pos: BlockPos::new(0, 64, 0),
            hoe_instance_id: 1,
            mode: SessionMode::Manual,
            terrain: TerrainKind::Grass,
            environment: PlotEnvironment::base(),
        });
        for _ in 0..TILL_MANUAL_TICKS {
            app.update();
        }
        let plot = app
            .world_mut()
            .query::<&LingtianPlot>()
            .iter(app.world())
            .next()
            .unwrap();
        assert!(
            (plot.plot_qi_cap - 0.8).abs() < 1e-6,
            "Summer base 应当 0.8（=1.0 - 0.2 summer），实际 {}",
            plot.plot_qi_cap
        );
    }

    #[test]
    fn till_xizhuan_environment_keeps_cap_at_1_0() {
        // 汐转期 modifier=0，plot_qi_cap 锁回 plan-lingtian-v1 的 1.0 基线。
        let mut app = build_app();
        let player = spawn_test_player(&mut app, make_inventory_with_hoe(HoeKind::Iron, 1.0));
        app.world_mut().send_event(StartTillRequest {
            player,
            pos: BlockPos::new(0, 64, 0),
            hoe_instance_id: 1,
            mode: SessionMode::Manual,
            terrain: TerrainKind::Grass,
            environment: PlotEnvironment {
                season: Season::SummerToWinter,
                ..PlotEnvironment::base()
            },
        });
        for _ in 0..TILL_MANUAL_TICKS {
            app.update();
        }
        let plot = app
            .world_mut()
            .query::<&LingtianPlot>()
            .iter(app.world())
            .next()
            .unwrap();
        assert!((plot.plot_qi_cap - 1.0).abs() < 1e-6);
    }

    // ------------------------------------------------------------------------
    // §5.1 密度阈值 e2e
    // ------------------------------------------------------------------------

    use crate::lingtian::pressure::{
        PressureLevel as PL, PRESSURE_HIGH, PRESSURE_LOW, PRESSURE_MID,
    };

    fn build_pressure_app(natural_supply: f32) -> App {
        // 默认 pin 在 Summer（plan §2 的"夏散" 物理常态）：natural_supply -10%，
        // amplitude=0 → jitter 不影响（可重现）。原 plan-lingtian-v1 §5.1 测试
        // 大多用 natural_supply=0，0 × 任何系数仍是 0，不受影响；只有
        // `natural_supply_offsets_demand` 的断言因夏 -10% 调整。
        build_pressure_app_with_season(natural_supply, Season::Summer)
    }

    fn build_pressure_app_with_season(natural_supply: f32, season: Season) -> App {
        let mut app = App::new();
        let mut tracker = ZonePressureTracker::new();
        tracker.set_natural_supply(DEFAULT_ZONE, natural_supply);
        let mut plant_registry = PlantKindRegistry::new();
        plant_registry
            .insert(PlantKind {
                id: "ling_mu_miao".into(),
                display_name: "灵木苗".into(),
                cultivable: true,
                growth_cost: GrowthCost::High, // 0.012 / tick
                growth_duration_ticks: 28800,
                rarity: PlantRarity::Rare,
                description: String::new(),
            })
            .unwrap();
        // 显式 pin 季节状态：测试可重现，不受默认 query_season 影响。
        // 用 Default + 字段覆写避开 `tick_offset` 私有字段限制（cross-module）。
        let mut season_state = crate::world::season::WorldSeasonState::default();
        season_state.current = crate::world::season::SeasonState {
            season,
            tick_into_phase: 0,
            phase_total_ticks: season.phase_total_ticks(),
            year_index: 0,
        };
        season_state.last_phase_change_tick = 0;

        app.insert_resource(LingtianTickAccumulator::new())
            .insert_resource(LingtianClock::default())
            .insert_resource(ZoneQiAccount::new())
            .insert_resource(plant_registry)
            .insert_resource(tracker)
            .insert_resource(season_state)
            .add_event::<ReplenishCompleted>()
            .add_event::<StartDrainQiRequest>()
            .add_event::<DrainQiCompleted>()
            .add_event::<QiTransfer>()
            .add_event::<ZonePressureCrossed>()
            .add_systems(
                Update,
                (
                    lingtian_growth_tick,
                    record_replenish_to_pressure,
                    compute_zone_pressure_system,
                )
                    .chain(),
            );
        app
    }

    fn spawn_high_cost_planted(app: &mut App, n: u32) {
        spawn_high_cost_planted_with_owner(app, n, None);
    }

    fn spawn_high_cost_planted_with_owner(app: &mut App, n: u32, owner: Option<Entity>) {
        spawn_high_cost_planted_with_owner_in_zone(app, n, owner, "");
    }

    fn spawn_high_cost_planted_in_zone(app: &mut App, n: u32, zone: &str) {
        spawn_high_cost_planted_with_owner_in_zone(app, n, None, zone);
    }

    fn spawn_high_cost_planted_with_owner_in_zone(
        app: &mut App,
        n: u32,
        owner: Option<Entity>,
        zone: &str,
    ) {
        for i in 0..n {
            let mut p = LingtianPlot::new(BlockPos::new(i as i32, 64, 0), owner);
            p.zone = zone.to_string();
            p.plot_qi = 1.0;
            p.crop = Some(CropInstance::new("ling_mu_miao".into()));
            app.world_mut().spawn(p);
        }
    }

    fn step_one_lingtian_tick(app: &mut App) {
        for _ in 0..BEVY_TICKS_PER_LINGTIAN_TICK {
            app.update();
        }
    }

    fn collect_pressure_events(app: &mut App) -> Vec<(PL, f32)> {
        let world = app.world_mut();
        let events = world.resource::<bevy_ecs::event::Events<ZonePressureCrossed>>();
        let mut reader = events.get_reader();
        reader
            .read(events)
            .map(|e| (e.level, e.raw_pressure))
            .collect()
    }

    fn collect_pressure_event_zones(app: &mut App) -> Vec<(String, PL, f32)> {
        let world = app.world_mut();
        let events = world.resource::<bevy_ecs::event::Events<ZonePressureCrossed>>();
        let mut reader = events.get_reader();
        reader
            .read(events)
            .map(|e| (e.zone.clone(), e.level, e.raw_pressure))
            .collect()
    }

    #[test]
    fn no_event_when_pressure_below_low() {
        let mut app = build_pressure_app(0.0);
        spawn_high_cost_planted(&mut app, 1);
        step_one_lingtian_tick(&mut app);
        assert!(collect_pressure_events(&mut app).is_empty());
        let tracker = app.world().resource::<ZonePressureTracker>();
        assert_eq!(
            tracker.state(DEFAULT_ZONE).map(|s| s.last_level),
            Some(PL::None)
        );
    }

    #[test]
    fn rises_through_low_mid_high_with_increasing_plot_count() {
        let mut app = build_pressure_app(0.0);
        // demand 0.012 × N。f32 累加噪音 ~1e-7：用整除留 5% 余量
        // LOW: 26 × 0.012 ≈ 0.312
        spawn_high_cost_planted(&mut app, 26);
        step_one_lingtian_tick(&mut app);
        let evts = collect_pressure_events(&mut app);
        assert_eq!(evts.len(), 1);
        assert_eq!(evts[0].0, PL::Low);

        // 加到 51（demand ≈ 0.612 → MID）
        spawn_high_cost_planted(&mut app, 25);
        step_one_lingtian_tick(&mut app);
        let evts = collect_pressure_events(&mut app);
        assert_eq!(evts.last().map(|(l, _)| *l), Some(PL::Mid));

        // 加到 85（demand ≈ 1.020 → HIGH）
        spawn_high_cost_planted(&mut app, 34);
        step_one_lingtian_tick(&mut app);
        let evts = collect_pressure_events(&mut app);
        assert_eq!(evts.last().map(|(l, _)| *l), Some(PL::High));
    }

    #[test]
    fn high_pressure_clears_zone_plot_qi() {
        let mut app = build_pressure_app(0.0);
        spawn_high_cost_planted(&mut app, 100); // demand ~1.2 → HIGH
        step_one_lingtian_tick(&mut app);
        let any_nonzero = app
            .world_mut()
            .query::<&LingtianPlot>()
            .iter(app.world())
            .any(|p| p.plot_qi > 0.0);
        assert!(!any_nonzero, "HIGH 应清掉所有 plot_qi");
    }

    #[test]
    fn high_pressure_clears_only_matching_non_default_zone() {
        let mut app = build_pressure_app(0.0);
        spawn_high_cost_planted(&mut app, 1);
        spawn_high_cost_planted_in_zone(&mut app, 100, "blood_valley");

        step_one_lingtian_tick(&mut app);

        let tracker = app.world().resource::<ZonePressureTracker>();
        assert_eq!(
            tracker.state("blood_valley").unwrap().last_level,
            PL::High,
            "blood_valley 自身 demand 应触发 HIGH"
        );
        assert_eq!(
            tracker.state(DEFAULT_ZONE).unwrap().last_level,
            PL::None,
            "default 只有 1 个 plot，不应被 blood_valley 串账抬压"
        );
        let events = collect_pressure_event_zones(&mut app);
        assert!(
            events
                .iter()
                .any(|(zone, level, _)| zone == "blood_valley" && *level == PL::High),
            "应发 blood_valley 的 HIGH 事件，实际 {events:?}"
        );
        let (default_nonzero, remote_nonzero) = {
            let mut query = app.world_mut().query::<&LingtianPlot>();
            let mut default_nonzero = false;
            let mut remote_nonzero = false;
            for plot in query.iter(app.world()) {
                if plot_zone_key(plot) == DEFAULT_ZONE && plot.plot_qi > 0.0 {
                    default_nonzero = true;
                }
                if plot_zone_key(plot) == "blood_valley" && plot.plot_qi > 0.0 {
                    remote_nonzero = true;
                }
            }
            (default_nonzero, remote_nonzero)
        };
        assert!(default_nonzero, "default plot_qi 不应被非默认区 HIGH 清掉");
        assert!(!remote_nonzero, "blood_valley HIGH 应清掉本区 plot_qi");
    }

    #[test]
    fn npc_owned_plots_count_toward_zone_pressure() {
        let mut app = build_pressure_app(0.0);
        let npc = app.world_mut().spawn(NpcMarker).id();
        spawn_high_cost_planted_with_owner(&mut app, 85, Some(npc)); // demand ~1.02 → HIGH
        step_one_lingtian_tick(&mut app);

        let tracker = app.world().resource::<ZonePressureTracker>();
        assert_eq!(
            tracker.state(DEFAULT_ZONE).unwrap().last_level,
            PL::High,
            "ZonePressureTracker 应统计 NPC owner 的灵田，而不是只统计玩家灵田"
        );
    }

    #[test]
    fn natural_supply_offsets_demand_in_summer() {
        // plan-lingtian-weather-v1 §2 — Summer natural_supply -10%：
        // base 0.5 × 0.9 = 0.45 effective；demand 0.6（50 × 0.012/tick）；
        // pressure = 0.6 - 0.45 = 0.15（仍在 LOW 阈值 0.3 以下 → None）。
        let mut app = build_pressure_app(0.5);
        spawn_high_cost_planted(&mut app, 50);
        step_one_lingtian_tick(&mut app);
        let tracker = app.world().resource::<ZonePressureTracker>();
        let p = tracker.state(DEFAULT_ZONE).unwrap().last_pressure;
        assert!(
            (p - 0.15).abs() < 1e-3,
            "summer natural_supply offset 应当 0.15，实际 {p}"
        );
        assert_eq!(tracker.state(DEFAULT_ZONE).unwrap().last_level, PL::None);
    }

    #[test]
    fn natural_supply_offsets_demand_in_winter_extra_supply() {
        // plan-lingtian-weather-v1 §2 — Winter natural_supply +10%：
        // base 0.5 × 1.1 = 0.55 effective；demand 0.6；pressure = 0.05。
        let mut app = build_pressure_app_with_season(0.5, Season::Winter);
        spawn_high_cost_planted(&mut app, 50);
        step_one_lingtian_tick(&mut app);
        let tracker = app.world().resource::<ZonePressureTracker>();
        let p = tracker.state(DEFAULT_ZONE).unwrap().last_pressure;
        assert!(
            (p - 0.05).abs() < 1e-3,
            "winter natural_supply offset 应当 0.05，实际 {p}"
        );
    }

    #[test]
    fn haze_active_relaxes_pressure_classification_by_one_tier() {
        // plan-lingtian-weather-v1 §5 / worldview §七 — 阴霾期间天道注视减弱，
        // 阈值降 1 档。
        // setup：冬季（Blizzard / Haze 可触发 zone），HeavyHaze active；
        // 灌满 100 个 high_cost 作物（demand = 100 × 0.012 = 1.2 → 原 raw=1.2 → HIGH）
        // 但 haze 阈值降 1 档 → 应被分类为 Mid。
        let mut app = build_pressure_app_with_season(0.0, Season::Winter);
        // 注入 HeavyHaze active weather（覆盖 ActiveWeather 默认空状态）
        let mut active_weather = crate::lingtian::weather::ActiveWeather::new();
        active_weather.insert(
            DEFAULT_ZONE,
            crate::lingtian::weather::WeatherEvent::HeavyHaze,
            0,      // started_at
            10_000, // expires_at（远期，本测期间不清）
        );
        app.insert_resource(active_weather);

        spawn_high_cost_planted(&mut app, 100); // demand 1.2 → raw HIGH
        step_one_lingtian_tick(&mut app);
        let tracker = app.world().resource::<ZonePressureTracker>();
        let s = tracker.state(DEFAULT_ZONE).unwrap();
        // raw pressure 应该 ≈ 1.2（natural_supply=0、winter +10% 不影响 0）
        assert!(
            (s.last_pressure - 1.2).abs() < 1e-3,
            "raw pressure ≈ 1.2，实际 {}",
            s.last_pressure
        );
        // 但 classified 档位应当被降为 Mid（1 档）而非 High
        assert_eq!(
            s.last_level,
            PL::Mid,
            "阴霾期间 raw=1.2 应被降为 Mid，实际 {:?}",
            s.last_level
        );
    }

    #[test]
    fn non_default_zone_pressure_uses_its_own_weather() {
        let mut app = build_pressure_app_with_season(0.0, Season::Winter);
        let mut active_weather = crate::lingtian::weather::ActiveWeather::new();
        active_weather.insert(
            "blood_valley",
            crate::lingtian::weather::WeatherEvent::HeavyHaze,
            0,
            10_000,
        );
        app.insert_resource(active_weather);

        spawn_high_cost_planted_in_zone(&mut app, 100, "blood_valley");
        step_one_lingtian_tick(&mut app);

        let tracker = app.world().resource::<ZonePressureTracker>();
        let s = tracker.state("blood_valley").unwrap();
        assert!(
            (s.last_pressure - 1.2).abs() < 1e-3,
            "blood_valley raw pressure ≈1.2，实际 {}",
            s.last_pressure
        );
        assert_eq!(
            s.last_level,
            PL::Mid,
            "blood_valley 的 HeavyHaze 应把 raw HIGH 降为 Mid，实际 {:?}",
            s.last_level
        );
        assert!(
            tracker.state(DEFAULT_ZONE).is_none()
                || tracker.state(DEFAULT_ZONE).unwrap().last_level == PL::None,
            "非默认区压力不应写入 default"
        );
    }

    #[test]
    fn no_haze_no_relax_pressure_classified_normally() {
        // 对照：相同 raw pressure 但无 haze → classified 仍为 High。
        let mut app = build_pressure_app_with_season(0.0, Season::Winter);
        spawn_high_cost_planted(&mut app, 100);
        step_one_lingtian_tick(&mut app);
        let tracker = app.world().resource::<ZonePressureTracker>();
        let s = tracker.state(DEFAULT_ZONE).unwrap();
        assert!((s.last_pressure - 1.2).abs() < 1e-3);
        assert_eq!(
            s.last_level,
            PL::High,
            "无阴霾时 raw=1.2 应当 High，实际 {:?}",
            s.last_level
        );
    }

    #[test]
    fn replenish_recent_7d_offsets_demand() {
        let mut app = build_pressure_app(0.0);
        spawn_high_cost_planted(&mut app, 50); // demand 0.6 → MID
        app.world_mut()
            .resource_mut::<ZonePressureTracker>()
            .state_mut(DEFAULT_ZONE)
            .record_replenish(0, 0.5);
        step_one_lingtian_tick(&mut app);
        let tracker = app.world().resource::<ZonePressureTracker>();
        assert_eq!(tracker.state(DEFAULT_ZONE).unwrap().last_level, PL::None);
    }

    #[test]
    fn replenish_pressure_record_uses_plot_zone() {
        let mut app = build_pressure_app(0.0);
        let pos = BlockPos::new(12, 64, 12);
        spawn_empty_plot_in_zone(&mut app, pos, "lingquan_marsh");
        let player = app.world_mut().spawn_empty().id();

        app.world_mut().send_event(ReplenishCompleted {
            player,
            pos,
            source: ReplenishSource::BoneCoin,
            plot_qi_added: 0.8,
            overflow_to_zone: 0.2,
        });
        app.update();

        let tracker = app.world().resource::<ZonePressureTracker>();
        assert!(
            (tracker
                .state("lingquan_marsh")
                .unwrap()
                .replenish_total_7d()
                - 1.0)
                .abs()
                < 1e-6,
            "补灵压力应记录到 lingquan_marsh"
        );
        assert!(
            tracker.state(DEFAULT_ZONE).is_none()
                || tracker
                    .state(DEFAULT_ZONE)
                    .unwrap()
                    .replenish_total_7d()
                    .abs()
                    < 1e-6,
            "非默认区补灵压力不应串到 default"
        );
    }

    #[test]
    fn no_duplicate_event_when_pressure_stays_at_same_level() {
        let mut app = build_pressure_app(0.0);
        spawn_high_cost_planted(&mut app, 26); // LOW (>= 0.30 with f32 margin)
        step_one_lingtian_tick(&mut app);
        let evts1 = collect_pressure_events(&mut app);
        assert_eq!(evts1.len(), 1);
        step_one_lingtian_tick(&mut app);
        let evts2 = collect_pressure_events(&mut app);
        assert!(evts2.is_empty(), "档位未上升不该重复发");
    }

    #[test]
    fn thresholds_match_plan_constants() {
        assert!((PRESSURE_LOW - 0.3).abs() < 1e-6);
        assert!((PRESSURE_MID - 0.6).abs() < 1e-6);
        assert!((PRESSURE_HIGH - 1.0).abs() < 1e-6);
    }

    // ------------------------------------------------------------------------
    // §1.7 偷菜匿名记账 e2e
    // ------------------------------------------------------------------------

    use crate::cultivation::life_record::{BiographyEntry as BE, LifeRecord};

    fn count_biography_matching<F: Fn(&BE) -> bool>(lr: &LifeRecord, f: F) -> usize {
        lr.biography.iter().filter(|e| f(e)).count()
    }

    /// build_harvest_app 已有；本 helper 在它基础上同时挂 LifeRecord 给 owner / operator
    fn spawn_player_with_lifelog(app: &mut App, character_id: &str, target: BlockPos) -> Entity {
        let inv = empty_inventory_8x8();
        let lr = LifeRecord::new(character_id);
        valid_test_player(app, (inv, lr), target)
    }

    fn spawn_owned_ripe_plot(
        app: &mut App,
        plant_id: &str,
        pos: BlockPos,
        owner: Option<Entity>,
    ) -> Entity {
        let mut p = LingtianPlot::new(pos, owner);
        let mut crop = CropInstance::new(plant_id.into());
        crop.growth = 1.0;
        p.crop = Some(crop);
        app.world_mut().spawn(p).id()
    }

    #[test]
    fn self_harvest_records_no_steal_entries() {
        let mut app = build_harvest_app();
        let pos = BlockPos::new(0, 64, 0);
        let player = spawn_player_with_lifelog(&mut app, "alice", pos);
        spawn_owned_ripe_plot(&mut app, "ci_she_hao", pos, Some(player));
        app.world_mut().send_event(StartHarvestRequest {
            player,
            pos,
            mode: SessionMode::Manual,
        });
        for _ in 0..HARVEST_MANUAL_TICKS {
            app.update();
        }
        let lr = app.world().get::<LifeRecord>(player).unwrap();
        assert_eq!(
            count_biography_matching(lr, |e| matches!(
                e,
                BE::PlotHarvestedByOther { .. } | BE::PlotHarvestedFromOther { .. }
            )),
            0,
            "自家收不应记偷菜条目"
        );
    }

    #[test]
    fn stolen_harvest_records_both_sides() {
        let mut app = build_harvest_app();
        let pos = BlockPos::new(3, 64, 7);
        let owner = spawn_player_with_lifelog(&mut app, "alice", pos);
        let thief = spawn_player_with_lifelog(&mut app, "bob", pos);
        spawn_owned_ripe_plot(&mut app, "ning_mai_cao", pos, Some(owner));
        app.world_mut().send_event(StartHarvestRequest {
            player: thief,
            pos,
            mode: SessionMode::Manual,
        });
        for _ in 0..HARVEST_MANUAL_TICKS {
            app.update();
        }
        let owner_lr = app.world().get::<LifeRecord>(owner).unwrap();
        let thief_lr = app.world().get::<LifeRecord>(thief).unwrap();

        assert_eq!(
            count_biography_matching(owner_lr, |e| matches!(
                e,
                BE::PlotHarvestedByOther {
                    plant_id, plot_pos, ..
                } if plant_id == "ning_mai_cao" && plot_pos == &[3, 64, 7]
            )),
            1,
            "owner 应记一条 PlotHarvestedByOther"
        );
        assert_eq!(
            count_biography_matching(thief_lr, |e| matches!(
                e,
                BE::PlotHarvestedFromOther {
                    plant_id, plot_pos, ..
                } if plant_id == "ning_mai_cao" && plot_pos == &[3, 64, 7]
            )),
            1,
            "operator 应记一条 PlotHarvestedFromOther"
        );
    }

    #[test]
    fn drain_qi_steals_into_player_and_zone_with_lifelog() {
        use crate::cultivation::components::Cultivation;
        use crate::lingtian::session::DRAIN_QI_TICKS;
        let mut app = build_app();
        let owner = app
            .world_mut()
            .spawn((empty_inventory_8x8(), LifeRecord::new("alice")))
            .id();
        let thief_cult = Cultivation {
            qi_current: 0.0,
            qi_max: 100.0,
            ..Default::default()
        };
        let pos = BlockPos::new(0, 64, 0);
        let thief = valid_test_player(
            &mut app,
            (empty_inventory_8x8(), LifeRecord::new("bob"), thief_cult),
            pos,
        );
        let mut p = LingtianPlot::new(pos, Some(owner));
        p.plot_qi = 0.5;
        let plot = app.world_mut().spawn(p).id();

        let zone_before = app.world().resource::<ZoneQiAccount>().get(DEFAULT_ZONE);
        app.world_mut()
            .send_event(StartDrainQiRequest { player: thief, pos });
        for _ in 0..DRAIN_QI_TICKS {
            app.update();
        }

        let p = app.world().get::<LingtianPlot>(plot).unwrap();
        assert!(p.plot_qi.abs() < 1e-6, "偷后 plot_qi 清零");

        let cult = app.world().get::<Cultivation>(thief).unwrap();
        assert!(
            (cult.qi_current - 0.4).abs() < 1e-5,
            "thief.qi_current={}",
            cult.qi_current
        );

        let zone_after = app.world().resource::<ZoneQiAccount>().get(DEFAULT_ZONE);
        assert!(
            (zone_after - zone_before - 0.1).abs() < 1e-5,
            "zone qi delta={}",
            zone_after - zone_before
        );
        let qi_transfers: Vec<_> = app
            .world_mut()
            .resource_mut::<Events<QiTransfer>>()
            .drain()
            .collect();
        assert_eq!(
            qi_transfers.len(),
            2,
            "偷灵应写 player + zone 两笔 ledger event"
        );
        let plot_account =
            QiAccountId::container(format!("lingtian_plot:{},{},{}", pos.x, pos.y, pos.z));
        assert_eq!(qi_transfers[0].from, plot_account);
        assert_eq!(qi_transfers[0].to, QiAccountId::player("bob"));
        assert!((qi_transfers[0].amount - 0.4).abs() < 1e-6);
        assert_eq!(qi_transfers[0].reason, QiTransferReason::Channeling);
        assert_eq!(qi_transfers[1].from, plot_account);
        assert_eq!(qi_transfers[1].to, QiAccountId::zone(DEFAULT_ZONE));
        assert!((qi_transfers[1].amount - 0.1).abs() < 1e-6);
        assert_eq!(qi_transfers[1].reason, QiTransferReason::ReleaseToZone);

        let owner_lr = app.world().get::<LifeRecord>(owner).unwrap();
        let thief_lr = app.world().get::<LifeRecord>(thief).unwrap();
        assert_eq!(
            count_biography_matching(owner_lr, |e| matches!(e, BE::PlotQiDrainedByOther { .. })),
            1
        );
        assert_eq!(
            count_biography_matching(thief_lr, |e| matches!(e, BE::PlotQiDrainedFromOther { .. })),
            1
        );
    }

    #[test]
    fn drain_qi_releases_to_non_default_plot_zone_and_ledger() {
        use crate::cultivation::components::Cultivation;
        use crate::lingtian::session::DRAIN_QI_TICKS;
        let mut app = build_app();
        app.world_mut()
            .resource_mut::<ZoneQiAccount>()
            .set(DEFAULT_ZONE, 0.0);
        app.world_mut()
            .resource_mut::<ZoneQiAccount>()
            .set("north_wastes", 3.0);
        let pos = BlockPos::new(4, 64, 4);
        let thief = valid_test_player(
            &mut app,
            (
                empty_inventory_8x8(),
                LifeRecord::new("bob"),
                Cultivation {
                    qi_current: 0.0,
                    qi_max: 100.0,
                    ..Default::default()
                },
            ),
            pos,
        );
        let mut plot = LingtianPlot::new(pos, None).with_zone("north_wastes");
        plot.plot_qi = 0.5;
        app.world_mut().spawn(plot);

        app.world_mut()
            .send_event(StartDrainQiRequest { player: thief, pos });
        for _ in 0..DRAIN_QI_TICKS {
            app.update();
        }

        let accounts = app.world().resource::<ZoneQiAccount>();
        assert!(
            (accounts.get("north_wastes") - 3.1).abs() < 1e-5,
            "偷灵 20% 散逸应回流 north_wastes，实际 {}",
            accounts.get("north_wastes")
        );
        assert_eq!(accounts.get(DEFAULT_ZONE), 0.0, "default 不应收到散逸回流");

        let qi_transfers: Vec<_> = app
            .world_mut()
            .resource_mut::<Events<QiTransfer>>()
            .drain()
            .collect();
        assert_eq!(
            qi_transfers.len(),
            2,
            "应写 player + north_wastes 两笔 ledger"
        );
        assert_eq!(qi_transfers[1].to, QiAccountId::zone("north_wastes"));
        assert_eq!(qi_transfers[1].reason, QiTransferReason::ReleaseToZone);
        assert!((qi_transfers[1].amount - 0.1).abs() < 1e-6);
    }

    #[test]
    fn drain_qi_caps_at_qi_max() {
        use crate::cultivation::components::Cultivation;
        use crate::lingtian::session::DRAIN_QI_TICKS;
        let mut app = build_app();
        // plot_qi=5.0 → drained 5.0 → to_player 4.0；qi_current=99 / qi_max=100 余 1
        // → 注 1.0 → cap 100
        let pos = BlockPos::new(0, 64, 0);
        let mut p = LingtianPlot::new(pos, None);
        p.plot_qi_cap = 5.0;
        p.plot_qi = 5.0;
        app.world_mut().spawn(p);
        let zone_before = app.world().resource::<ZoneQiAccount>().get(DEFAULT_ZONE);
        let cult = Cultivation {
            qi_current: 99.0,
            qi_max: 100.0,
            ..Default::default()
        };
        let player = valid_test_player(
            &mut app,
            (empty_inventory_8x8(), LifeRecord::new("p"), cult),
            pos,
        );
        app.world_mut()
            .send_event(StartDrainQiRequest { player, pos });
        for _ in 0..DRAIN_QI_TICKS {
            app.update();
        }
        let cult = app.world().get::<Cultivation>(player).unwrap();
        assert!(
            (cult.qi_current - 100.0).abs() < 1e-5,
            "应封顶 qi_max=100, 实得 {}",
            cult.qi_current
        );
        let zone_after = app.world().resource::<ZoneQiAccount>().get(DEFAULT_ZONE);
        assert!(
            (zone_after - zone_before - 4.0).abs() < 1e-5,
            "玩家 cap 溢出应回流 zone, delta={}",
            zone_after - zone_before
        );
        let qi_transfers: Vec<_> = app
            .world_mut()
            .resource_mut::<Events<QiTransfer>>()
            .drain()
            .collect();
        assert_eq!(qi_transfers.len(), 2);
        let plot_account =
            QiAccountId::container(format!("lingtian_plot:{},{},{}", pos.x, pos.y, pos.z));
        assert_eq!(qi_transfers[0].from, plot_account);
        assert_eq!(qi_transfers[0].to, QiAccountId::player("p"));
        assert!((qi_transfers[0].amount - 1.0).abs() < 1e-6);
        assert_eq!(qi_transfers[0].reason, QiTransferReason::Channeling);
        assert_eq!(qi_transfers[1].from, plot_account);
        assert_eq!(qi_transfers[1].to, QiAccountId::zone(DEFAULT_ZONE));
        assert!((qi_transfers[1].amount - 4.0).abs() < 1e-6);
        assert_eq!(qi_transfers[1].reason, QiTransferReason::ReleaseToZone);
    }

    #[test]
    fn drain_qi_without_life_record_still_credits_cultivation_but_skips_player_ledger() {
        use crate::cultivation::components::Cultivation;
        use crate::lingtian::session::DRAIN_QI_TICKS;
        let mut app = build_app();
        let pos = BlockPos::new(0, 64, 0);
        let mut plot = LingtianPlot::new(pos, None);
        plot.plot_qi = 0.5;
        app.world_mut().spawn(plot);
        let player = valid_test_player(
            &mut app,
            (
                empty_inventory_8x8(),
                Cultivation {
                    qi_current: 0.0,
                    qi_max: 100.0,
                    ..Default::default()
                },
            ),
            pos,
        );

        app.world_mut()
            .send_event(StartDrainQiRequest { player, pos });
        for _ in 0..DRAIN_QI_TICKS {
            app.update();
        }

        let cult = app.world().get::<Cultivation>(player).unwrap();
        assert!(
            (cult.qi_current - 0.4).abs() < 1e-5,
            "缺 LifeRecord 不应阻止 Cultivation 实际增长, got {}",
            cult.qi_current
        );
        let qi_transfers: Vec<_> = app
            .world_mut()
            .resource_mut::<Events<QiTransfer>>()
            .drain()
            .collect();
        assert_eq!(qi_transfers.len(), 1, "缺稳定玩家账户时只写 zone 回流账");
        let plot_account =
            QiAccountId::container(format!("lingtian_plot:{},{},{}", pos.x, pos.y, pos.z));
        assert_eq!(qi_transfers[0].from, plot_account);
        assert_eq!(qi_transfers[0].to, QiAccountId::zone(DEFAULT_ZONE));
        assert!((qi_transfers[0].amount - 0.1).abs() < 1e-6);
        assert_eq!(qi_transfers[0].reason, QiTransferReason::ReleaseToZone);
    }

    #[test]
    fn drain_qi_rejected_on_empty_plot() {
        let mut app = build_app();
        let pos = BlockPos::new(0, 64, 0);
        let player =
            valid_test_player(&mut app, (empty_inventory_8x8(), LifeRecord::new("p")), pos);
        let mut p = LingtianPlot::new(pos, None);
        p.plot_qi = 0.0;
        app.world_mut().spawn(p);
        app.world_mut()
            .send_event(StartDrainQiRequest { player, pos });
        app.update();
        assert!(app.world().resource::<ActiveLingtianSessions>().is_empty());
    }

    #[test]
    fn ownerless_harvest_records_neither_side() {
        let mut app = build_harvest_app();
        let pos = BlockPos::new(0, 64, 0);
        let player = spawn_player_with_lifelog(&mut app, "wanderer", pos);
        spawn_owned_ripe_plot(&mut app, "ci_she_hao", pos, None); // 无主田
        app.world_mut().send_event(StartHarvestRequest {
            player,
            pos,
            mode: SessionMode::Manual,
        });
        for _ in 0..HARVEST_MANUAL_TICKS {
            app.update();
        }
        let lr = app.world().get::<LifeRecord>(player).unwrap();
        assert_eq!(
            count_biography_matching(lr, |e| matches!(
                e,
                BE::PlotHarvestedByOther { .. } | BE::PlotHarvestedFromOther { .. }
            )),
            0
        );
    }

    fn queue_test_request(app: &mut App, action: &str, player: Entity, pos: BlockPos) {
        let request = match action {
            "till" => PendingLingtianRequest::Till {
                actor: player,
                pos,
                hoe_instance_id: 1,
                mode: SessionMode::Manual,
            },
            "renew" => PendingLingtianRequest::Renew {
                actor: player,
                pos,
                hoe_instance_id: 1,
            },
            "planting" => PendingLingtianRequest::Planting {
                actor: player,
                pos,
                plant_id: "ci_she_hao".into(),
            },
            "harvest" => PendingLingtianRequest::Harvest {
                actor: player,
                pos,
                mode: SessionMode::Manual,
            },
            "replenish" => PendingLingtianRequest::Replenish {
                actor: player,
                pos,
                source: ReplenishSource::BoneCoin,
            },
            "drain_qi" => PendingLingtianRequest::DrainQi { actor: player, pos },
            _ => unreachable!(),
        };
        app.world_mut()
            .resource_mut::<PendingLingtianRequests>()
            .push(request);
    }

    #[test]
    fn validator_preserves_cross_action_fifo_per_actor() {
        let mut app = App::new();
        app.init_resource::<PendingLingtianRequests>()
            .add_event::<StartTillRequest>()
            .add_event::<StartRenewRequest>()
            .add_event::<StartPlantingRequest>()
            .add_event::<StartHarvestRequest>()
            .add_event::<StartReplenishRequest>()
            .add_event::<StartDrainQiRequest>();
        app.add_systems(Update, validate_and_dispatch_lingtian_requests);
        // spawn bundle 与 Position 分开插入：bundle 自带 Position，同 tuple spawn 会
        // 撞重复组件 panic（insert 语义是替换所以分开插安全）
        let actor = spawn_test_player(&mut app, ());
        let pos = BlockPos::new(0, 64, 0);
        queue_test_request(&mut app, "till", actor, pos);
        queue_test_request(&mut app, "renew", actor, pos);
        queue_test_request(&mut app, "planting", actor, pos);
        queue_test_request(&mut app, "harvest", actor, pos);
        queue_test_request(&mut app, "replenish", actor, pos);
        queue_test_request(&mut app, "drain_qi", actor, pos);

        // central review 1984-31332727941 finding [5]：先前只验证前两条，后四条被
        // 重排（如 DrainQi 抢在 Planting 前）仍会通过。这里把六种 action 全部推进
        // 到底，每 tick 必须恰好 dispatch 入队顺序对应的那一种类型，且队列每 tick
        // 精确收缩 1 条（6→0）。任何乱序 dispatch 都会在它应出队的那一轮返回 0。
        // central review 1984-31447628937 finding [1]：每轮把**全部六种**事件资源
        // 排空计数——不只断言本轮应 dispatch 的那种恰好 1 条，还断言其余五种为 0。
        // 只检查应 dispatch 资源会把「正确弹出队列请求、但每 tick 额外多发一种
        // 事件」的坏实现放行（发错类型的多余事件从不落在被检查的资源上，或落在
        // 上一轮已被排空、此后不再检查的资源上）；排空全部资源使跨类型重复/多发
        // 在发生的当轮立刻暴露。
        let expected_order = [
            "till",
            "renew",
            "planting",
            "harvest",
            "replenish",
            "drain_qi",
        ];
        for (turn, action) in expected_order.into_iter().enumerate() {
            app.update();
            let dispatched_counts = [
                (
                    "till",
                    app.world_mut()
                        .resource_mut::<Events<StartTillRequest>>()
                        .drain()
                        .count(),
                ),
                (
                    "renew",
                    app.world_mut()
                        .resource_mut::<Events<StartRenewRequest>>()
                        .drain()
                        .count(),
                ),
                (
                    "planting",
                    app.world_mut()
                        .resource_mut::<Events<StartPlantingRequest>>()
                        .drain()
                        .count(),
                ),
                (
                    "harvest",
                    app.world_mut()
                        .resource_mut::<Events<StartHarvestRequest>>()
                        .drain()
                        .count(),
                ),
                (
                    "replenish",
                    app.world_mut()
                        .resource_mut::<Events<StartReplenishRequest>>()
                        .drain()
                        .count(),
                ),
                (
                    "drain_qi",
                    app.world_mut()
                        .resource_mut::<Events<StartDrainQiRequest>>()
                        .drain()
                        .count(),
                ),
            ];
            for (name, count) in dispatched_counts {
                if name == action {
                    assert_eq!(
                        count, 1,
                        "update {}: exactly one `{action}` must dispatch (per-actor FIFO), got {count}",
                        turn + 1
                    );
                } else {
                    assert_eq!(
                        count,
                        0,
                        "update {}: `{name}` must NOT dispatch on the `{action}` tick \
                         (exactly one action per tick), got {count}",
                        turn + 1
                    );
                }
            }
            assert_eq!(
                app.world().resource::<PendingLingtianRequests>().len(),
                6 - turn - 1,
                "update {}: queue must shrink by one per tick (FIFO)",
                turn + 1
            );
        }
    }

    #[test]
    fn start_handlers_index_plots_only_for_matching_event_ticks() {
        let mut app = App::new();
        app.insert_resource(ActiveLingtianSessions::new())
            .insert_resource(SeedRegistry::new())
            .insert_resource(ZoneQiAccount::new())
            .insert_resource(LingtianClock::default())
            .insert_resource(StartHandlerPlotScanCount::default())
            .add_event::<StartTillRequest>()
            .add_event::<StartRenewRequest>()
            .add_event::<StartPlantingRequest>()
            .add_event::<StartHarvestRequest>()
            .add_event::<StartReplenishRequest>()
            .add_event::<StartDrainQiRequest>()
            .add_systems(
                Update,
                (
                    handle_start_till,
                    handle_start_renew,
                    handle_start_planting,
                    handle_start_harvest,
                    handle_start_replenish,
                    handle_start_drain_qi,
                )
                    .chain(),
            );

        let plot_count = 7;
        for x in 0..plot_count {
            let mut plot = LingtianPlot::new(BlockPos::new(x, 64, 0), None);
            plot.plot_qi = 0.5;
            app.world_mut().spawn(plot);
        }

        app.update();
        app.update();
        let idle_count = app.world().resource::<StartHandlerPlotScanCount>();
        assert_eq!(
            (idle_count.index_builds, idle_count.scanned_plots),
            (0, 0),
            "six idle start handlers must not build an index or scan any plot"
        );

        app.world_mut().send_event(StartDrainQiRequest {
            player: Entity::from_raw(100),
            pos: BlockPos::new(0, 64, 0),
        });
        app.world_mut().send_event(StartDrainQiRequest {
            player: Entity::from_raw(101),
            pos: BlockPos::new(1, 64, 0),
        });
        app.update();
        let event_count = app.world().resource::<StartHandlerPlotScanCount>();
        assert_eq!(
            event_count.index_builds, 1,
            "only the handler with matching events may build a plot index on this tick"
        );
        assert_eq!(
            event_count.scanned_plots, plot_count as usize,
            "one event batch must scan each plot once, not once per request or idle handler"
        );

        app.update();
        let next_idle_count = app.world().resource::<StartHandlerPlotScanCount>();
        assert_eq!(
            (next_idle_count.index_builds, next_idle_count.scanned_plots),
            (1, plot_count as usize),
            "after the batch is consumed, the next idle tick must not scan or re-index plots"
        );
    }

    #[test]
    fn every_start_handler_fails_closed_for_wrong_or_missing_authority_components() {
        for (action_index, action) in [
            "till",
            "renew",
            "planting",
            "harvest",
            "replenish",
            "drain_qi",
        ]
        .into_iter()
        .enumerate()
        {
            for (denial_index, denial) in
                ["wrong_dimension", "missing_position", "missing_dimension"]
                    .into_iter()
                    .enumerate()
            {
                let pos = BlockPos::new(
                    10_000 + action_index as i32 * 10 + denial_index as i32,
                    64,
                    0,
                );
                let mut app = build_planting_app();
                let inventory = match action {
                    "till" | "renew" => make_inventory_with_hoe(HoeKind::Iron, 1.0),
                    "planting" => make_inventory_with_seed("ci_she_hao_seed", 2),
                    "replenish" => {
                        let mut inventory = empty_inventory_8x8();
                        inventory.bone_coins = 2;
                        inventory
                    }
                    _ => empty_inventory_8x8(),
                };
                let player = valid_test_player(
                    &mut app,
                    (
                        inventory,
                        Cultivation::default(),
                        LifeRecord::new("authority"),
                    ),
                    pos,
                );
                match action {
                    "renew" => {
                        let mut plot = LingtianPlot::new(pos, Some(player));
                        plot.harvest_count = crate::lingtian::plot::N_RENEW;
                        app.world_mut().spawn(plot);
                    }
                    "planting" => {
                        app.world_mut().spawn(LingtianPlot::new(pos, Some(player)));
                    }
                    "harvest" => {
                        spawn_ripe_plot(&mut app, "ci_she_hao", pos);
                    }
                    "replenish" => {
                        app.world_mut().spawn(LingtianPlot::new(pos, Some(player)));
                    }
                    "drain_qi" => {
                        let mut plot = LingtianPlot::new(pos, None);
                        plot.plot_qi = 0.5;
                        app.world_mut().spawn(plot);
                    }
                    _ => {}
                }
                let expected_reason = match denial {
                    "wrong_dimension" => {
                        app.world_mut().entity_mut(player).insert(CurrentDimension(
                            crate::world::dimension::DimensionKind::Tsy,
                        ));
                        crate::lingtian::range_gate::LingtianInteractionDenial::WrongDimension
                    }
                    "missing_position" => {
                        app.world_mut().entity_mut(player).remove::<Position>();
                        crate::lingtian::range_gate::LingtianInteractionDenial::MissingPosition
                    }
                    "missing_dimension" => {
                        app.world_mut()
                            .entity_mut(player)
                            .remove::<CurrentDimension>();
                        crate::lingtian::range_gate::LingtianInteractionDenial::MissingDimension
                    }
                    _ => unreachable!(),
                };
                queue_test_request(&mut app, action, player, pos);
                app.update();
                assert!(
                    crate::lingtian::range_gate::denial_was_logged(player, pos, expected_reason,),
                    "{action} {denial} must execute the interaction gate denial path"
                );
                assert!(
                    app.world().resource::<ActiveLingtianSessions>().is_empty(),
                    "{action} must fail closed for {denial} even when all action prerequisites are valid"
                );
            }
        }
    }

    fn move_test_player_out_of_range(app: &mut App, player: Entity, target: BlockPos) {
        app.world_mut()
            .entity_mut(player)
            .insert(Position(DVec3::new(
                f64::from(target.x) + 20.5,
                f64::from(target.y) + 0.5,
                f64::from(target.z) + 0.5,
            )));
    }

    fn run_until_session_finishes(app: &mut App, ticks: u32) {
        for _ in 0..ticks {
            app.update();
        }
        assert!(
            app.world().resource::<ActiveLingtianSessions>().is_empty(),
            "finished session must leave the active-session table even when completion is denied"
        );
    }

    #[test]
    fn all_start_handlers_reject_out_of_range_without_mutating_targets_or_inventory() {
        let far = Position(DVec3::new(20.5, 64.5, 0.5));
        let overworld = CurrentDimension(crate::world::dimension::DimensionKind::Overworld);

        let mut till = build_app();
        let till_player = spawn_test_player(&mut till, make_inventory_with_hoe(HoeKind::Iron, 1.0));
        till.world_mut().entity_mut(till_player).insert(far);
        queue_test_request(&mut till, "till", till_player, BlockPos::new(0, 64, 0));
        till.update();
        assert!(till.world().resource::<ActiveLingtianSessions>().is_empty());
        assert_eq!(
            till.world()
                .get::<PlayerInventory>(till_player)
                .unwrap()
                .equipped[MAIN_HAND_SLOT]
                .held
                .as_ref()
                .unwrap()
                .durability,
            1.0,
            "remote till must not wear the hoe"
        );
        assert_eq!(
            till.world_mut()
                .query::<&LingtianPlot>()
                .iter(till.world())
                .count(),
            0,
            "remote till must not create a plot"
        );

        let mut renew = build_app();
        let renew_pos = BlockPos::new(0, 64, 0);
        let renew_player = valid_test_player(
            &mut renew,
            make_inventory_with_hoe(HoeKind::Iron, 1.0),
            renew_pos,
        );
        renew.world_mut().entity_mut(renew_player).insert(far);
        let mut barren = LingtianPlot::new(renew_pos, Some(renew_player));
        barren.harvest_count = crate::lingtian::plot::N_RENEW;
        let renew_plot = renew.world_mut().spawn(barren).id();
        queue_test_request(&mut renew, "renew", renew_player, renew_pos);
        renew.update();
        assert!(renew
            .world()
            .resource::<ActiveLingtianSessions>()
            .is_empty());
        assert!(renew
            .world()
            .get::<LingtianPlot>(renew_plot)
            .unwrap()
            .is_barren());

        let mut planting = build_planting_app();
        let planting_pos = BlockPos::new(0, 64, 0);
        let planting_player = valid_test_player(
            &mut planting,
            make_inventory_with_seed("ci_she_hao_seed", 2),
            planting_pos,
        );
        planting.world_mut().entity_mut(planting_player).insert(far);
        let planting_plot = planting
            .world_mut()
            .spawn(LingtianPlot::new(planting_pos, Some(planting_player)))
            .id();
        queue_test_request(&mut planting, "planting", planting_player, planting_pos);
        planting.update();
        assert!(planting
            .world()
            .resource::<ActiveLingtianSessions>()
            .is_empty());
        assert!(planting
            .world()
            .get::<LingtianPlot>(planting_plot)
            .unwrap()
            .crop
            .is_none());
        assert_eq!(
            planting
                .world()
                .get::<PlayerInventory>(planting_player)
                .unwrap()
                .containers[0]
                .items[0]
                .instance
                .stack_count,
            2,
            "remote planting must not consume seed"
        );

        let mut harvest = build_harvest_app();
        let harvest_pos = BlockPos::new(0, 64, 0);
        let harvest_player = valid_test_player(&mut harvest, empty_inventory_8x8(), harvest_pos);
        harvest.world_mut().entity_mut(harvest_player).insert(far);
        let harvest_plot = spawn_ripe_plot(&mut harvest, "ci_she_hao", harvest_pos);
        queue_test_request(&mut harvest, "harvest", harvest_player, harvest_pos);
        harvest.update();
        assert!(harvest
            .world()
            .resource::<ActiveLingtianSessions>()
            .is_empty());
        assert!(harvest
            .world()
            .get::<LingtianPlot>(harvest_plot)
            .unwrap()
            .crop
            .as_ref()
            .is_some_and(|crop| crop.is_ripe()));

        let mut replenish = build_app();
        let replenish_pos = BlockPos::new(0, 64, 0);
        let mut replenish_inventory = empty_inventory_8x8();
        replenish_inventory.bone_coins = 2;
        let replenish_player =
            valid_test_player(&mut replenish, replenish_inventory, replenish_pos);
        replenish
            .world_mut()
            .entity_mut(replenish_player)
            .insert(far);
        let replenish_plot = replenish
            .world_mut()
            .spawn(LingtianPlot::new(replenish_pos, Some(replenish_player)))
            .id();
        queue_test_request(&mut replenish, "replenish", replenish_player, replenish_pos);
        replenish.update();
        assert!(replenish
            .world()
            .resource::<ActiveLingtianSessions>()
            .is_empty());
        assert_eq!(
            replenish
                .world()
                .get::<PlayerInventory>(replenish_player)
                .unwrap()
                .bone_coins,
            2,
            "remote replenish must not consume material"
        );
        assert_eq!(
            replenish
                .world()
                .get::<LingtianPlot>(replenish_plot)
                .unwrap()
                .plot_qi,
            0.0
        );

        let mut drain = build_app();
        let drain_pos = BlockPos::new(0, 64, 0);
        let drain_player = valid_test_player(
            &mut drain,
            (
                empty_inventory_8x8(),
                Cultivation::default(),
                LifeRecord::new("remote"),
            ),
            drain_pos,
        );
        drain.world_mut().entity_mut(drain_player).insert(far);
        drain.world_mut().entity_mut(drain_player).insert(overworld);
        let mut qi_plot = LingtianPlot::new(drain_pos, None);
        qi_plot.plot_qi = 0.5;
        let drain_plot = drain.world_mut().spawn(qi_plot).id();
        queue_test_request(&mut drain, "drain_qi", drain_player, drain_pos);
        drain.update();
        assert!(drain
            .world()
            .resource::<ActiveLingtianSessions>()
            .is_empty());
        assert_eq!(
            drain
                .world()
                .get::<LingtianPlot>(drain_plot)
                .unwrap()
                .plot_qi,
            0.5
        );
        assert_eq!(
            drain
                .world()
                .get::<Cultivation>(drain_player)
                .unwrap()
                .qi_current,
            0.0
        );
    }

    #[test]
    fn all_player_completion_paths_revalidate_range_before_side_effects() {
        let pos = BlockPos::new(0, 64, 0);

        let mut till = build_app();
        let till_player =
            valid_test_player(&mut till, make_inventory_with_hoe(HoeKind::Iron, 1.0), pos);
        till.world_mut().send_event(StartTillRequest {
            player: till_player,
            pos,
            hoe_instance_id: 1,
            mode: SessionMode::Manual,
            terrain: TerrainKind::Grass,
            environment: PlotEnvironment::base(),
        });
        till.update();
        move_test_player_out_of_range(&mut till, till_player, pos);
        run_until_session_finishes(&mut till, TILL_MANUAL_TICKS - 1);
        assert_eq!(
            till.world_mut()
                .query::<&LingtianPlot>()
                .iter(till.world())
                .count(),
            0
        );
        assert_eq!(
            till.world()
                .get::<PlayerInventory>(till_player)
                .unwrap()
                .equipped[MAIN_HAND_SLOT]
                .held
                .as_ref()
                .unwrap()
                .durability,
            1.0
        );
        assert_eq!(
            till.world_mut()
                .resource_mut::<Events<TillCompleted>>()
                .drain()
                .count(),
            0
        );

        let mut renew = build_app();
        let renew_player =
            valid_test_player(&mut renew, make_inventory_with_hoe(HoeKind::Iron, 1.0), pos);
        let mut barren = LingtianPlot::new(pos, Some(renew_player));
        barren.harvest_count = crate::lingtian::plot::N_RENEW;
        let renew_plot = renew.world_mut().spawn(barren).id();
        renew.world_mut().send_event(StartRenewRequest {
            player: renew_player,
            pos,
            hoe_instance_id: 1,
        });
        renew.update();
        move_test_player_out_of_range(&mut renew, renew_player, pos);
        run_until_session_finishes(&mut renew, RENEW_TICKS - 1);
        assert!(renew
            .world()
            .get::<LingtianPlot>(renew_plot)
            .unwrap()
            .is_barren());
        assert_eq!(
            renew
                .world_mut()
                .resource_mut::<Events<RenewCompleted>>()
                .drain()
                .count(),
            0
        );

        let mut planting = build_planting_app();
        let planting_player = valid_test_player(
            &mut planting,
            make_inventory_with_seed("ci_she_hao_seed", 2),
            pos,
        );
        let planting_plot = planting
            .world_mut()
            .spawn(LingtianPlot::new(pos, Some(planting_player)))
            .id();
        planting.world_mut().send_event(StartPlantingRequest {
            player: planting_player,
            pos,
            plant_id: "ci_she_hao".into(),
        });
        planting.update();
        move_test_player_out_of_range(&mut planting, planting_player, pos);
        run_until_session_finishes(&mut planting, PLANTING_TICKS - 1);
        assert!(planting
            .world()
            .get::<LingtianPlot>(planting_plot)
            .unwrap()
            .crop
            .is_none());
        assert_eq!(
            planting
                .world()
                .get::<PlayerInventory>(planting_player)
                .unwrap()
                .containers[0]
                .items[0]
                .instance
                .stack_count,
            2
        );
        assert_eq!(
            planting
                .world_mut()
                .resource_mut::<Events<PlantingCompleted>>()
                .drain()
                .count(),
            0
        );

        let mut harvest = build_harvest_app();
        let harvest_player = valid_test_player(&mut harvest, empty_inventory_8x8(), pos);
        let harvest_plot = spawn_ripe_plot(&mut harvest, "ci_she_hao", pos);
        harvest.world_mut().send_event(StartHarvestRequest {
            player: harvest_player,
            pos,
            mode: SessionMode::Manual,
        });
        harvest.update();
        move_test_player_out_of_range(&mut harvest, harvest_player, pos);
        run_until_session_finishes(&mut harvest, HARVEST_MANUAL_TICKS - 1);
        let plot = harvest.world().get::<LingtianPlot>(harvest_plot).unwrap();
        assert!(plot.crop.as_ref().is_some_and(|crop| crop.is_ripe()));
        assert_eq!(plot.harvest_count, 0);
        assert_eq!(
            count_in_main_pack(
                harvest
                    .world()
                    .get::<PlayerInventory>(harvest_player)
                    .unwrap(),
                "ci_she_hao"
            ),
            0
        );
        assert_eq!(
            harvest
                .world_mut()
                .resource_mut::<Events<HarvestCompleted>>()
                .drain()
                .count(),
            0
        );

        let mut replenish = build_app();
        let mut inventory = empty_inventory_8x8();
        inventory.bone_coins = 2;
        let replenish_player = valid_test_player(&mut replenish, inventory, pos);
        let replenish_plot = replenish
            .world_mut()
            .spawn(LingtianPlot::new(pos, Some(replenish_player)))
            .id();
        replenish.world_mut().send_event(StartReplenishRequest {
            player: replenish_player,
            pos,
            source: ReplenishSource::BoneCoin,
        });
        replenish.update();
        move_test_player_out_of_range(&mut replenish, replenish_player, pos);
        run_until_session_finishes(
            &mut replenish,
            ReplenishSource::BoneCoin.duration_ticks() - 1,
        );
        assert_eq!(
            replenish
                .world()
                .get::<PlayerInventory>(replenish_player)
                .unwrap()
                .bone_coins,
            2
        );
        assert_eq!(
            replenish
                .world()
                .get::<LingtianPlot>(replenish_plot)
                .unwrap()
                .plot_qi,
            0.0
        );
        assert_eq!(
            replenish
                .world_mut()
                .resource_mut::<Events<ReplenishCompleted>>()
                .drain()
                .count(),
            0
        );

        let mut drain = build_app();
        let drain_player = valid_test_player(
            &mut drain,
            (
                empty_inventory_8x8(),
                Cultivation::default(),
                LifeRecord::new("completion"),
            ),
            pos,
        );
        let mut qi_plot = LingtianPlot::new(pos, None);
        qi_plot.plot_qi = 0.5;
        let drain_plot = drain.world_mut().spawn(qi_plot).id();
        drain.world_mut().send_event(StartDrainQiRequest {
            player: drain_player,
            pos,
        });
        drain.update();
        move_test_player_out_of_range(&mut drain, drain_player, pos);
        run_until_session_finishes(&mut drain, DRAIN_QI_TICKS - 1);
        assert_eq!(
            drain
                .world()
                .get::<LingtianPlot>(drain_plot)
                .unwrap()
                .plot_qi,
            0.5
        );
        assert_eq!(
            drain
                .world()
                .get::<Cultivation>(drain_player)
                .unwrap()
                .qi_current,
            0.0
        );
        assert_eq!(
            drain
                .world_mut()
                .resource_mut::<Events<QiTransfer>>()
                .drain()
                .count(),
            0
        );
        assert_eq!(
            drain
                .world_mut()
                .resource_mut::<Events<DrainQiCompleted>>()
                .drain()
                .count(),
            0
        );
    }

    #[test]
    fn completion_gate_rejects_wrong_or_missing_player_authority_components() {
        let pos = BlockPos::new(0, 64, 0);
        for denial in ["wrong_dimension", "missing_position", "missing_dimension"] {
            let mut app = build_app();
            let player =
                valid_test_player(&mut app, make_inventory_with_hoe(HoeKind::Iron, 1.0), pos);
            app.world_mut().send_event(StartTillRequest {
                player,
                pos,
                hoe_instance_id: 1,
                mode: SessionMode::Manual,
                terrain: TerrainKind::Grass,
                environment: PlotEnvironment::base(),
            });
            app.update();
            match denial {
                "wrong_dimension" => {
                    app.world_mut().entity_mut(player).insert(CurrentDimension(
                        crate::world::dimension::DimensionKind::Tsy,
                    ));
                }
                "missing_position" => {
                    app.world_mut().entity_mut(player).remove::<Position>();
                }
                "missing_dimension" => {
                    app.world_mut()
                        .entity_mut(player)
                        .remove::<CurrentDimension>();
                }
                _ => unreachable!(),
            }
            run_until_session_finishes(&mut app, TILL_MANUAL_TICKS - 1);
            assert_eq!(
                app.world_mut()
                    .query::<&LingtianPlot>()
                    .iter(app.world())
                    .count(),
                0,
                "{denial} must reject before till creates a plot"
            );
            assert_eq!(
                app.world_mut()
                    .resource_mut::<Events<TillCompleted>>()
                    .drain()
                    .count(),
                0,
                "{denial} must not emit TillCompleted"
            );
        }
    }

    #[test]
    fn same_tick_dimension_transfer_precedes_start_validation() {
        let mut app = build_app();
        let overworld = app.world_mut().spawn(OverworldLayer).id();
        let tsy = app.world_mut().spawn(TsyLayer).id();
        app.insert_resource(DimensionLayers { overworld, tsy });
        app.add_event::<DimensionTransferRequest>();
        app.add_systems(
            Update,
            apply_dimension_transfers
                .in_set(DimensionTransferSet)
                .in_set(crate::world::movement_commit::AuthoritativePositionCommitSet),
        );

        let pos = BlockPos::new(0, 64, 0);
        let player = valid_test_player(&mut app, make_inventory_with_hoe(HoeKind::Iron, 1.0), pos);
        let mut visible_layers = VisibleEntityLayers::default();
        visible_layers.0.insert(tsy);
        app.world_mut().entity_mut(player).insert((
            CurrentDimension(DimensionKind::Tsy),
            Position(DVec3::new(100.5, 80.5, 100.5)),
            EntityLayerId(tsy),
            VisibleChunkLayer(tsy),
            visible_layers,
        ));
        app.world_mut().send_event(DimensionTransferRequest {
            entity: player,
            target: DimensionKind::Overworld,
            target_pos: DVec3::new(0.5, 64.5, 0.5),
        });
        app.world_mut()
            .resource_mut::<crate::lingtian::requests::PendingLingtianRequests>()
            .push(PendingLingtianRequest::Till {
                actor: player,
                pos,
                hoe_instance_id: 1,
                mode: SessionMode::Manual,
            });

        app.update();

        assert_eq!(
            app.world().get::<CurrentDimension>(player),
            Some(&CurrentDimension(DimensionKind::Overworld)),
            "same-tick transfer must be applied before start authority is read"
        );
        let start_events = app.world().resource::<Events<StartTillRequest>>();
        assert_eq!(
            start_events.get_reader().read(start_events).count(),
            1,
            "post-transfer gate must dispatch the real pending request on the first update"
        );
        assert!(
            app.world()
                .resource::<crate::lingtian::requests::PendingLingtianRequests>()
                .is_empty(),
            "accepted request must leave the persistent ingress queue"
        );
    }

    #[test]
    fn same_tick_dimension_transfer_precedes_completion_revalidation() {
        let mut app = build_app();
        let overworld = app.world_mut().spawn(OverworldLayer).id();
        let tsy = app.world_mut().spawn(TsyLayer).id();
        app.insert_resource(DimensionLayers { overworld, tsy });
        app.add_event::<DimensionTransferRequest>();
        app.add_systems(
            Update,
            apply_dimension_transfers
                .in_set(DimensionTransferSet)
                .in_set(crate::world::movement_commit::AuthoritativePositionCommitSet),
        );

        let pos = BlockPos::new(0, 64, 0);
        let player = valid_test_player(&mut app, make_inventory_with_hoe(HoeKind::Iron, 1.0), pos);
        let mut visible_layers = VisibleEntityLayers::default();
        visible_layers.0.insert(overworld);
        app.world_mut().entity_mut(player).insert((
            EntityLayerId(overworld),
            VisibleChunkLayer(overworld),
            visible_layers,
        ));
        app.world_mut().send_event(StartTillRequest {
            player,
            pos,
            hoe_instance_id: 1,
            mode: SessionMode::Manual,
            terrain: TerrainKind::Grass,
            environment: PlotEnvironment::base(),
        });
        app.update();
        for _ in 0..TILL_MANUAL_TICKS - 2 {
            app.update();
        }

        app.world_mut().send_event(DimensionTransferRequest {
            entity: player,
            target: DimensionKind::Tsy,
            target_pos: DVec3::new(0.5, 80.5, 0.5),
        });
        app.update();

        assert_eq!(
            app.world().get::<CurrentDimension>(player),
            Some(&CurrentDimension(DimensionKind::Tsy)),
            "same-tick transfer must be applied before completion authority is read"
        );
        assert_eq!(
            app.world_mut()
                .query::<&LingtianPlot>()
                .iter(app.world())
                .count(),
            0,
            "transferred player must not create a plot on the finishing tick"
        );
        assert_eq!(
            app.world().get::<PlayerInventory>(player).unwrap().equipped[MAIN_HAND_SLOT]
                .held
                .as_ref()
                .unwrap()
                .durability,
            1.0,
            "transferred player must not wear the hoe on denied completion"
        );
        assert_eq!(
            app.world_mut()
                .resource_mut::<Events<TillCompleted>>()
                .drain()
                .count(),
            0,
            "transferred player must not emit TillCompleted"
        );
    }

    fn finished_session(mut session: ActiveSession) -> ActiveSession {
        while !session.is_finished() {
            session.tick();
        }
        session
    }

    #[test]
    fn till_reservation_is_shared_exclusive_and_released_on_cancel_and_settlement() {
        let actor_a = Entity::from_raw(1);
        let actor_b = Entity::from_raw(2);
        let pos = BlockPos::new(5, 64, 5);
        let session = TillSession::new(
            pos,
            HoeKind::Iron,
            11,
            SessionMode::Manual,
            PlotEnvironment::base(),
        );
        let mut sessions = ActiveLingtianSessions::new();

        assert!(sessions.try_insert_till(actor_a, session.clone(), false));
        assert!(!sessions.try_insert_till(actor_b, session.clone(), false));
        assert!(!sessions.try_insert_till(actor_b, session.clone(), true));
        assert_eq!(sessions.pending_reservations(), 1);

        sessions.clear(actor_a);
        assert_eq!(sessions.pending_reservations(), 0);
        let finished = finished_session(ActiveSession::Till(session));
        let ActiveSession::Till(finished) = finished else {
            unreachable!();
        };
        assert!(sessions.try_insert_till(actor_b, finished, false));

        let drained = sessions.drain_finished();
        assert_eq!(drained.len(), 1);
        assert_eq!(sessions.pending_reservations(), 1);
        sessions.settle_reservations();
        assert_eq!(sessions.pending_reservations(), 0);
    }

    #[test]
    fn npc_finished_sessions_settle_all_direct_farming_variants() {
        let pos = BlockPos::new(0, 64, 0);

        let mut till_app = build_app();
        let till_npc = till_app.world_mut().spawn(NpcMarker).id();
        let ActiveSession::Till(finished_till) =
            finished_session(ActiveSession::Till(TillSession::new(
                pos,
                HoeKind::Iron,
                1,
                SessionMode::Manual,
                PlotEnvironment::base(),
            )))
        else {
            unreachable!();
        };
        till_app
            .world_mut()
            .resource_mut::<ActiveLingtianSessions>()
            .try_insert_till(till_npc, finished_till, false);
        till_app.update();
        assert_eq!(
            till_app
                .world_mut()
                .query::<&LingtianPlot>()
                .iter(till_app.world())
                .filter(|plot| plot.pos == pos)
                .count(),
            1,
            "live NPC Till completion must create the plot"
        );

        let plant_id: PlantId = "ci_she_hao".into();
        let mut planting_app = build_app();
        let plant_registry = registry_with_three_test_plants();
        planting_app.insert_resource(SeedRegistry::from_plant_registry(&plant_registry));
        planting_app.insert_resource(plant_registry);
        let planting_npc = planting_app.world_mut().spawn(NpcMarker).id();
        let planting_plot = planting_app
            .world_mut()
            .spawn(LingtianPlot::new(pos, None))
            .id();
        planting_app
            .world_mut()
            .resource_mut::<ActiveLingtianSessions>()
            .try_insert(
                planting_npc,
                finished_session(ActiveSession::Planting(PlantingSession::new(
                    pos,
                    plant_id.clone(),
                ))),
            );
        planting_app.update();
        assert_eq!(
            planting_app
                .world()
                .get::<LingtianPlot>(planting_plot)
                .unwrap()
                .crop
                .as_ref()
                .map(|crop| &crop.kind),
            Some(&plant_id),
            "live NPC Planting completion must populate the crop"
        );

        let mut harvest_app = build_app();
        harvest_app.insert_resource(registry_with_three_test_plants());
        harvest_app.insert_resource(registry_with_herb_and_seed_templates());
        let harvest_npc = harvest_app.world_mut().spawn(NpcMarker).id();
        let mut harvest_plot_value = LingtianPlot::new(pos, None);
        let mut crop = CropInstance::new(plant_id.clone());
        crop.growth = 1.0;
        harvest_plot_value.crop = Some(crop);
        let harvest_plot = harvest_app.world_mut().spawn(harvest_plot_value).id();
        harvest_app
            .world_mut()
            .resource_mut::<ActiveLingtianSessions>()
            .try_insert(
                harvest_npc,
                finished_session(ActiveSession::Harvest(HarvestSession::new(
                    pos,
                    plant_id.clone(),
                    SessionMode::Auto,
                ))),
            );
        harvest_app.update();
        assert!(
            harvest_app
                .world()
                .get::<LingtianPlot>(harvest_plot)
                .unwrap()
                .crop
                .is_none(),
            "live NPC Harvest completion must clear the ripe crop"
        );

        let mut replenish_app = build_app();
        replenish_app
            .world_mut()
            .resource_mut::<ZoneQiAccount>()
            .set(DEFAULT_ZONE, 1.0);
        let replenish_npc = replenish_app.world_mut().spawn(NpcMarker).id();
        let replenish_plot = replenish_app
            .world_mut()
            .spawn(LingtianPlot::new(pos, None))
            .id();
        replenish_app
            .world_mut()
            .resource_mut::<ActiveLingtianSessions>()
            .try_insert(
                replenish_npc,
                finished_session(ActiveSession::Replenish(ReplenishSession::new(
                    pos,
                    ReplenishSource::Zone,
                ))),
            );
        replenish_app.update();
        assert_eq!(
            replenish_app
                .world()
                .get::<LingtianPlot>(replenish_plot)
                .unwrap()
                .plot_qi,
            ReplenishSource::Zone.plot_qi_amount(),
            "live NPC Replenish completion must deposit plot qi"
        );

        // fix-spec-1901-v2 §6.3 / OPEN-1 — NPC DrainQi 生产链不存在（farming
        // brain 只注册 Till/Plant/Harvest/Replenish/Migrate），不保留"测试
        // 手塞可达、生产永远不可达"的假链路；NPC 抽灵由未来独立 NPC plan
        // 定义 scorer/action/producer/qi ownership 合同。这里不再有
        // DrainQi 的 NPC completion fixture。
    }

    #[test]
    fn despawned_npc_finished_session_is_discarded() {
        let mut app = build_app();
        let pos = BlockPos::new(0, 64, 0);
        let npc = app.world_mut().spawn((NpcMarker, Despawned)).id();
        let ActiveSession::Till(finished_till) =
            finished_session(ActiveSession::Till(TillSession::new(
                pos,
                HoeKind::Iron,
                1,
                SessionMode::Manual,
                PlotEnvironment::base(),
            )))
        else {
            unreachable!();
        };
        app.world_mut()
            .resource_mut::<ActiveLingtianSessions>()
            .try_insert_till(npc, finished_till, false);

        app.update();

        assert_eq!(
            app.world_mut()
                .query::<&LingtianPlot>()
                .iter(app.world())
                .count(),
            0,
            "Despawned NPC must not settle a finished farming session"
        );
        assert_eq!(
            app.world_mut()
                .resource_mut::<Events<TillCompleted>>()
                .drain()
                .count(),
            0,
            "Despawned NPC must not emit completion events"
        );
    }
    //
    // Regression: the previous implementation used `Added<LingtianPlot>` and
    // returned early when ZoneRegistry was missing. If a plot spawned on a
    // frame where the registry hadn't been inserted yet, `Added` fired once,
    // the system bailed, and the plot's zone field stayed empty forever —
    // breaking later zone-keyed queries (e.g. daoshen spawn).

    fn zone_named(name: &str, min: DVec3, max: DVec3) -> crate::world::zone::Zone {
        crate::world::zone::Zone {
            name: name.to_string(),
            dimension: crate::world::dimension::DimensionKind::Overworld,
            bounds: (min, max),
            spirit_qi: 1.0,
            danger_level: 0,
            active_events: Vec::new(),
            patrol_anchors: Vec::new(),
            blocked_tiles: Vec::new(),
            qi_equilibrium: 0.0,
            qi_inflow_per_min: 0.0,
        }
    }

    #[test]
    fn auto_set_plot_zone_retries_when_registry_inserted_late() {
        use crate::lingtian::plot::LingtianPlot;
        use crate::world::zone::ZoneRegistry;

        let mut app = App::new();
        app.init_resource::<PendingPlotZones>();
        app.add_systems(Update, auto_set_plot_zone);

        // Spawn a plot WITHOUT a ZoneRegistry resource (simulates registry
        // not yet ready when worldgen-driven plot spawning happens).
        let plot_entity = app
            .world_mut()
            .spawn(LingtianPlot::new(BlockPos::new(50, 64, 50), None))
            .id();

        app.update();

        let plot = app.world().get::<LingtianPlot>(plot_entity).unwrap();
        assert!(
            plot.zone.is_empty(),
            "tick 1: no registry → zone must stay empty (got {:?})",
            plot.zone
        );

        // Now insert the registry at its normal revision-zero baseline. The
        // pending-entity cache must retry because the registry was previously
        // unobserved (`last_seen_spatial_revision = None`), NOT because the
        // revision differs — a fresh insert must not be conflated with an
        // already-observed revision 0. Previously this fixture had to fake
        // `spatial_revision: 1` to force the retry, masking the production bug.
        app.insert_resource(ZoneRegistry {
            zones: vec![zone_named(
                "spawn_zone",
                DVec3::new(0.0, 0.0, 0.0),
                DVec3::new(100.0, 100.0, 100.0),
            )],
            spatial_revision: 0,
        });

        app.update();

        let plot = app.world().get::<LingtianPlot>(plot_entity).unwrap();
        assert_eq!(
            plot.zone, "spawn_zone",
            "tick 2: registry present → zone must be back-filled (got {:?})",
            plot.zone
        );
    }

    #[test]
    fn auto_set_plot_zone_retries_when_existing_registry_changes() {
        let mut app = App::new();
        app.insert_resource(ZoneRegistry {
            zones: Vec::new(),
            spatial_revision: 0,
        });
        app.init_resource::<PendingPlotZones>();
        app.add_systems(Update, auto_set_plot_zone);

        let plot_entity = app
            .world_mut()
            .spawn(LingtianPlot::new(BlockPos::new(50, 64, 50), None))
            .id();
        app.update();
        assert!(
            app.world()
                .get::<LingtianPlot>(plot_entity)
                .unwrap()
                .zone
                .is_empty(),
            "empty existing registry must leave the plot pending"
        );

        app.world_mut()
            .resource_mut::<ZoneRegistry>()
            .zones
            .push(zone_named(
                "added_zone",
                DVec3::new(0.0, 0.0, 0.0),
                DVec3::new(100.0, 100.0, 100.0),
            ));
        app.world_mut()
            .resource_mut::<ZoneRegistry>()
            .spatial_revision = 1;
        app.update();

        assert_eq!(
            app.world().get::<LingtianPlot>(plot_entity).unwrap().zone,
            "added_zone",
            "in-place ZoneRegistry mutation must retry unresolved plots"
        );
    }

    #[test]
    fn auto_set_plot_zone_does_not_retry_history_for_each_new_plot() {
        let mut app = App::new();
        app.insert_resource(ZoneRegistry {
            zones: vec![zone_named(
                "registry_zone",
                DVec3::new(0.0, 0.0, 0.0),
                DVec3::new(100.0, 100.0, 100.0),
            )],
            spatial_revision: 0,
        });
        app.init_resource::<PendingPlotZones>();
        app.add_systems(Update, auto_set_plot_zone);

        let unresolved = app
            .world_mut()
            .spawn(LingtianPlot::new(BlockPos::new(500, 64, 500), None))
            .id();
        app.update();
        app.world_mut()
            .get_mut::<LingtianPlot>(unresolved)
            .unwrap()
            .pos = BlockPos::new(50, 64, 50);

        let new_plot = app
            .world_mut()
            .spawn(LingtianPlot::new(BlockPos::new(60, 64, 60), None))
            .id();
        app.update();

        assert!(
            app.world()
                .get::<LingtianPlot>(unresolved)
                .unwrap()
                .zone
                .is_empty(),
            "a new plot must not trigger a rescan of historical unresolved entries"
        );
        assert_eq!(
            app.world().get::<LingtianPlot>(new_plot).unwrap().zone,
            "registry_zone",
            "the newly added plot must still resolve immediately"
        );
    }

    #[test]
    fn auto_set_plot_zone_does_not_overwrite_existing_zone() {
        use crate::lingtian::plot::LingtianPlot;
        use crate::world::zone::ZoneRegistry;

        let mut app = App::new();
        app.insert_resource(ZoneRegistry {
            zones: vec![zone_named(
                "registry_zone",
                DVec3::new(0.0, 0.0, 0.0),
                DVec3::new(100.0, 100.0, 100.0),
            )],
            spatial_revision: 0,
        });
        app.init_resource::<PendingPlotZones>();
        app.add_systems(Update, auto_set_plot_zone);

        // Pre-set plot zone — system must NOT overwrite it (idempotent).
        let plot_entity = app
            .world_mut()
            .spawn(LingtianPlot::new(BlockPos::new(50, 64, 50), None).with_zone("explicit_zone"))
            .id();

        app.update();
        app.update();

        let plot = app.world().get::<LingtianPlot>(plot_entity).unwrap();
        assert_eq!(
            plot.zone, "explicit_zone",
            "system must not overwrite a non-empty zone field"
        );
    }
}
