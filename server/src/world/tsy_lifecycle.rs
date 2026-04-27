//! plan-tsy-lifecycle-v1 §1–§6 — 活坍缩渊（TSY）生命周期与道伥转化。
//!
//! 状态机：
//! ```text
//!   New → Active → Declining → Collapsing → Dead
//! ```
//!
//! - **New**：family 已注册但还没人发现（spawn 后 + 首次入场前）
//! - **Active**：首位玩家进入；常驻状态
//! - **Declining**：剩余骨架 < 初始 50% → 灵压加深，更致命
//! - **Collapsing**：所有骨架被取走 → 30 秒倒计时窗口，灵压翻倍
//! - **Dead**：倒计时结束 → 三层 subzone 从 registry 移除，剩余玩家强制弹回主世界
//!
//! Tick 依赖序：
//! ```text
//!   tsy_loot_spawn_on_enter (P1)
//!     ↓ family 自动注册（first enter）
//!   tsy_lifecycle_tick (本 plan)
//!     ↓ remaining_skeleton 同步 + 状态机
//!   tsy_lifecycle_apply_spirit_qi (本 plan)
//!     ↓ 写回 Zone.spirit_qi
//!   tsy_drain_tick (P0)
//!     ↓ 抽真元
//!   tsy_corpse_to_daoxiang_tick (本 plan)
//!     ↓ 干尸转化
//!   tsy_collapse_completed_cleanup (本 plan)
//!     ↓ Dead 清理（玩家弹回 / loot 蒸发 / 道伥 50% 喷出）
//! ```

use std::collections::{HashMap, HashSet};

use bevy_transform::components::{GlobalTransform, Transform};
use valence::entity::zombie::ZombieEntityBundle;
use valence::math::DVec3;
use valence::prelude::{
    bevy_ecs, App, Commands, Entity, EntityKind, EntityLayerId, Event, EventReader, EventWriter,
    IntoSystemConfigs, Position, Query, Res, ResMut, Resource, Update,
};

use crate::combat::CombatClock;
use crate::inventory::ancient_relics::AncientRelicSource;
use crate::inventory::corpse::CorpseEmbalmed;
use crate::inventory::tsy_loot_spawn::source_class_from_family_id;
use crate::inventory::DroppedLootRegistry;
use crate::npc::lifecycle::{npc_runtime_bundle, NpcArchetype};
use crate::npc::spawn::NpcMarker;
use crate::world::dimension::{DimensionLayers, TsyLayer};
use crate::world::dimension_transfer::{DimensionTransferRequest, DimensionTransferSet};
use crate::world::tsy::{DimensionAnchor, TsyPresence};
use crate::world::zone::{TsyDepth, ZoneRegistry};

/// 塌缩窗口长度（30 秒 × 20 tick/s）。
pub const COLLAPSE_DURATION_TICKS: u64 = 30 * 20;

/// 干尸 → 道伥的自然累积阈值。MVP = 5 分钟。
pub const DAOXIANG_NATURAL_TICKS: u64 = 6_000;

/// 喷出主世界的概率（其余 50% 随 zone 一起 despawn）。
pub const DAOXIANG_EJECT_PROBABILITY_THOUSANDTH: u64 = 500;

/// 喷出落点的水平随机偏移上界（±10 格）。
const DAOXIANG_EJECT_OFFSET_RANGE: f64 = 10.0;

/// 道伥喷出主世界的 Y 偏移（落地点抬一格避免卡进方块）。
const DAOXIANG_EJECT_Y_OFFSET: f64 = 1.0;

/// TSY zone 的生命周期阶段。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TsyLifecycle {
    /// 已 spawn 但还没玩家发现。
    New,
    /// 至少有过一位玩家进入。
    Active,
    /// 剩余骨架 < 初始 50%，灵压加深。
    Declining,
    /// 所有骨架被取走，进入 30 秒塌缩窗口。
    Collapsing,
    /// 已经塌缩完成，subzone 已被移除，family_id 永久作废。
    Dead,
}

impl TsyLifecycle {
    /// 进入 Collapsing 后是否应当让 spirit_qi 翻倍（race-out 体感）。
    pub fn is_collapsing(self) -> bool {
        matches!(self, Self::Collapsing)
    }

    /// 是否已经死透（不再做任何 tick 处理）。
    pub fn is_dead(self) -> bool {
        matches!(self, Self::Dead)
    }
}

/// 单个 TSY family 的运行时状态。
///
/// 部分字段（`source_class` / `*_at_tick`）目前仅供 narration / 未来 IPC 消费，
/// 运行时 logic 不直接读 —— 用 `#[allow(dead_code)]` 抑制 warning，等 schema bridge
/// 落地后转为 schema 字段。
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct TsyZoneState {
    pub family_id: String,
    pub lifecycle: TsyLifecycle,
    pub source_class: AncientRelicSource,
    /// 初始 spawn 的 ancient relic instance_ids；P1 `tsy_loot_spawn_on_enter`
    /// 实际 spawn 后回写。空集合表示 spawn 阶段没有放下任何遗物（zone 未 ready），
    /// lifecycle_tick 在这种情况下不会推进 Declining/Collapsing。
    pub initial_skeleton: Vec<u64>,
    /// 当前还在 zone 内（即仍存在于 `DroppedLootRegistry.entries`）的骨架 instance_ids。
    pub remaining_skeleton: HashSet<u64>,
    pub created_at_tick: u64,
    pub activated_at_tick: Option<u64>,
    pub collapsing_started_at_tick: Option<u64>,
    pub dead_at_tick: Option<u64>,
    /// 主世界对应裂缝锚点（= 玩家入场时的 `presence.return_to`）。
    /// 用于塌缩瞬间把道伥喷回主世界（§6）。
    pub main_world_anchor: DimensionAnchor,
}

impl TsyZoneState {
    /// 已被取走的骨架数量（initial - remaining）。
    /// 测试 + 未来 inspect 命令使用；运行时 logic 走 `skeleton_ratio`。
    #[allow(dead_code)]
    pub fn taken_count(&self) -> usize {
        self.initial_skeleton
            .len()
            .saturating_sub(self.remaining_skeleton.len())
    }

    /// 剩余骨架比例（0.0 ~ 1.0）；空 initial 视为 1.0（避免除零）。
    pub fn skeleton_ratio(&self) -> f64 {
        if self.initial_skeleton.is_empty() {
            return 1.0;
        }
        self.remaining_skeleton.len() as f64 / self.initial_skeleton.len() as f64
    }
}

/// 全局 TSY family 状态表。
#[derive(Debug, Default, Resource)]
pub struct TsyZoneStateRegistry {
    pub by_family: HashMap<String, TsyZoneState>,
}

impl TsyZoneStateRegistry {
    /// 玩家首次进入触发；幂等：已存在 family 不重置状态。
    /// 首次写入并立即转为 Active（New 阶段太短，玩家进入即跨过）。
    /// 返回是否新建。
    pub fn ensure_active(
        &mut self,
        family_id: &str,
        source_class: AncientRelicSource,
        main_world_anchor: DimensionAnchor,
        tick: u64,
    ) -> bool {
        if let Some(existing) = self.by_family.get_mut(family_id) {
            // 已 dead 的 family 永远不复活；其他状态不动，仅补一次 activated_at_tick。
            if existing.lifecycle == TsyLifecycle::New {
                existing.lifecycle = TsyLifecycle::Active;
                existing.activated_at_tick = Some(tick);
            }
            return false;
        }
        self.by_family.insert(
            family_id.to_string(),
            TsyZoneState {
                family_id: family_id.to_string(),
                lifecycle: TsyLifecycle::Active,
                source_class,
                initial_skeleton: Vec::new(),
                remaining_skeleton: HashSet::new(),
                created_at_tick: tick,
                activated_at_tick: Some(tick),
                collapsing_started_at_tick: None,
                dead_at_tick: None,
                main_world_anchor,
            },
        );
        true
    }

    /// 接受 P1 loot spawn 回写的初始 skeleton id 列表。
    /// 仅在初始集合还为空时写入（避免 spawn 重入误把已减少的 skeleton 重置）。
    /// 返回是否真正写入。
    pub fn mark_initial_skeleton(&mut self, family_id: &str, ids: Vec<u64>) -> bool {
        let Some(state) = self.by_family.get_mut(family_id) else {
            return false;
        };
        if !state.initial_skeleton.is_empty() {
            return false;
        }
        state.remaining_skeleton = ids.iter().copied().collect();
        state.initial_skeleton = ids;
        true
    }

    /// family 是否处于 Collapsing 阶段（rift portal Collapsing-block 用）。
    pub fn is_collapsing(&self, family_id: &str) -> bool {
        self.by_family
            .get(family_id)
            .map(|s| s.lifecycle == TsyLifecycle::Collapsing)
            .unwrap_or(false)
    }

    /// family 是否已死（runtime portal idempotency 用）。
    pub fn is_dead(&self, family_id: &str) -> bool {
        self.by_family
            .get(family_id)
            .map(|s| s.lifecycle.is_dead())
            .unwrap_or(false)
    }
}

/// 通知"某 TSY 进入 Active"——预留给 agent IPC / narration / HUD pre-warm 用。
/// 当前 P0/P1/P2 内部不消费；schema bridge 由 §7.4 的 `TsyZoneActivatedV1` 接通。
#[allow(dead_code)]
#[derive(Event, Debug, Clone)]
pub struct TsyZoneActivated {
    pub family_id: String,
    pub source_class: AncientRelicSource,
    pub at_tick: u64,
}

/// "塌缩开始"信号。所有还在 zone 内的玩家 client 端会收到（HUD 倒计时）；
/// 同时 lifecycle_apply_spirit_qi 下一 tick 把 spirit_qi 翻倍。
///
/// 字段当前由 schema bridge / agent narration 消费（§7.4 后续 commit），
/// 运行时模块仅写不读 → 标 dead_code 防 -D warnings。
#[allow(dead_code)]
#[derive(Event, Debug, Clone)]
pub struct TsyCollapseStarted {
    pub family_id: String,
    pub at_tick: u64,
}

/// "塌缩完成"信号。`tsy_collapse_completed_cleanup` 消费此 event 做清理。
#[derive(Event, Debug, Clone)]
pub struct TsyCollapseCompleted {
    pub family_id: String,
    pub at_tick: u64,
}

/// 计算单层 spirit_qi。`skeleton_ratio` ∈ [0, 1]，0 = 全被取走（骨架空）。
///
/// 公式（plan §2.1）：base 给到原 §-1 设定的 -0.3 / -0.6 / -0.9；
/// 随骨架减少线性加深 depth_factor × (1 - ratio)；Collapsing 阶段额外 ×2。
pub fn compute_layer_spirit_qi(layer: TsyDepth, skeleton_ratio: f64, is_collapsing: bool) -> f64 {
    let base = match layer {
        TsyDepth::Shallow => -0.3,
        TsyDepth::Mid => -0.6,
        TsyDepth::Deep => -0.9,
    };
    let depth_factor = match layer {
        TsyDepth::Shallow => -0.3,
        TsyDepth::Mid => -0.4,
        TsyDepth::Deep => -0.3,
    };
    let ratio = skeleton_ratio.clamp(0.0, 1.0);
    let after_decay = base + depth_factor * (1.0 - ratio);
    let result = if is_collapsing {
        after_decay * 2.0
    } else {
        after_decay
    };
    // Zone.spirit_qi 校验区间是 [-1, 1]，clamp 防止下溢。
    result.clamp(-1.0, 1.0)
}

/// plan §1.4 — 状态机推进 + remaining_skeleton 同步。
///
/// 规则：
/// 1. Dead 不再处理。
/// 2. 同步 `remaining_skeleton = initial ∩ DroppedLootRegistry.entries.keys()`。
/// 3. Active/Declining 阶段：
///    - remaining 为空 → Collapsing（前提 initial 非空，否则视为"还没 spawn 完"）
///    - remaining < initial / 2 → Declining（不可逆方向，已 Declining 不回退）
/// 4. Collapsing 阶段：tick 超过 COLLAPSE_DURATION_TICKS → Dead，发 completed event。
pub fn tsy_lifecycle_tick(
    mut state_reg: ResMut<TsyZoneStateRegistry>,
    loot: Res<DroppedLootRegistry>,
    clock: Res<CombatClock>,
    mut emit_started: EventWriter<TsyCollapseStarted>,
    mut emit_completed: EventWriter<TsyCollapseCompleted>,
) {
    for state in state_reg.by_family.values_mut() {
        match state.lifecycle {
            TsyLifecycle::Dead => continue,
            TsyLifecycle::New => {
                // 玩家入场前；ensure_active 会推过 New，此处保留兜底。
            }
            TsyLifecycle::Active | TsyLifecycle::Declining => {
                // 同步 remaining：扫 initial，留下仍在 entries 里 *且* `source_container_id`
                // 仍是 `tsy_spawn:{family}` 的 id。
                //
                // 这一约束是必要的（Codex review P1）：玩家捡走 ancient relic 后再 discard
                // 回主世界，instance_id 仍存在于 `DroppedLootRegistry.entries`，但
                // `source_container_id` 已变成玩家 container（"main_pack" / "hotbar"），
                // 不再匹配 `tsy_spawn:` 前缀 —— 不算 remaining，状态机能正确推进 Collapsing。
                let prefix = format!("tsy_spawn:{}", state.family_id);
                let mut remaining = HashSet::with_capacity(state.initial_skeleton.len());
                for id in &state.initial_skeleton {
                    if let Some(entry) = loot.entries.get(id) {
                        if entry.source_container_id == prefix {
                            remaining.insert(*id);
                        }
                    }
                }
                state.remaining_skeleton = remaining;

                let total = state.initial_skeleton.len();
                if total == 0 {
                    // initial_skeleton 还没回写（spawn 失败 / zone 未 ready）→ 不推进。
                    continue;
                }

                if state.remaining_skeleton.is_empty() {
                    state.lifecycle = TsyLifecycle::Collapsing;
                    state.collapsing_started_at_tick = Some(clock.tick);
                    emit_started.send(TsyCollapseStarted {
                        family_id: state.family_id.clone(),
                        at_tick: clock.tick,
                    });
                } else if state.remaining_skeleton.len() * 2 < total
                    && state.lifecycle == TsyLifecycle::Active
                {
                    state.lifecycle = TsyLifecycle::Declining;
                }
            }
            TsyLifecycle::Collapsing => {
                let started = state.collapsing_started_at_tick.unwrap_or(clock.tick);
                let elapsed = clock.tick.saturating_sub(started);
                if elapsed >= COLLAPSE_DURATION_TICKS {
                    state.lifecycle = TsyLifecycle::Dead;
                    state.dead_at_tick = Some(clock.tick);
                    emit_completed.send(TsyCollapseCompleted {
                        family_id: state.family_id.clone(),
                        at_tick: clock.tick,
                    });
                }
            }
        }
    }
}

/// plan §2.2 — 把 lifecycle 状态映射回 `Zone.spirit_qi`。
///
/// Dead family 不修改对应 zone（zone 在 cleanup 中已被移除，找不到）；其他状态
/// 按 `compute_layer_spirit_qi(layer, ratio, is_collapsing)` 推一次。
pub fn tsy_lifecycle_apply_spirit_qi(
    state_reg: Res<TsyZoneStateRegistry>,
    mut zones: ResMut<ZoneRegistry>,
) {
    for state in state_reg.by_family.values() {
        if state.lifecycle.is_dead() {
            continue;
        }
        let ratio = state.skeleton_ratio();
        let is_collapsing = state.lifecycle.is_collapsing();
        for layer in [TsyDepth::Shallow, TsyDepth::Mid, TsyDepth::Deep] {
            let suffix = match layer {
                TsyDepth::Shallow => "_shallow",
                TsyDepth::Mid => "_mid",
                TsyDepth::Deep => "_deep",
            };
            let zone_name = format!("{}{}", state.family_id, suffix);
            if let Some(zone) = zones.find_zone_mut(&zone_name) {
                zone.spirit_qi = compute_layer_spirit_qi(layer, ratio, is_collapsing);
            }
        }
    }
}

/// plan §3.3 + §6 — `TsyCollapseCompleted` 事件消费器。
///
/// 1. 玩家化灰由 `extract_system::on_tsy_collapse_completed` 触发 DeathEvent；
///    本 cleanup 只处理 loot / 道伥 / zone 移除，不再把玩家安全弹回主世界。
/// 2. 删除 `DroppedLootRegistry` 中位于本 family 三层 AABB 内的 entry（"凡物随 zone 蒸发"）。
/// 3. 找出 zone 内所有 `Daoxiang` archetype NPC：50% 跨位面喷出主世界 ±10 格，50% despawn。
/// 4. 同时把 zone 内未激活的 `CorpseEmbalmed` 立刻激活成道伥（也走 50% Roll）。
/// 5. 从 `ZoneRegistry` 移除三层 subzone。
/// 6. state_reg 内对应 family 标 Dead（保留记录用于历史查询 / spawn 拒绝）。
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn tsy_collapse_completed_cleanup(
    mut commands: Commands,
    mut events: EventReader<TsyCollapseCompleted>,
    mut zones: ResMut<ZoneRegistry>,
    mut state_reg: ResMut<TsyZoneStateRegistry>,
    mut loot_registry: ResMut<DroppedLootRegistry>,
    presence_q: Query<(Entity, &TsyPresence)>,
    daoxiang_q: Query<(Entity, &Position, &NpcArchetype)>,
    corpse_q: Query<(Entity, &Position, &CorpseEmbalmed)>,
    layers: Option<Res<DimensionLayers>>,
    clock: Res<CombatClock>,
    mut dim_transfer: EventWriter<DimensionTransferRequest>,
) {
    for ev in events.read() {
        let family = ev.family_id.clone();
        let main_world_anchor = state_reg
            .by_family
            .get(&family)
            .map(|s| s.main_world_anchor)
            .unwrap_or(DimensionAnchor {
                dimension: crate::world::dimension::DimensionKind::Overworld,
                pos: DVec3::ZERO,
            });

        let aabbs = collect_family_aabbs(&zones, &family);

        // Step 1: 玩家不再弹回主世界。P5 撤离 plan 监听同一个 completed event，
        // 对仍持 TsyPresence 的玩家发 DeathEvent(cause="tsy_collapsed")，让 P1 death drop
        // 路径处理干尸 / 化灰。
        for (_entity, presence) in &presence_q {
            if presence.family_id != family {
                continue;
            }
        }

        // Step 2: 凡物 / 残留 ancient relic 随 zone 蒸发。
        //
        // **过滤规则（Codex review P1）**：仅删除 `source_container_id` 带本 family 标记的 entries。
        // - `tsy_spawn:{family}` —— 本 plan §1.5 spawn 的 ancient relic
        // - `tsy_corpse:{family}/...` —— P1 TSY 内死亡 drop 走 plan §3.3 加上的 family 前缀
        // 这样既排除了主世界同 XYZ 的无关 drop 被误删，也让本 family 关联的所有 entries
        // 能在塌缩时确定性蒸发。AABB 命中作为额外 sanity（同 family 多次实例化时区分）。
        let spawn_prefix = format!("tsy_spawn:{family}");
        let corpse_prefix = format!("tsy_corpse:{family}/");
        loot_registry.entries.retain(|_, entry| {
            let belongs_to_family = entry.source_container_id == spawn_prefix
                || entry.source_container_id.starts_with(&corpse_prefix);
            if !belongs_to_family {
                return true;
            }
            let pos = DVec3::new(entry.world_pos[0], entry.world_pos[1], entry.world_pos[2]);
            !point_in_any_aabb(pos, &aabbs)
        });

        // Step 3+4: 道伥喷出 / despawn；干尸先被加速激活成道伥再走同一 Roll
        // 用 family + tick 派生确定性 RNG，免引入 rand 依赖。
        let mut rng_seed = collapse_seed(&family, clock.tick);

        // 4a: 把 zone 内未激活的 CorpseEmbalmed 转成道伥（占用临时 vec 因为 spawn API 需要 Commands）
        let layer_entity = layers.as_deref().map(|l| l.tsy);
        for (corpse_entity, pos, corpse) in &corpse_q {
            if corpse.family_id != family || corpse.activated_to_daoxiang {
                continue;
            }
            if !point_in_any_aabb(pos.0, &aabbs) {
                continue;
            }
            // 加速激活：与自然激活同样 spawn，但立刻进 Roll 决定喷不喷。
            if let Some(layer) = layer_entity {
                let spawn_pos = pos.0;
                let new_entity =
                    spawn_daoxiang_from_corpse(&mut commands, layer, corpse, spawn_pos, clock.tick);
                let (decision, next_seed) = collapse_roll(rng_seed);
                rng_seed = next_seed;
                if decision {
                    let offset_pos = ejection_target(main_world_anchor.pos, rng_seed);
                    rng_seed = rng_seed.wrapping_mul(0x9E37_79B9).wrapping_add(1);
                    dim_transfer.send(DimensionTransferRequest {
                        entity: new_entity,
                        target: main_world_anchor.dimension,
                        target_pos: offset_pos,
                    });
                } else {
                    commands
                        .entity(new_entity)
                        .insert(valence::prelude::Despawned);
                }
            }
            // 干尸本体一律消失（已被激活）
            commands
                .entity(corpse_entity)
                .insert(valence::prelude::Despawned);
        }

        // 4b: 已存在的 Daoxiang NPC 同样走 50% Roll
        for (entity, pos, archetype) in &daoxiang_q {
            if !matches!(archetype, NpcArchetype::Daoxiang) {
                continue;
            }
            if !point_in_any_aabb(pos.0, &aabbs) {
                continue;
            }
            let (decision, next_seed) = collapse_roll(rng_seed);
            rng_seed = next_seed;
            if decision {
                let offset_pos = ejection_target(main_world_anchor.pos, rng_seed);
                rng_seed = rng_seed.wrapping_mul(0x9E37_79B9).wrapping_add(1);
                dim_transfer.send(DimensionTransferRequest {
                    entity,
                    target: main_world_anchor.dimension,
                    target_pos: offset_pos,
                });
            } else {
                commands.entity(entity).insert(valence::prelude::Despawned);
            }
        }

        // Step 5: 从 ZoneRegistry 移除三层 subzone
        let suffixes = ["_shallow", "_mid", "_deep"];
        for suffix in suffixes {
            let zone_name = format!("{family}{suffix}");
            zones.zones.retain(|z| z.name != zone_name);
        }

        // Step 6: 标 Dead（如果之前未 mark）
        if let Some(state) = state_reg.by_family.get_mut(&family) {
            state.lifecycle = TsyLifecycle::Dead;
            if state.dead_at_tick.is_none() {
                state.dead_at_tick = Some(ev.at_tick);
            }
        }
    }
}

/// plan §5.2 — 干尸自然累积转化为道伥。
///
/// 同一具干尸只激活一次：spawn 道伥后给 `CorpseEmbalmed` entity 插
/// `valence::prelude::Despawned` 标志（与 lifecycle 模块同惯例）。
/// 塌缩加速由 `tsy_collapse_completed_cleanup` 直接处理，本 tick 不重复。
pub fn tsy_corpse_to_daoxiang_tick(
    mut commands: Commands,
    corpse_q: Query<(Entity, &Position, &CorpseEmbalmed)>,
    state_reg: Res<TsyZoneStateRegistry>,
    layers: Option<Res<DimensionLayers>>,
    clock: Res<CombatClock>,
) {
    let Some(layers) = layers else { return };
    for (entity, pos, corpse) in &corpse_q {
        if corpse.activated_to_daoxiang {
            continue;
        }
        // 已 Collapsing 的 family 由 cleanup 一锅端，避免本 tick 与 cleanup 重复 spawn。
        if let Some(state) = state_reg.by_family.get(&corpse.family_id) {
            if matches!(
                state.lifecycle,
                TsyLifecycle::Collapsing | TsyLifecycle::Dead
            ) {
                continue;
            }
        }
        let elapsed = clock.tick.saturating_sub(corpse.died_at_tick);
        if elapsed < DAOXIANG_NATURAL_TICKS {
            continue;
        }
        spawn_daoxiang_from_corpse(&mut commands, layers.tsy, corpse, pos.0, clock.tick);
        commands.entity(entity).insert(valence::prelude::Despawned);
    }
}

/// 道伥来源记录（lore 用）。击杀道伥时反查可以重建"某某玩家死后其遗骸被激活"。
#[allow(dead_code)]
#[derive(Debug, Clone, valence::prelude::Component)]
pub struct DaoxiangOrigin {
    pub from_family: String,
    pub from_corpse_death_cause: String,
    pub activated_at_tick: u64,
    /// 原干尸已 drop 过的 instance_id；P3 / 后续 plan 用来反查 loot 继承链。
    pub inherited_drops: Vec<u64>,
}

/// plan §4.3 — 从 `CorpseEmbalmed` 激活道伥实体。
///
/// MVP 简化：复用 zombie entity bundle + `NpcArchetype::Daoxiang` + `DaoxiangOrigin`；
/// brain tree（disguise/aggro）由后续 plan 接入。本 plan 只负责 spawn API + 标记。
///
/// 两段式 spawn：先拿到 entity id 再 insert `npc_runtime_bundle`，保证 `Lifecycle.character_id`
/// 被正确填充（`canonical_npc_id` 依赖真实 entity id）。
pub fn spawn_daoxiang_from_corpse(
    commands: &mut Commands,
    layer: Entity,
    corpse: &CorpseEmbalmed,
    pos: DVec3,
    tick: u64,
) -> Entity {
    let entity = commands
        .spawn((
            ZombieEntityBundle {
                kind: EntityKind::ZOMBIE,
                layer: EntityLayerId(layer),
                position: Position::new([pos.x, pos.y, pos.z]),
                ..Default::default()
            },
            Transform::from_xyz(pos.x as f32, pos.y as f32, pos.z as f32),
            GlobalTransform::default(),
            NpcMarker,
            NpcArchetype::Daoxiang,
            DaoxiangOrigin {
                from_family: corpse.family_id.clone(),
                from_corpse_death_cause: corpse.death_cause.clone(),
                activated_at_tick: tick,
                inherited_drops: corpse.drops.clone(),
            },
        ))
        .id();
    commands
        .entity(entity)
        .insert(npc_runtime_bundle(entity, NpcArchetype::Daoxiang));
    entity
}

fn collect_family_aabbs(zones: &ZoneRegistry, family: &str) -> Vec<(DVec3, DVec3)> {
    let suffixes = ["_shallow", "_mid", "_deep"];
    suffixes
        .iter()
        .filter_map(|suffix| zones.find_zone_by_name(&format!("{family}{suffix}")))
        .map(|z| z.bounds)
        .collect()
}

fn point_in_any_aabb(pos: DVec3, aabbs: &[(DVec3, DVec3)]) -> bool {
    aabbs.iter().any(|(min, max)| {
        pos.x >= min.x
            && pos.x <= max.x
            && pos.y >= min.y
            && pos.y <= max.y
            && pos.z >= min.z
            && pos.z <= max.z
    })
}

/// 从 family 名 + 当前 tick 生成确定性塌缩 RNG 起点。
fn collapse_seed(family_id: &str, tick: u64) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    family_id.hash(&mut h);
    tick.hash(&mut h);
    h.finish()
}

/// 50% 概率掷骰；返回 `(命中, 下一 seed)`，调用方需用新 seed 串起来避免相关性。
fn collapse_roll(seed: u64) -> (bool, u64) {
    let next = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15).wrapping_add(1);
    let hit = (next % 1000) < DAOXIANG_EJECT_PROBABILITY_THOUSANDTH;
    (hit, next)
}

/// 在主世界裂缝锚点周围 ±10 格（XZ）随机选喷出落点。Y 抬 1 格防卡进方块。
fn ejection_target(anchor: DVec3, seed: u64) -> DVec3 {
    let off_x_raw = (seed % 2_001) as f64 / 1_000.0 - 1.0; // [-1, 1]
    let off_z_raw = ((seed >> 11) % 2_001) as f64 / 1_000.0 - 1.0;
    DVec3::new(
        anchor.x + off_x_raw * DAOXIANG_EJECT_OFFSET_RANGE,
        anchor.y + DAOXIANG_EJECT_Y_OFFSET,
        anchor.z + off_z_raw * DAOXIANG_EJECT_OFFSET_RANGE,
    )
}

/// 由 P1 `tsy_loot_spawn_on_enter` 驱动的"首次进入"hook。封装在本模块内
/// 让 P1 模块只多一行调用，避免在 inventory 模块里反向引用 lifecycle 细节。
///
/// 实参：
/// - `family_id`：从 `TsyEnterEmit.family_id` 直接拿
/// - `return_to`：从 `TsyEnterEmit.return_to`（主世界裂缝锚点）拿
/// - `tick`：当前 `CombatClock.tick`
///
/// 行为：family 不存在 → 注册并直接置 Active；存在 → no-op（除非 New，会推到 Active）。
pub fn on_first_enter(
    state_reg: &mut TsyZoneStateRegistry,
    family_id: &str,
    return_to: DimensionAnchor,
    tick: u64,
) {
    let source = source_class_from_family_id(family_id);
    state_reg.ensure_active(family_id, source, return_to, tick);
}

pub fn register(app: &mut App) {
    app.insert_resource(TsyZoneStateRegistry::default())
        .add_event::<TsyZoneActivated>()
        .add_event::<TsyCollapseStarted>()
        .add_event::<TsyCollapseCompleted>()
        .add_systems(
            Update,
            (
                tsy_lifecycle_tick,
                tsy_lifecycle_apply_spirit_qi.after(tsy_lifecycle_tick),
                tsy_corpse_to_daoxiang_tick.after(tsy_lifecycle_apply_spirit_qi),
                tsy_collapse_completed_cleanup
                    .after(tsy_lifecycle_tick)
                    .after(crate::world::extract_system::on_tsy_collapse_completed)
                    .before(DimensionTransferSet),
            ),
        );
    // TsyLayer 由 dimension 模块注册；这里仅引用类型保证没人误删。
    let _ = std::any::TypeId::of::<TsyLayer>();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::dimension::DimensionKind;
    use crate::world::zone::Zone;

    fn anchor() -> DimensionAnchor {
        DimensionAnchor {
            dimension: DimensionKind::Overworld,
            pos: DVec3::new(10.0, 65.0, 10.0),
        }
    }

    #[test]
    fn ensure_active_creates_state_in_active_with_anchor() {
        let mut reg = TsyZoneStateRegistry::default();
        let inserted =
            reg.ensure_active("tsy_lingxu_01", AncientRelicSource::DaoLord, anchor(), 100);
        assert!(inserted);
        let s = reg.by_family.get("tsy_lingxu_01").unwrap();
        assert_eq!(s.lifecycle, TsyLifecycle::Active);
        assert_eq!(s.activated_at_tick, Some(100));
        assert_eq!(s.created_at_tick, 100);
        assert_eq!(s.main_world_anchor.pos, DVec3::new(10.0, 65.0, 10.0));
    }

    #[test]
    fn ensure_active_is_idempotent() {
        let mut reg = TsyZoneStateRegistry::default();
        reg.ensure_active("tsy_lingxu_01", AncientRelicSource::DaoLord, anchor(), 100);
        let inserted2 = reg.ensure_active(
            "tsy_lingxu_01",
            AncientRelicSource::SectRuins,
            anchor(),
            200,
        );
        assert!(!inserted2);
        // source_class 不被覆盖；activated_at_tick 不被改写
        let s = reg.by_family.get("tsy_lingxu_01").unwrap();
        assert_eq!(s.source_class, AncientRelicSource::DaoLord);
        assert_eq!(s.activated_at_tick, Some(100));
    }

    #[test]
    fn mark_initial_skeleton_first_call_only() {
        let mut reg = TsyZoneStateRegistry::default();
        reg.ensure_active("tsy_a", AncientRelicSource::DaoLord, anchor(), 0);
        assert!(reg.mark_initial_skeleton("tsy_a", vec![1, 2, 3]));
        let s = reg.by_family.get("tsy_a").unwrap();
        assert_eq!(s.initial_skeleton, vec![1, 2, 3]);
        assert_eq!(s.remaining_skeleton.len(), 3);
        // 第二次写不覆盖
        assert!(!reg.mark_initial_skeleton("tsy_a", vec![9, 9, 9]));
        let s = reg.by_family.get("tsy_a").unwrap();
        assert_eq!(s.initial_skeleton, vec![1, 2, 3]);
    }

    #[test]
    fn mark_initial_skeleton_unknown_family_noop() {
        let mut reg = TsyZoneStateRegistry::default();
        assert!(!reg.mark_initial_skeleton("missing", vec![1]));
    }

    #[test]
    fn skeleton_ratio_full_when_initial_empty() {
        let s = TsyZoneState {
            family_id: "x".into(),
            lifecycle: TsyLifecycle::Active,
            source_class: AncientRelicSource::DaoLord,
            initial_skeleton: vec![],
            remaining_skeleton: HashSet::new(),
            created_at_tick: 0,
            activated_at_tick: None,
            collapsing_started_at_tick: None,
            dead_at_tick: None,
            main_world_anchor: anchor(),
        };
        assert_eq!(s.skeleton_ratio(), 1.0);
        assert_eq!(s.taken_count(), 0);
    }

    #[test]
    fn skeleton_ratio_half() {
        let s = TsyZoneState {
            family_id: "x".into(),
            lifecycle: TsyLifecycle::Active,
            source_class: AncientRelicSource::DaoLord,
            initial_skeleton: vec![1, 2, 3, 4],
            remaining_skeleton: HashSet::from([1, 2]),
            created_at_tick: 0,
            activated_at_tick: None,
            collapsing_started_at_tick: None,
            dead_at_tick: None,
            main_world_anchor: anchor(),
        };
        assert_eq!(s.skeleton_ratio(), 0.5);
        assert_eq!(s.taken_count(), 2);
    }

    #[test]
    fn compute_layer_spirit_qi_full_skeleton_returns_base() {
        assert!((compute_layer_spirit_qi(TsyDepth::Shallow, 1.0, false) - (-0.3)).abs() < 1e-9);
        assert!((compute_layer_spirit_qi(TsyDepth::Mid, 1.0, false) - (-0.6)).abs() < 1e-9);
        assert!((compute_layer_spirit_qi(TsyDepth::Deep, 1.0, false) - (-0.9)).abs() < 1e-9);
    }

    #[test]
    fn compute_layer_spirit_qi_zero_skeleton_at_max_depth_factor() {
        // shallow: -0.3 + (-0.3) * 1 = -0.6
        assert!((compute_layer_spirit_qi(TsyDepth::Shallow, 0.0, false) - (-0.6)).abs() < 1e-9);
        // deep: -0.9 + (-0.3) * 1 = -1.2 → clamp 到 -1.0
        assert!((compute_layer_spirit_qi(TsyDepth::Deep, 0.0, false) - (-1.0)).abs() < 1e-9);
    }

    #[test]
    fn compute_layer_spirit_qi_collapsing_doubles_then_clamps() {
        // shallow ratio=0 base value = -0.6, ×2 = -1.2 → clamp -1.0
        assert!(
            (compute_layer_spirit_qi(TsyDepth::Shallow, 0.0, true) - (-1.0)).abs() < 1e-9,
            "shallow collapsing should clamp at -1.0"
        );
        // mid ratio=1.0 base = -0.6, ×2 = -1.2 → clamp -1.0
        assert!((compute_layer_spirit_qi(TsyDepth::Mid, 1.0, true) - (-1.0)).abs() < 1e-9);
    }

    #[test]
    fn compute_layer_spirit_qi_is_monotonic_in_skeleton_depletion() {
        // 同 layer / 非 collapsing：ratio 越低（骨架越少）越深
        let r1 = compute_layer_spirit_qi(TsyDepth::Shallow, 1.0, false);
        let r2 = compute_layer_spirit_qi(TsyDepth::Shallow, 0.5, false);
        let r3 = compute_layer_spirit_qi(TsyDepth::Shallow, 0.25, false);
        assert!(r1 > r2 && r2 > r3, "got {r1}, {r2}, {r3}");
    }

    #[test]
    fn collapse_roll_is_deterministic_per_seed() {
        let (a, _) = collapse_roll(123);
        let (b, _) = collapse_roll(123);
        assert_eq!(a, b);
    }

    #[test]
    fn collapse_roll_distribution_around_50_percent() {
        let mut hits = 0u32;
        let mut seed = 42u64;
        let trials = 10_000u32;
        for _ in 0..trials {
            let (hit, next) = collapse_roll(seed);
            seed = next;
            if hit {
                hits += 1;
            }
        }
        // 期望 ~5000；放宽到 ±10% 通过 CI 抖动
        let lo = (trials as f64 * 0.40) as u32;
        let hi = (trials as f64 * 0.60) as u32;
        assert!(hits > lo && hits < hi, "hits={hits} not in [{lo},{hi}]");
    }

    #[test]
    fn ejection_target_lies_within_offset_range() {
        let anchor_pos = DVec3::new(100.0, 64.0, 200.0);
        for seed in 0..256u64 {
            let p = ejection_target(anchor_pos, seed);
            assert!(
                (p.x - 100.0).abs() <= DAOXIANG_EJECT_OFFSET_RANGE + 1e-6,
                "x off by {} (seed={seed})",
                (p.x - 100.0).abs()
            );
            assert!((p.z - 200.0).abs() <= DAOXIANG_EJECT_OFFSET_RANGE + 1e-6);
            assert!((p.y - (64.0 + DAOXIANG_EJECT_Y_OFFSET)).abs() < 1e-9);
        }
    }

    #[test]
    fn point_in_any_aabb_works() {
        let aabbs = vec![(DVec3::new(0.0, 0.0, 0.0), DVec3::new(10.0, 10.0, 10.0))];
        assert!(point_in_any_aabb(DVec3::new(5.0, 5.0, 5.0), &aabbs));
        assert!(point_in_any_aabb(DVec3::new(0.0, 0.0, 0.0), &aabbs));
        assert!(point_in_any_aabb(DVec3::new(10.0, 10.0, 10.0), &aabbs));
        assert!(!point_in_any_aabb(DVec3::new(11.0, 5.0, 5.0), &aabbs));
        assert!(!point_in_any_aabb(DVec3::new(5.0, 5.0, -1.0), &aabbs));
    }

    #[test]
    fn collect_family_aabbs_returns_three_layers() {
        fn mk(name: &str) -> Zone {
            Zone {
                name: name.to_string(),
                dimension: DimensionKind::Tsy,
                bounds: (DVec3::ZERO, DVec3::splat(1.0)),
                spirit_qi: 0.0,
                danger_level: 5,
                active_events: vec![],
                patrol_anchors: vec![],
                blocked_tiles: vec![],
            }
        }
        let zones = ZoneRegistry {
            zones: vec![
                mk("tsy_a_shallow"),
                mk("tsy_a_mid"),
                mk("tsy_a_deep"),
                mk("tsy_b_shallow"),
            ],
        };
        let aabbs = collect_family_aabbs(&zones, "tsy_a");
        assert_eq!(aabbs.len(), 3);
    }

    #[test]
    fn lifecycle_helpers_classify_states() {
        assert!(TsyLifecycle::Collapsing.is_collapsing());
        assert!(!TsyLifecycle::Active.is_collapsing());
        assert!(TsyLifecycle::Dead.is_dead());
        assert!(!TsyLifecycle::Collapsing.is_dead());
    }

    #[test]
    fn registry_query_helpers() {
        let mut reg = TsyZoneStateRegistry::default();
        reg.ensure_active("tsy_x", AncientRelicSource::DaoLord, anchor(), 0);
        assert!(!reg.is_collapsing("tsy_x"));
        assert!(!reg.is_dead("tsy_x"));
        reg.by_family.get_mut("tsy_x").unwrap().lifecycle = TsyLifecycle::Collapsing;
        assert!(reg.is_collapsing("tsy_x"));
        reg.by_family.get_mut("tsy_x").unwrap().lifecycle = TsyLifecycle::Dead;
        assert!(reg.is_dead("tsy_x"));
        // unknown family
        assert!(!reg.is_collapsing("tsy_unknown"));
    }
}
