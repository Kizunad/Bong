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
//! 而非真正的 valence BlockEntity（后者依 plan-persistence-v1）。Renew 在准入时
//! 绑定匹配的 plot Entity，结算时按实体复验，避免 session 期间位置复用导致误操作。

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

fn build_start_plot_entity_index<'a>(
    plots: impl Iterator<Item = (Entity, &'a LingtianPlot)>,
) -> HashMap<BlockPos, Vec<(Entity, &'a LingtianPlot)>> {
    plots.fold(HashMap::new(), |mut index, (entity, plot)| {
        index.entry(plot.pos).or_default().push((entity, plot));
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
    plots: Query<(Entity, &LingtianPlot)>,
) {
    if events.is_empty() {
        return;
    }
    // central review 1984-31332727941 finding [4] — 与 handle_start_till 同款：
    // 每批请求快照一次 plot 位置索引；空闲 tick 不扫描，批内不做二次方扫描。
    let plot_positions = build_start_plot_entity_index(plots.iter());
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
        let Some(plot_entity) = plot_positions.get(&req.pos).and_then(|plots| {
            plots
                .iter()
                .find(|(_, plot)| plot.is_barren())
                .map(|(entity, _)| *entity)
        }) else {
            tracing::warn!(
                "[bong][lingtian] StartRenewRequest rejected: no barren plot at {:?}",
                req.pos
            );
            continue;
        };
        let session = RenewSession::new(req.pos, plot_entity, kind, instance_id);
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
                let renewed = if let Ok((_entity, mut plot)) = plots.get_mut(s.plot_entity) {
                    if plot.pos == s.pos && plot.is_barren() {
                        plot.renew();
                        true
                    } else {
                        false
                    }
                } else {
                    false
                };
                if renewed {
                    if let Ok(mut inv) = inventories.get_mut(player) {
                        wear_main_hand_hoe(&mut inv, s.hoe, s.hoe_instance_id);
                    }
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
        let Some((_e, mut plot)) = plots
            .iter_mut()
            .find(|(_, p)| &p.pos == pos && p.plot_qi > 0.0)
        else {
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
#[path = "systems_tests.rs"]
mod tests;
