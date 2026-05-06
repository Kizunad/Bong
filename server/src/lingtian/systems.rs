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

use std::collections::HashMap;

use valence::prelude::bevy_ecs::system::SystemParam;
use valence::prelude::{
    bevy_ecs, BlockState, ChunkLayer, Client, Commands, DVec3, Entity, EventReader, EventWriter,
    Events, Query, Res, ResMut, Resource, Username, With,
};

use crate::alchemy::residue::{consume_one_residue, inventory_has_usable_residue};
use crate::botany::{PlantId, PlantKindRegistry};
use crate::combat::events::DeathEvent;
use crate::combat::CombatClock;
use crate::cultivation::components::Cultivation;
use crate::cultivation::life_record::{BiographyEntry, LifeRecord};
use crate::inventory::{
    InventoryInstanceIdAllocator, ItemInstance, ItemRegistry, ItemTemplate, PlayerInventory,
    MAIN_PACK_CONTAINER_ID,
};
use crate::network::inventory_snapshot_emit::send_inventory_snapshot_to_client;
use crate::npc::spawn::NpcMarker;
use crate::player::state::{canonical_player_id, PlayerState};
use crate::schema::common::GameEventType;
use crate::schema::world_state::GameEvent;
use crate::skill::components::SkillId;
use crate::skill::events::{SkillXpGain, XpGainSource};
use crate::world::events::ActiveEventsResource;

use super::contamination::{apply_dye_contamination_on_replenish, dye_contamination_decay_tick};
use super::environment::compute_plot_qi_cap;
use super::events::{
    DrainQiCompleted, DyeContaminationWarning, HarvestCompleted, PlantingCompleted, RenewCompleted,
    ReplenishCompleted, StartDrainQiRequest, StartHarvestRequest, StartPlantingRequest,
    StartRenewRequest, StartReplenishRequest, StartTillRequest, TillCompleted, ZonePressureCrossed,
};
use super::growth::advance_one_lingtian_tick;
use super::hoe::HoeKind;
use super::network_emit::replenish_source_wire;
use super::plot::{CropInstance, LingtianPlot};
use super::pressure::{compute_zone_pressure, PressureLevel, ZonePressureTracker};
use super::qi_account::{
    LingtianTickAccumulator, ZoneQiAccount, BEVY_TICKS_PER_LINGTIAN_TICK, DEFAULT_ZONE,
};
use super::seed::{seed_id_for, SeedRegistry};
use super::session::{
    DrainQiSession, HarvestSession, PlantingSession, RenewSession, ReplenishSession,
    ReplenishSource, TillSession, DRAIN_QI_TO_PLAYER_RATIO, DRAIN_QI_TO_ZONE_RATIO,
    REPLENISH_COOLDOWN_LINGTIAN_TICKS,
};
use super::terrain::classify_for_till;
use crate::world::events::EVENT_REALM_COLLAPSE;
use crate::world::zone::ZoneRegistry;

const LING_SHUI_ITEM_ID: &str = "ling_shui";
const BEAST_CORE_ITEM_ID: &str = "mutant_beast_core";

const MAIN_HAND_SLOT: &str = "main_hand";

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
}

/// 累计的 lingtian-tick（lingtian_growth_tick 触发时 ++）。用于补灵冷却比对。
#[derive(Debug, Default, Resource)]
pub struct LingtianClock {
    pub lingtian_tick: u64,
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

    /// 插入新 session。若该 actor 已有则返回 false，调用方丢弃请求。
    pub fn try_insert(&mut self, actor: Entity, session: ActiveSession) -> bool {
        if self.by_actor.contains_key(&actor) {
            return false;
        }
        self.by_actor.insert(actor, session);
        true
    }

    /// 清掉某 actor 的 session（cancel / 完成结算后）。
    pub fn clear(&mut self, actor: Entity) -> Option<ActiveSession> {
        self.by_actor.remove(&actor)
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
    let item = inventory.equipped.get(MAIN_HAND_SLOT)?;
    let kind = HoeKind::from_item_id(&item.template_id)?;
    Some((kind, item.instance_id))
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
        let barren = plots.iter().any(|p| p.pos == req.pos && p.is_barren());
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
) {
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
        let target_ok = plots
            .iter()
            .any(|p| p.pos == req.pos && p.is_empty() && !p.is_barren());
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
) {
    for req in events.read() {
        if sessions.has_session(req.player) {
            tracing::warn!(
                "[bong][lingtian] StartDrainQiRequest rejected: player={:?} already has active session",
                req.player
            );
            continue;
        }
        let exists_with_qi = plots.iter().any(|p| p.pos == req.pos && p.plot_qi > 0.0);
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
) {
    for req in events.read() {
        if sessions.has_session(req.player) {
            tracing::warn!(
                "[bong][lingtian] StartHarvestRequest rejected: player={:?} already has active session",
                req.player
            );
            continue;
        }
        let plant_id = plots
            .iter()
            .find(|p| p.pos == req.pos)
            .and_then(|p| p.crop.as_ref())
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

pub fn handle_start_replenish(
    mut events: EventReader<StartReplenishRequest>,
    mut sessions: ResMut<ActiveLingtianSessions>,
    time: LingtianTime,
    inventories: Query<&PlayerInventory>,
    plots: Query<&LingtianPlot>,
    zone_qi: Res<ZoneQiAccount>,
) {
    let residue_tick = time.residue_tick();
    for req in events.read() {
        if sessions.has_session(req.player) {
            tracing::warn!(
                "[bong][lingtian] StartReplenishRequest rejected: player={:?} already has active session",
                req.player
            );
            continue;
        }
        let Some(plot) = plots.iter().find(|p| p.pos == req.pos) else {
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
            ReplenishSource::Zone => zone_qi.get(DEFAULT_ZONE) >= req.source.plot_qi_amount(),
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
    time: LingtianTime,
    mut writers: CompletionEventWriters,
    mut layers: Query<&mut ChunkLayer, With<crate::world::dimension::OverworldLayer>>,
    mut skill_xp_events: Option<ResMut<Events<SkillXpGain>>>,
) {
    for (player, finished) in sessions.drain_finished() {
        match finished {
            ActiveSession::Till(s) => {
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
                    emit_lingtian_skill_xp(&mut skill_xp_events, player, 2, "renew");
                } else {
                    tracing::warn!(
                        "[bong][lingtian] RenewSession finished but plot at {:?} vanished",
                        s.pos
                    );
                }
            }
            ActiveSession::Planting(s) => {
                apply_planting_completion(
                    player,
                    &s.pos,
                    &s.plant_id,
                    &mut inventories,
                    &mut plots,
                    &seeds,
                    &mut writers.planting,
                    &mut skill_xp_events,
                );
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
                    time.lingtian_tick(),
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
                let residue_tick = time.residue_tick();
                apply_replenish_completion(
                    player,
                    &s.pos,
                    s.source,
                    &mut inventories,
                    &mut plots,
                    &mut zone_qi,
                    time.lingtian_tick(),
                    residue_tick,
                    &mut harvest_rng,
                    &mut writers.replenish,
                    &mut writers.dye_warning,
                    &mut skill_xp_events,
                );
            }
            ActiveSession::DrainQi(s) => {
                apply_drain_qi_completion(
                    player,
                    &s.pos,
                    &mut plots,
                    &mut cultivations,
                    &mut life_records,
                    &mut zone_qi,
                    time.lingtian_tick(),
                    &mut writers.drain_qi,
                );
            }
        }
    }
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
    mut deaths: EventReader<DeathEvent>,
    dead_npcs: Query<(), With<NpcMarker>>,
    mut plots: Query<&mut LingtianPlot>,
) {
    for death in deaths.read() {
        if dead_npcs.get(death.target).is_err() {
            continue;
        }

        for mut plot in &mut plots {
            if plot.owner == Some(death.target) {
                plot.owner = None;
            }
        }
    }
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
) {
    let Some(seed_id) = seeds.seed_for_plant(plant_id).cloned() else {
        tracing::warn!(
            "[bong][lingtian] PlantingSession finished but plant_id={} no longer in SeedRegistry",
            plant_id
        );
        return;
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
        return;
    };
    if let Some(inv) = inventory.as_deref_mut() {
        if !consume_one_seed(inv, &seed_id) {
            tracing::warn!(
                "[bong][lingtian] PlantingSession finished but seed `{seed_id}` no longer in inventory"
            );
            return;
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
        let Some(plant_item_template) = item_registry.get(plant_id) else {
            tracing::warn!(
                "[bong][lingtian] no ItemTemplate for plant_id={plant_id} (need entry in herbs.toml)"
            );
            return;
        };
        if let Some(inv) = inventory.as_deref_mut() {
            if !award_item_to_inventory(inv, plant_item_template, allocator) {
                tracing::warn!(
                    "[bong][lingtian] inventory full; dropped 1× {plant_id} for actor={actor:?}"
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
            if let (Some(seed_template), Some(inv)) =
                (item_registry.get(&seed_id), inventory.as_deref_mut())
            {
                if !award_item_to_inventory(inv, seed_template, allocator) {
                    tracing::warn!(
                        "[bong][lingtian] inventory full; dropped 1× {seed_id} for actor={actor:?}"
                    );
                }
                true
            } else if inventory.is_some() {
                tracing::warn!(
                    "[bong][lingtian] no ItemTemplate for seed `{seed_id}` (need entry in seeds.toml)"
                );
                false
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

/// 把一个 1×1 item 加到玩家背包。
///
/// 策略（最简）：
///   1. 在 `main_pack` 里找同 template_id 的栈 → stack += 1
///   2. 否则在 `main_pack` 里找首个空 (row, col) 1×1 槽 → spawn 新 instance（allocator 给 id）
///   3. 没空位 → 返回 false（调用方 warn 丢弃）
///
/// 仅支持 1×1 item（herbs / seeds 是 1×1）。多格 item 走另一路径（未实装，
/// 留 P5+ 通用 inventory placement helper）。
fn award_item_to_inventory(
    inv: &mut PlayerInventory,
    template: &ItemTemplate,
    allocator: &mut InventoryInstanceIdAllocator,
) -> bool {
    if template.grid_w != 1 || template.grid_h != 1 {
        tracing::warn!(
            "[bong][lingtian] award_item_to_inventory: only 1×1 supported (template={} is {}×{})",
            template.id,
            template.grid_w,
            template.grid_h,
        );
        return false;
    }
    let Some(main_pack) = inv
        .containers
        .iter_mut()
        .find(|c| c.id == MAIN_PACK_CONTAINER_ID)
    else {
        tracing::warn!("[bong][lingtian] award_item_to_inventory: no main_pack container");
        return false;
    };

    // 1. stack
    if let Some(slot) = main_pack
        .items
        .iter_mut()
        .find(|p| p.instance.template_id == template.id)
    {
        slot.instance.stack_count = slot.instance.stack_count.saturating_add(1);
        bump_revision(inv);
        return true;
    }

    // 2. 找首个空 (row, col)
    let (rows, cols) = (main_pack.rows, main_pack.cols);
    let mut occupied = vec![vec![false; usize::from(cols)]; usize::from(rows)];
    for placed in &main_pack.items {
        for dr in 0..placed.instance.grid_h {
            for dc in 0..placed.instance.grid_w {
                let r = usize::from(placed.row + dr);
                let c = usize::from(placed.col + dc);
                if r < occupied.len() && c < occupied[0].len() {
                    occupied[r][c] = true;
                }
            }
        }
    }
    for r in 0..rows {
        for c in 0..cols {
            if !occupied[usize::from(r)][usize::from(c)] {
                let Ok(instance_id) = allocator.next_id() else {
                    tracing::warn!(
                        "[bong][lingtian] award_item_to_inventory: instance_id allocator exhausted"
                    );
                    return false;
                };
                let instance = ItemInstance {
                    instance_id,
                    template_id: template.id.clone(),
                    display_name: template.display_name.clone(),
                    grid_w: template.grid_w,
                    grid_h: template.grid_h,
                    weight: template.base_weight,
                    rarity: template.rarity,
                    description: template.description.clone(),
                    stack_count: 1,
                    spirit_quality: template.spirit_quality_initial,
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
                };
                main_pack.items.push(crate::inventory::PlacedItemState {
                    row: r,
                    col: c,
                    instance,
                });
                bump_revision(inv);
                return true;
            }
        }
    }
    false
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
) {
    let (plot_owner, drained, to_player, to_zone) = {
        let Some((_e, mut plot)) = plots.iter_mut().find(|(_, p)| &p.pos == pos) else {
            tracing::warn!("[bong][lingtian] DrainQiSession finished but plot at {pos:?} vanished");
            return;
        };
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
        (owner, drained, to_player, to_zone)
    };

    // 注入操作者 cultivation.qi_current（cap at qi_max）
    if let Ok(mut cult) = cultivations.get_mut(player) {
        let room = (cult.qi_max - cult.qi_current).max(0.0);
        cult.qi_current += (to_player as f64).min(room);
    }
    // 散逸 zone qi
    *zone_qi.get_mut(DEFAULT_ZONE) += to_zone;

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
        qi_to_player: to_player,
        qi_to_zone: to_zone,
    });
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
) {
    let Some((_e, mut plot)) = plots.iter_mut().find(|(_, p)| &p.pos == pos) else {
        tracing::warn!("[bong][lingtian] ReplenishSession finished but plot at {pos:?} vanished");
        return;
    };

    // 复验 / 扣材料：plan §1.4 来源材料**不退**，若 session 期间被消耗也照付
    let amount = source.plot_qi_amount();
    let zone_key = DEFAULT_ZONE;
    let mut paid = true;
    match source {
        ReplenishSource::Zone => {
            let z = zone_qi.get_mut(zone_key);
            if *z >= amount {
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
        return;
    }

    // 注入 plot_qi，溢出回馈 zone（plan §1.4）
    let cap_room = (plot.plot_qi_cap - plot.plot_qi).max(0.0);
    let added = amount.min(cap_room);
    let overflow = amount - added;
    plot.plot_qi += added;
    if overflow > 0.0 {
        // 溢出回馈：Zone source 自身的 overflow 也回馈（plan 没明说 zone 来源
        // 是否例外，本切片按"统一回馈环境"处理）
        let z = zone_qi.get_mut(zone_key);
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
) {
    let Some(active_events) = active_events.as_deref_mut() else {
        for _ in events.read() {}
        return;
    };

    for event in events.read() {
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
            zone: Some(DEFAULT_ZONE.to_string()),
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
    let Some(item) = inventory.equipped.get_mut(MAIN_HAND_SLOT) else {
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
        inventory.equipped.remove(MAIN_HAND_SLOT);
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
/// zone 解析当前简化为 `DEFAULT_ZONE`（plan §1.3 注释：world::zone 真挂接
/// 留 P3+，与 plan-zhenfa-v1 / WorldQiAccount 整合）；若当前 world zone 已域崩，
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
/// 本系统单 zone 简化：全部 plot 算作 `DEFAULT_ZONE`。
pub fn record_replenish_to_pressure(
    mut events: EventReader<ReplenishCompleted>,
    clock: Res<LingtianClock>,
    mut tracker: ResMut<ZonePressureTracker>,
) {
    for e in events.read() {
        let total = e.plot_qi_added + e.overflow_to_zone;
        tracker
            .state_mut(DEFAULT_ZONE)
            .record_replenish(clock.lingtian_tick, total);
    }
}

/// plan §5.1 — 每 lingtian-tick 后（通过读 `LingtianTickAccumulator` 刚归零）
/// 重算 zone pressure、prune 7d 窗口、跨档上升时发 `ZonePressureCrossed`
/// 事件；HIGH 进入时清 zone 所有 plot_qi（道伥 spawn 由下游 npc 系统接）。
pub fn compute_zone_pressure_system(
    accumulator: Res<LingtianTickAccumulator>,
    clock: Res<LingtianClock>,
    mut tracker: ResMut<ZonePressureTracker>,
    registry: Res<PlantKindRegistry>,
    mut plots: Query<&mut LingtianPlot>,
    mut events: EventWriter<ZonePressureCrossed>,
) {
    // 与 lingtian_growth_tick 同节拍：accumulator 刚在同一 Update 归零
    // → 本 tick 刚跑过一 lingtian-tick，现在是对齐点。
    if accumulator.raw() != 0 {
        return;
    }
    let zone = DEFAULT_ZONE.to_string();
    let now = clock.lingtian_tick;
    tracker.state_mut(&zone).prune(now);

    // 借用拆分：读出 pressure 先丢作用域，再改 state
    let pressure = {
        let plots_iter = plots.iter().map(|m| -> &LingtianPlot { m });
        compute_zone_pressure(&zone, plots_iter, &registry, &tracker)
    };
    let new_level = PressureLevel::classify(pressure);
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
            // plan §5.1 — HIGH 触发 zone plot_qi 瞬时清零
            for mut plot in plots.iter_mut() {
                plot.plot_qi = 0.0;
            }
            tracing::warn!(
                "[bong][lingtian] zone `{zone}` pressure HIGH (raw={pressure:.3}); cleared plot_qi"
            );
        }
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
    let zone_qi_ref = zone_qi.get_mut(DEFAULT_ZONE);
    advance_one_lingtian_tick(plot, kind, zone_qi_ref);
}

fn plot_zone_is_collapsed(plot: &LingtianPlot, zone_registry: Option<&ZoneRegistry>) -> bool {
    let Some(zone_registry) = zone_registry else {
        return false;
    };
    let plot_pos = DVec3::new(
        plot.pos.x as f64 + 0.5,
        plot.pos.y as f64,
        plot.pos.z as f64 + 0.5,
    );
    zone_registry
        .find_zone(crate::world::dimension::DimensionKind::Overworld, plot_pos)
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
    use crate::inventory::{InventoryRevision, ItemInstance, ItemRarity, PlayerInventory};
    use crate::npc::spawn::NpcMarker;
    use std::collections::HashMap;
    use valence::prelude::{App, BlockPos, DVec3, IntoSystemConfigs, Update};

    use super::super::events::{
        RenewCompleted, StartRenewRequest, StartTillRequest, TillCompleted,
    };
    use super::super::hoe::HoeKind;
    use super::super::session::{SessionMode, RENEW_TICKS, TILL_MANUAL_TICKS};
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
            .insert_resource(SeedRegistry::new())
            .insert_resource(PlantKindRegistry::new())
            .insert_resource(ItemRegistry::default())
            .insert_resource(InventoryInstanceIdAllocator::default())
            .insert_resource(LingtianHarvestRng::default())
            .insert_resource(ZoneQiAccount::new())
            .insert_resource(LingtianClock::default())
            .insert_resource(CombatClock::default())
            .insert_resource(ActiveEventsResource::default())
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
            .add_event::<SkillXpGain>()
            .add_systems(
                Update,
                (
                    handle_start_till,
                    handle_start_renew,
                    handle_start_harvest,
                    handle_start_replenish,
                    handle_start_drain_qi,
                    tick_lingtian_sessions,
                    apply_completed_sessions,
                    record_dye_contamination_warning_recent_events,
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
        let player = app
            .world_mut()
            .spawn(make_inventory_with_hoe(HoeKind::Iron, 1.0))
            .id();
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
            ItemInstance {
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
            },
        );
        let inv = PlayerInventory {
            revision: InventoryRevision(0),
            containers: vec![],
            equipped,
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 45.0,
        };
        assert!(equipped_main_hand_hoe(&inv).is_none());
    }

    #[test]
    fn release_lingtian_plot_owner_on_npc_death_clears_npc_owner() {
        let mut app = App::new();
        app.add_event::<DeathEvent>();
        app.add_systems(Update, release_lingtian_plot_owner_on_npc_death);

        let owner = app.world_mut().spawn(NpcMarker).id();
        let plot = app
            .world_mut()
            .spawn(LingtianPlot::new(BlockPos::new(1, 64, 1), Some(owner)))
            .id();

        app.world_mut().send_event(DeathEvent {
            target: owner,
            cause: "test".into(),
            attacker: None,
            attacker_player_id: None,
            at_tick: 1,
        });
        app.update();

        assert_eq!(app.world().get::<LingtianPlot>(plot).unwrap().owner, None);
    }

    #[test]
    fn release_lingtian_plot_owner_ignores_player_death() {
        let mut app = App::new();
        app.add_event::<DeathEvent>();
        app.add_systems(Update, release_lingtian_plot_owner_on_npc_death);

        let owner = app.world_mut().spawn_empty().id();
        let plot = app
            .world_mut()
            .spawn(LingtianPlot::new(BlockPos::new(1, 64, 1), Some(owner)))
            .id();

        app.world_mut().send_event(DeathEvent {
            target: owner,
            cause: "test".into(),
            attacker: None,
            attacker_player_id: None,
            at_tick: 1,
        });
        app.update();

        assert_eq!(
            app.world().get::<LingtianPlot>(plot).unwrap().owner,
            Some(owner)
        );
    }

    #[test]
    fn till_rejected_when_request_instance_id_mismatches_main_hand() {
        let mut app = build_app();
        let player = app
            .world_mut()
            .spawn(make_inventory_with_hoe(HoeKind::Iron, 1.0))
            .id();
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
        let player = app
            .world_mut()
            .spawn(make_inventory_with_hoe(HoeKind::Iron, 1.0))
            .id();
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
            hoe_instance_id: 1,
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
    use crate::lingtian::environment::{PlotBiome, PlotEnvironment};
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
            }],
        });
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
    use crate::lingtian::session::PLANTING_TICKS;

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
            id: "main_pack".into(),
            name: "main_pack".into(),
            rows: 4,
            cols: 4,
            items: vec![PlacedItemState {
                row: 0,
                col: 0,
                instance: make_seed_instance(template_id, stack),
            }],
        };
        PlayerInventory {
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
            .add_event::<SkillXpGain>()
            .add_systems(
                Update,
                (
                    handle_start_till,
                    handle_start_renew,
                    handle_start_planting,
                    handle_start_harvest,
                    handle_start_replenish,
                    handle_start_drain_qi,
                    tick_lingtian_sessions,
                    apply_completed_sessions,
                )
                    .chain(),
            );
        app
    }

    #[test]
    fn planting_e2e_spawns_crop_and_consumes_seed() {
        let mut app = build_planting_app();
        let player = app
            .world_mut()
            .spawn(make_inventory_with_seed("ci_she_hao_seed", 5))
            .id();
        let pos = BlockPos::new(0, 64, 0);
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
        let player = app
            .world_mut()
            .spawn(make_inventory_with_seed("ning_mai_cao_seed", 1))
            .id();
        let pos = BlockPos::new(1, 64, 1);
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
        let player = app
            .world_mut()
            .spawn(make_inventory_with_seed("ci_she_hao_seed", 1))
            .id();
        let pos = BlockPos::new(0, 64, 0);
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
        let player = app
            .world_mut()
            .spawn(make_inventory_with_seed("ci_she_hao_seed", 5))
            .id();
        let pos = BlockPos::new(0, 64, 0);
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
        let player = app
            .world_mut()
            .spawn(make_inventory_with_seed("ci_she_hao_seed", 5))
            .id();
        let pos = BlockPos::new(0, 64, 0);
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
        let player = app
            .world_mut()
            .spawn(make_inventory_with_seed("ci_she_hao_seed", 5))
            .id();
        let pos = BlockPos::new(0, 64, 0);
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
    use crate::lingtian::session::HARVEST_MANUAL_TICKS;

    fn herb_template(id: &str, display: &str) -> ItemTemplate {
        ItemTemplate {
            id: id.into(),
            display_name: display.into(),
            category: ItemCategory::Herb,
            max_stack_count: 1,
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
        }
    }

    fn seed_template(id: &str) -> ItemTemplate {
        ItemTemplate {
            id: id.into(),
            display_name: id.into(),
            category: ItemCategory::Misc,
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

    fn build_harvest_app() -> App {
        let mut app = App::new();
        let plant_registry = registry_with_three_test_plants();
        let seeds = SeedRegistry::from_plant_registry(&plant_registry);
        app.insert_resource(ActiveLingtianSessions::new())
            .insert_resource(plant_registry)
            .insert_resource(seeds)
            .insert_resource(registry_with_herb_and_seed_templates())
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
            .add_event::<SkillXpGain>()
            .add_systems(
                Update,
                (
                    handle_start_till,
                    handle_start_renew,
                    handle_start_planting,
                    handle_start_harvest,
                    handle_start_replenish,
                    handle_start_drain_qi,
                    tick_lingtian_sessions,
                    apply_completed_sessions,
                )
                    .chain(),
            );
        app
    }

    fn empty_inventory_8x8() -> PlayerInventory {
        let main_pack = ContainerState {
            id: MAIN_PACK_CONTAINER_ID.into(),
            name: "main".into(),
            rows: 8,
            cols: 8,
            items: vec![],
        };
        PlayerInventory {
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

    #[test]
    fn harvest_e2e_drops_plant_and_clears_plot() {
        let mut app = build_harvest_app();
        let player = app.world_mut().spawn(empty_inventory_8x8()).id();
        let pos = BlockPos::new(2, 64, 2);
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
    fn harvest_rejected_when_crop_not_ripe() {
        let mut app = build_harvest_app();
        let player = app.world_mut().spawn(empty_inventory_8x8()).id();
        let pos = BlockPos::new(0, 64, 0);
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
        let player = app.world_mut().spawn(empty_inventory_8x8()).id();
        let pos = BlockPos::new(0, 64, 0);
        app.world_mut().spawn(LingtianPlot::new(pos, None));
        app.world_mut().send_event(StartHarvestRequest {
            player,
            pos,
            mode: SessionMode::Manual,
        });
        app.update();
        assert!(app.world().resource::<ActiveLingtianSessions>().is_empty());
    }

    #[test]
    fn five_harvests_make_plot_barren() {
        let mut app = build_harvest_app();
        let player = app.world_mut().spawn(empty_inventory_8x8()).id();
        let pos = BlockPos::new(3, 64, 3);
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
        let player = app.world_mut().spawn(inv).id();
        let pos = BlockPos::new(0, 64, 0);
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
        let player = app.world_mut().spawn(empty_inventory_8x8()).id();
        let pos = BlockPos::new(0, 64, 0);
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
            .add_event::<SkillXpGain>()
            .add_systems(
                Update,
                (
                    handle_start_harvest,
                    tick_lingtian_sessions,
                    apply_completed_sessions,
                )
                    .chain(),
            );

        let player = app.world_mut().spawn(empty_inventory_8x8()).id();
        let pos = BlockPos::new(0, 64, 0);
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
        let mut p = LingtianPlot::new(pos, None);
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
        let player = app.world_mut().spawn(empty_inventory_8x8()).id();
        let pos = BlockPos::new(0, 64, 0);
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
    fn replenish_bone_coin_consumes_one_coin_and_adds_0_8() {
        let mut app = build_app();
        let player = app
            .world_mut()
            .spawn(make_inventory_with_bone_coins(3))
            .id();
        let pos = BlockPos::new(0, 64, 0);
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
        let player = app
            .world_mut()
            .spawn(make_inventory_with_misc_stack("mutant_beast_core", 1))
            .id();
        let pos = BlockPos::new(0, 64, 0);
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
    fn replenish_ling_shui_consumes_one_bottle() {
        let mut app = build_app();
        let player = app
            .world_mut()
            .spawn(make_inventory_with_misc_stack("ling_shui", 2))
            .id();
        let pos = BlockPos::new(0, 64, 0);
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
            let player = app
                .world_mut()
                .spawn(make_inventory_with_residue(kind, 0, 1))
                .id();
            let pos = BlockPos::new(0, 64, 0);
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
        let player = app
            .world_mut()
            .spawn((
                Username("Azure".to_string()),
                make_inventory_with_residue(kind, 0, 1),
            ))
            .id();
        let pos = BlockPos::new(0, 64, 0);
        let plot = spawn_empty_plot(&mut app, pos);
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
        assert_eq!(events[0].zone.as_deref(), Some(DEFAULT_ZONE));
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
        let player = app
            .world_mut()
            .spawn(make_inventory_with_residue(kind, 10, 1))
            .id();
        app.world_mut().resource_mut::<CombatClock>().tick =
            10 + crate::alchemy::residue::PILL_RESIDUE_TTL_TICKS;
        let pos = BlockPos::new(0, 64, 0);
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
        let player = app.world_mut().spawn(empty_inventory_8x8()).id();
        let pos = BlockPos::new(0, 64, 0);
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
        let player = app
            .world_mut()
            .spawn(make_inventory_with_bone_coins(2))
            .id();
        let pos = BlockPos::new(0, 64, 0);
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
        let player = app
            .world_mut()
            .spawn(make_inventory_with_bone_coins(2))
            .id();
        let pos = BlockPos::new(0, 64, 0);
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
        let player = app
            .world_mut()
            .spawn(make_inventory_with_hoe(HoeKind::Iron, 1.0))
            .id();
        app.world_mut().send_event(StartTillRequest {
            player,
            pos: BlockPos::new(0, 64, 0),
            hoe_instance_id: 1,
            mode: SessionMode::Manual,
            terrain: TerrainKind::Grass,
            environment: PlotEnvironment {
                water_adjacent: true,
                biome: PlotBiome::Wetland,
                zhenfa_jvling: true,
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
    fn till_default_environment_keeps_cap_at_1_0() {
        let mut app = build_app();
        let player = app
            .world_mut()
            .spawn(make_inventory_with_hoe(HoeKind::Iron, 1.0))
            .id();
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
        assert!((plot.plot_qi_cap - 1.0).abs() < 1e-6);
    }

    // ------------------------------------------------------------------------
    // §5.1 密度阈值 e2e
    // ------------------------------------------------------------------------

    use crate::lingtian::pressure::{
        PressureLevel as PL, PRESSURE_HIGH, PRESSURE_LOW, PRESSURE_MID,
    };

    fn build_pressure_app(natural_supply: f32) -> App {
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
        app.insert_resource(LingtianTickAccumulator::new())
            .insert_resource(LingtianClock::default())
            .insert_resource(ZoneQiAccount::new())
            .insert_resource(plant_registry)
            .insert_resource(tracker)
            .add_event::<ReplenishCompleted>()
            .add_event::<StartDrainQiRequest>()
            .add_event::<DrainQiCompleted>()
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
        for i in 0..n {
            let mut p = LingtianPlot::new(BlockPos::new(i as i32, 64, 0), owner);
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
    fn natural_supply_offsets_demand() {
        let mut app = build_pressure_app(0.5);
        spawn_high_cost_planted(&mut app, 50); // demand 0.6
        step_one_lingtian_tick(&mut app);
        let tracker = app.world().resource::<ZonePressureTracker>();
        let p = tracker.state(DEFAULT_ZONE).unwrap().last_pressure;
        assert!((p - 0.1).abs() < 1e-3);
        assert_eq!(tracker.state(DEFAULT_ZONE).unwrap().last_level, PL::None);
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
    fn spawn_player_with_lifelog(app: &mut App, character_id: &str) -> Entity {
        let inv = empty_inventory_8x8();
        let lr = LifeRecord::new(character_id);
        app.world_mut().spawn((inv, lr)).id()
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
        let player = spawn_player_with_lifelog(&mut app, "alice");
        let pos = BlockPos::new(0, 64, 0);
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
        let owner = spawn_player_with_lifelog(&mut app, "alice");
        let thief = spawn_player_with_lifelog(&mut app, "bob");
        let pos = BlockPos::new(3, 64, 7);
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
        let thief = app
            .world_mut()
            .spawn((empty_inventory_8x8(), LifeRecord::new("bob"), thief_cult))
            .id();
        let pos = BlockPos::new(0, 64, 0);
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
        let cult = Cultivation {
            qi_current: 99.0,
            qi_max: 100.0,
            ..Default::default()
        };
        let player = app
            .world_mut()
            .spawn((empty_inventory_8x8(), LifeRecord::new("p"), cult))
            .id();
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
    }

    #[test]
    fn drain_qi_rejected_on_empty_plot() {
        let mut app = build_app();
        let player = app
            .world_mut()
            .spawn((empty_inventory_8x8(), LifeRecord::new("p")))
            .id();
        let pos = BlockPos::new(0, 64, 0);
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
        let player = spawn_player_with_lifelog(&mut app, "wanderer");
        let pos = BlockPos::new(0, 64, 0);
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
}
