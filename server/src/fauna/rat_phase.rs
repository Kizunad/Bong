use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use valence::entity::entity::{CustomName, NameVisible};
use valence::prelude::{
    bevy_ecs, BlockPos, Changed, ChunkPos, Color, Component, DVec3, Entity, Event, EventReader,
    EventWriter, IntoText, ParamSet, Position, Query, Res, ResMut, Resource, Text, With,
};

use crate::combat::events::DeathEvent;
use crate::combat::CombatClock;
use crate::network::vfx_event_emit::VfxEventRequest;
use crate::npc::spawn::NpcMarker;
use crate::npc::spawn_rat::RatBlackboard;
use crate::qi_physics::constants::{QI_EPSILON, QI_ZONE_UNIT_CAPACITY};
use crate::qi_physics::{
    qi_release_to_zone, QiAccountId, QiPhysicsError, QiTransfer, QiTransferReason, WorldQiAccount,
};
use crate::schema::vfx_event::VfxEventPayloadV1;
use crate::world::dimension::{CurrentDimension, DimensionKind};
use crate::world::karma::{QiDensityHeatmap, QI_DENSITY_CELL_SIZE};
use crate::world::zone::{Zone, ZoneRegistry};

pub const RAT_PHASE_DENSITY_THRESHOLD: f32 = 8.0;
pub const RAT_PHASE_QI_GRADIENT_THRESHOLD: f32 = 0.20;
pub const SURGE_TRIGGER_THRESHOLD: f32 = 1.0;
pub const TRANSITION_DURATION_TICKS: u16 = 600;
pub const RAT_DRAINED_CHUNK_WINDOW: usize = 8;
/// ★守恒红线 blocker 修正——与 `network::command_executor` / `world::pseudo_vein_runtime`
/// 同源数值的本地拷贝（仓库既有惯例：无共享符号，每个调用点各自维护同值 const）。
const ZONE_SPIRIT_QI_MIN: f64 = -1.0;
const ZONE_SPIRIT_QI_MAX: f64 = 1.0;

type RatPhaseReadQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Position,
        Option<&'static CurrentDimension>,
        &'static RatGroupId,
        &'static RatBlackboard,
        &'static RatPhase,
    ),
    With<NpcMarker>,
>;

type RatPhaseUpdateQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Position,
        Option<&'static CurrentDimension>,
        &'static RatGroupId,
        &'static RatBlackboard,
        &'static mut PressureSensor,
        &'static RatPhase,
    ),
    With<NpcMarker>,
>;

type RatPhaseVisualQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static RatPhase,
        &'static mut CustomName,
        &'static mut NameVisible,
    ),
    (With<NpcMarker>, Changed<RatPhase>),
>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Component)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum RatPhase {
    #[default]
    Solitary,
    Transitioning {
        progress: u16,
    },
    Gregarious,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Component)]
pub struct RatGroupId(pub u64);

impl RatGroupId {
    pub fn for_zone_chunk(zone_name: &str, chunk: ChunkPos) -> Self {
        let mut hash = 0xcbf2_9ce4_8422_2325u64;
        for byte in zone_name.bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x100_0000_01b3);
        }
        hash ^= (chunk.x as u64).rotate_left(17);
        hash ^= (chunk.z as u64).rotate_right(11);
        Self(hash)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Component)]
pub struct PressureSensor {
    pub local_density: f32,
    pub qi_pressure_grad: f32,
    pub surge_intensity: f32,
    pub negative_pressure_avoidance: f32,
}

impl Default for PressureSensor {
    fn default() -> Self {
        Self {
            local_density: 0.0,
            qi_pressure_grad: 0.0,
            surge_intensity: 0.0,
            negative_pressure_avoidance: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub struct MeditatingState {
    pub since_tick: u64,
}

#[derive(Debug, Clone, Event, PartialEq, Serialize, Deserialize)]
pub struct RatPhaseChangeEvent {
    pub chunk: [i32; 2],
    pub zone: String,
    pub group_id: u64,
    pub from: RatPhase,
    pub to: RatPhase,
    pub rat_count: u32,
    pub local_qi: f32,
    pub qi_gradient: f32,
    pub tick: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RatDensitySnapshot {
    pub total: u32,
    pub solitary: u32,
    pub transitioning: u32,
    pub gregarious: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RatDensityHeatmapV1 {
    pub zones: HashMap<String, RatDensitySnapshot>,
}

#[derive(Debug, Clone, Resource)]
pub struct LocustSwarmCooldownStore {
    pub last_swarm_by_zone: HashMap<String, u64>,
    pub cooldown_ticks: u64,
}

impl LocustSwarmCooldownStore {
    pub const DEFAULT_COOLDOWN_TICKS: u64 = 24 * 3600 * 20;

    pub fn ready_at(&self, zone: &str, tick: u64) -> bool {
        self.last_swarm_by_zone
            .get(zone)
            .is_none_or(|last| tick.saturating_sub(*last) >= self.cooldown_ticks)
    }

    pub fn mark(&mut self, zone: impl Into<String>, tick: u64) {
        self.last_swarm_by_zone.insert(zone.into(), tick);
    }
}

impl Default for LocustSwarmCooldownStore {
    fn default() -> Self {
        Self {
            last_swarm_by_zone: HashMap::new(),
            cooldown_ticks: Self::DEFAULT_COOLDOWN_TICKS,
        }
    }
}

pub fn chunk_pos_from_world(position: DVec3) -> ChunkPos {
    ChunkPos::new(
        (position.x.floor() as i32).div_euclid(16),
        (position.z.floor() as i32).div_euclid(16),
    )
}

pub fn rat_phase_wire_name(phase: RatPhase) -> &'static str {
    match phase {
        RatPhase::Solitary => "solitary",
        RatPhase::Transitioning { .. } => "transitioning",
        RatPhase::Gregarious => "gregarious",
    }
}

pub fn rat_phase_display_name(phase: &RatPhase) -> Text {
    match phase {
        RatPhase::Solitary => "噬元鼠".color(Color::GRAY),
        RatPhase::Transitioning { .. } => "赤噬元鼠".color(Color::DARK_RED),
        RatPhase::Gregarious => "灵蝗噬元鼠".color(Color::RED),
    }
}

pub fn is_drained_chunk(blackboard: &RatBlackboard, chunk: ChunkPos) -> bool {
    blackboard.recently_drained.contains(&chunk)
}

pub fn remember_drained_chunk(blackboard: &mut RatBlackboard, chunk: ChunkPos) {
    if is_drained_chunk(blackboard, chunk) {
        return;
    }
    blackboard.recently_drained.push(chunk);
    if blackboard.recently_drained.len() > RAT_DRAINED_CHUNK_WINDOW {
        let overflow = blackboard.recently_drained.len() - RAT_DRAINED_CHUNK_WINDOW;
        blackboard.recently_drained.drain(0..overflow);
    }
}

pub fn pressure_sensor_tick_system(
    clock: Option<Res<CombatClock>>,
    heatmap: Option<Res<QiDensityHeatmap>>,
    mut phase_events: EventWriter<RatPhaseChangeEvent>,
    mut rats: ParamSet<(RatPhaseReadQuery<'_, '_>, RatPhaseUpdateQuery<'_, '_>)>,
) {
    let tick = clock.as_deref().map(|clock| clock.tick).unwrap_or_default();
    let mut counts: HashMap<(u64, ChunkPos), u32> = HashMap::new();
    for (_, position, _, group_id, _, _) in rats.p0().iter() {
        *counts
            .entry((group_id.0, chunk_pos_from_world(position.get())))
            .or_default() += 1;
    }

    let mut emitted_transitions = HashSet::new();
    for (_, position, dimension, group_id, blackboard, mut sensor, phase) in rats.p1().iter_mut() {
        let chunk = chunk_pos_from_world(position.get());
        let rat_count = counts.get(&(group_id.0, chunk)).copied().unwrap_or(0);
        sensor.local_density = rat_count as f32 / RAT_PHASE_DENSITY_THRESHOLD;
        sensor.qi_pressure_grad = qi_gradient_for_chunk(
            heatmap.as_deref(),
            dimension.map(|dim| dim.0).unwrap_or_default(),
            chunk,
        );
        if sensor.local_density >= 1.0 && sensor.qi_pressure_grad >= RAT_PHASE_QI_GRADIENT_THRESHOLD
        {
            sensor.surge_intensity += sensor.local_density * sensor.qi_pressure_grad;
        } else {
            sensor.surge_intensity *= 0.5;
        }

        if matches!(phase, RatPhase::Solitary)
            && sensor.surge_intensity >= SURGE_TRIGGER_THRESHOLD
            && !is_drained_chunk(blackboard, chunk)
            && emitted_transitions.insert((group_id.0, chunk.x, chunk.z))
        {
            phase_events.send(RatPhaseChangeEvent {
                chunk: [chunk.x, chunk.z],
                zone: blackboard.home_zone.clone(),
                group_id: group_id.0,
                from: RatPhase::Solitary,
                to: RatPhase::Transitioning { progress: 0 },
                rat_count,
                local_qi: heatmap
                    .as_deref()
                    .map(|heatmap| {
                        heatmap.heat_at(
                            dimension.map(|dim| dim.0).unwrap_or_default(),
                            chunk_block_pos(chunk),
                        )
                    })
                    .unwrap_or_default(),
                qi_gradient: sensor.qi_pressure_grad,
                tick,
            });
        }
    }
}

pub fn apply_rat_phase_change_system(
    mut events: EventReader<RatPhaseChangeEvent>,
    mut rats: Query<(&Position, &RatGroupId, &mut RatPhase), With<NpcMarker>>,
) {
    for event in events.read() {
        let chunk = ChunkPos::new(event.chunk[0], event.chunk[1]);
        for (position, group_id, mut phase) in &mut rats {
            if group_id.0 == event.group_id
                && chunk_pos_from_world(position.get()) == chunk
                && *phase == event.from
            {
                *phase = event.to;
            }
        }
    }
}

pub fn emit_rat_swarm_aura_vfx_system(
    mut events: EventReader<RatPhaseChangeEvent>,
    rats: Query<(&Position, &RatGroupId), With<NpcMarker>>,
    mut vfx_events: EventWriter<VfxEventRequest>,
) {
    for event in events.read() {
        if !matches!(event.to, RatPhase::Gregarious) {
            continue;
        }
        let chunk = ChunkPos::new(event.chunk[0], event.chunk[1]);
        let mut total = DVec3::ZERO;
        let mut count = 0u32;
        for (position, group_id) in &rats {
            if group_id.0 == event.group_id && chunk_pos_from_world(position.get()) == chunk {
                total += position.get();
                count = count.saturating_add(1);
            }
        }
        if count == 0 {
            continue;
        }
        let center = total / f64::from(count);
        vfx_events.send(VfxEventRequest::new(
            center,
            VfxEventPayloadV1::SpawnParticle {
                event_id: "bong:rat_swarm_aura".to_string(),
                origin: [center.x, center.y, center.z],
                direction: None,
                color: Some("#FF4444".to_string()),
                strength: Some(0.9),
                count: Some(24),
                duration_ticks: Some(36),
            },
        ));
    }
}

pub fn advance_transitioning_phase_system(mut rats: Query<&mut RatPhase, With<NpcMarker>>) {
    for mut phase in &mut rats {
        if let RatPhase::Transitioning { progress } = *phase {
            let next = progress.saturating_add(1);
            *phase = if next >= TRANSITION_DURATION_TICKS {
                RatPhase::Gregarious
            } else {
                RatPhase::Transitioning { progress: next }
            };
        }
    }
}

pub fn apply_rat_phase_visual_system(mut rats: RatPhaseVisualQuery<'_, '_>) {
    for (phase, mut custom_name, mut name_visible) in &mut rats {
        custom_name.0 = Some(rat_phase_display_name(phase));
        name_visible.0 = true;
    }
}

pub fn release_drained_qi_on_death_system(
    mut deaths: EventReader<DeathEvent>,
    rats: Query<(&Position, Option<&CurrentDimension>, &RatBlackboard), With<NpcMarker>>,
    mut zones: Option<ResMut<ZoneRegistry>>,
    mut ledger: Option<ResMut<WorldQiAccount>>,
) {
    let (Some(zones), Some(ledger)) = (zones.as_deref_mut(), ledger.as_deref_mut()) else {
        for _ in deaths.read() {}
        return;
    };

    for death in deaths.read() {
        let Ok((position, dimension, rat)) = rats.get(death.target) else {
            continue;
        };
        if rat.drained_qi <= 0.0 {
            continue;
        }
        let dim = dimension.map(|dim| dim.0).unwrap_or_default();
        let Some(zone_name) = zones
            .find_zone(dim, position.get())
            .map(|zone| zone.name.clone())
        else {
            continue;
        };
        if let Some(zone) = zones.find_zone_mut(zone_name.as_str()) {
            let rat_account = QiAccountId::npc(format!("rat:{}", death.target.index()));
            if let Err(error) = transfer_rat_drained_qi_to_zone(ledger, zone, &rat_account) {
                tracing::debug!(
                    "[bong][fauna] rat death qi transfer to zone failed for {:?}: {:?}",
                    death.target,
                    error
                );
            }
        }
    }
}

/// §8.1 决议 #3（promote 博弈 blocker 修正 v2）—— field-authority 三段式："鼠 -> zone" 这一腿
/// 从"account-only 转账"改为"同步→accepted/overflow 分流→写回字段"。
///
/// **v1 遗留漏洞**（本次 blocker）：v1 把 `amount` 100% 无条件转进 zone 账户、只 clamp 写回的
/// **字段**、不 clamp **账户**。zone 逼近 cap 时账户会越过 cap，字段被 clamp 后账户/字段失配；
/// 下一次任何覆盖式重同步（本函数为下一只鼠调用时开场的 `set_balance`，或生产常驻
/// `npc::dormant::apply_dormant_regen_with_multiplier` / `world::heartbeat::zone_qi_inflow_tick`
/// 门 `spirit_qi > QI_NPC_ABSORB_FLOOR` 时的 `set_balance(zone_account,
/// spirit_qi.max(0)*CAP)`）会把账户覆写回 `field.clamp * CAP`，无对偿地销毁溢出——`ledger_qi`
/// 净减、无对应增长，违反 qi_physics 守恒律。
///
/// v2 修法：复用 `qi_physics::release::qi_release_to_zone` 算出 accepted/overflow 分流
/// （`accepted = amount.min(room)`，`room = (zone_cap - zone_current).max(0)`），accepted 腿
/// 用它返回的 `ZoneReleaseOutcome::transfer`（reason=`ReleaseToZone`，语义比旧 `RatBiteDrain`
/// 更准——这一腿是"释放回 zone"而非"咬取"）真实入账；overflow 腿另建一条
/// `rat_account -> QiAccountId::overflow("rat_bite_drain:<rat_account.id>")` 转账
/// （命名/reason 均照抄 `cultivation::death_hooks::release_qi_overflow_from` 的
/// overflow 账户 + `ReleaseToZone` 约定），进可审计的 overflow 账户而不是蒸发。
/// 两腿相加 == amount，`rat_account` 必然被全额抽干（不留僵尸余额）。
///
/// `zone_account` 由 `zone.name` 内部派生，不再由调用方单独传入，防止字段/账户对错 zone。
/// `ledger.balance(rat_account) <= 0.0` 时 no-op，返回 `Ok(0.0)`。
pub fn transfer_rat_drained_qi_to_zone(
    ledger: &mut WorldQiAccount,
    zone: &mut Zone,
    rat_account: &QiAccountId,
) -> Result<f64, QiPhysicsError> {
    let amount = ledger.balance(rat_account);
    if amount <= 0.0 {
        return Ok(0.0);
    }

    let zone_account = QiAccountId::zone(zone.name.clone());
    // 转账前：把 zone 账户镜像同步到字段真实值，accepted/overflow 分流针对的是真实容量，
    // 而不是陈旧的镜像余额（同 `zone_qi_inflow_tick` / `inject_zone_for_pseudo_vein` 范式）。
    ledger.set_balance(
        zone_account.clone(),
        zone.spirit_qi.max(0.0) * QI_ZONE_UNIT_CAPACITY,
    )?;

    let zone_current = zone.spirit_qi.max(0.0) * QI_ZONE_UNIT_CAPACITY;
    let zone_cap = ZONE_SPIRIT_QI_MAX * QI_ZONE_UNIT_CAPACITY;
    let outcome = qi_release_to_zone(
        amount,
        rat_account.clone(),
        zone_account.clone(),
        zone_current,
        zone_cap,
    )?;

    // accepted 腿：截到 zone 剩余容量为止，绝不越 cap。
    if let Some(transfer) = outcome.transfer {
        ledger.transfer(transfer)?;
    }

    // overflow 腿：zone 吃不下的部分进可审计 overflow 账户，绝不蒸发。
    if outcome.overflow > QI_EPSILON {
        let overflow_account = QiAccountId::overflow(format!("rat_bite_drain:{}", rat_account.id));
        let overflow_transfer = QiTransfer::new(
            rat_account.clone(),
            overflow_account,
            outcome.overflow,
            QiTransferReason::ReleaseToZone,
        )?;
        ledger.transfer(overflow_transfer)?;
    }

    // 转账后写回字段——此时账户余额已 <= zone_cap，clamp 仍保留作防御但不再是"救火"。
    // 缺了这行，下一次生产常驻覆盖式重同步（见上方 doc-comment）依旧会用陈旧字段值
    // 覆写账户，造成二次蒸发。
    zone.spirit_qi = (ledger.balance(&zone_account) / QI_ZONE_UNIT_CAPACITY)
        .clamp(ZONE_SPIRIT_QI_MIN, ZONE_SPIRIT_QI_MAX);

    Ok(amount)
}

pub fn collect_rat_density_heatmap<'a, I>(rats: I) -> RatDensityHeatmapV1
where
    I: IntoIterator<Item = (&'a RatBlackboard, &'a RatPhase)>,
{
    let mut heatmap = RatDensityHeatmapV1::default();
    for (blackboard, phase) in rats {
        let entry = heatmap
            .zones
            .entry(blackboard.home_zone.clone())
            .or_default();
        entry.total = entry.total.saturating_add(1);
        match phase {
            RatPhase::Solitary => entry.solitary = entry.solitary.saturating_add(1),
            RatPhase::Transitioning { .. } => {
                entry.transitioning = entry.transitioning.saturating_add(1)
            }
            RatPhase::Gregarious => entry.gregarious = entry.gregarious.saturating_add(1),
        }
    }
    heatmap
}

fn qi_gradient_for_chunk(
    heatmap: Option<&QiDensityHeatmap>,
    dimension: DimensionKind,
    chunk: ChunkPos,
) -> f32 {
    let Some(heatmap) = heatmap else {
        return 0.0;
    };
    let mut min_heat = f32::MAX;
    let mut max_heat = f32::MIN;
    for dx in -1..=1 {
        for dz in -1..=1 {
            let sample = ChunkPos::new(chunk.x + dx, chunk.z + dz);
            let heat = heatmap.heat_at(dimension, chunk_block_pos(sample));
            min_heat = min_heat.min(heat);
            max_heat = max_heat.max(heat);
        }
    }
    if min_heat == f32::MAX {
        0.0
    } else {
        (max_heat - min_heat).max(0.0)
    }
}

fn chunk_block_pos(chunk: ChunkPos) -> BlockPos {
    BlockPos::new(
        chunk.x * QI_DENSITY_CELL_SIZE,
        64,
        chunk.z * QI_DENSITY_CELL_SIZE,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use valence::entity::entity::{CustomName, NameVisible};
    use valence::prelude::{App, Events, Update};

    use crate::world::karma::QiDensityHeatmap;
    use crate::world::zone::ZoneRegistry;

    fn rat_blackboard(zone: &str, chunk: ChunkPos) -> RatBlackboard {
        RatBlackboard {
            home_chunk: chunk,
            home_zone: zone.to_string(),
            group_id: RatGroupId(7),
            last_pressure_target: None,
            recently_drained: Vec::new(),
            drained_qi: 0.0,
            harass_cooldown_until_tick: 0,
        }
    }

    #[test]
    fn rat_phase_default_is_solitary() {
        assert_eq!(RatPhase::default(), RatPhase::Solitary);
    }

    #[test]
    fn pressure_sensor_density_threshold_triggers_transition() {
        let mut app = App::new();
        app.add_event::<RatPhaseChangeEvent>();
        app.insert_resource(CombatClock { tick: 12345 });
        let mut heatmap = QiDensityHeatmap::default();
        heatmap.add_heat(DimensionKind::Overworld, BlockPos::new(0, 64, 0), 0.0);
        heatmap.add_heat(DimensionKind::Overworld, BlockPos::new(16, 64, 0), 1.0);
        app.insert_resource(heatmap);
        app.add_systems(Update, pressure_sensor_tick_system);

        let chunk = ChunkPos::new(0, 0);
        for i in 0..8 {
            app.world_mut().spawn((
                NpcMarker,
                Position::new([f64::from(i), 64.0, 0.0]),
                RatGroupId(7),
                rat_blackboard("spawn", chunk),
                PressureSensor::default(),
                RatPhase::Solitary,
            ));
        }

        app.update();

        let events = app.world().resource::<Events<RatPhaseChangeEvent>>();
        let emitted = events.iter_current_update_events().collect::<Vec<_>>();
        assert_eq!(
            emitted.len(),
            1,
            "one dense group chunk should emit exactly one phase-change event"
        );
        let event = emitted
            .first()
            .expect("dense rat chunk with qi gradient should emit transition");
        assert_eq!(event.chunk, [0, 0]);
        assert_eq!(event.from, RatPhase::Solitary);
        assert_eq!(event.to, RatPhase::Transitioning { progress: 0 });
        assert_eq!(event.rat_count, 8);
    }

    #[test]
    fn pressure_sensor_low_qi_gradient_does_not_transition_alone() {
        let mut app = App::new();
        app.add_event::<RatPhaseChangeEvent>();
        app.insert_resource(QiDensityHeatmap::default());
        app.add_systems(Update, pressure_sensor_tick_system);
        let chunk = ChunkPos::new(0, 0);
        for i in 0..8 {
            app.world_mut().spawn((
                NpcMarker,
                Position::new([f64::from(i), 64.0, 0.0]),
                RatGroupId(7),
                rat_blackboard("spawn", chunk),
                PressureSensor::default(),
                RatPhase::Solitary,
            ));
        }

        app.update();

        assert!(
            app.world()
                .resource::<Events<RatPhaseChangeEvent>>()
                .is_empty(),
            "density without qi gradient must not trigger phase transition"
        );
    }

    #[test]
    fn pressure_sensor_high_qi_gradient_alone_does_not_transition() {
        let mut app = App::new();
        app.add_event::<RatPhaseChangeEvent>();
        let mut heatmap = QiDensityHeatmap::default();
        heatmap.add_heat(DimensionKind::Overworld, BlockPos::new(0, 64, 0), 0.0);
        heatmap.add_heat(DimensionKind::Overworld, BlockPos::new(16, 64, 0), 1.0);
        app.insert_resource(heatmap);
        app.add_systems(Update, pressure_sensor_tick_system);

        let chunk = ChunkPos::new(0, 0);
        app.world_mut().spawn((
            NpcMarker,
            Position::new([0.0, 64.0, 0.0]),
            RatGroupId(7),
            rat_blackboard("spawn", chunk),
            PressureSensor::default(),
            RatPhase::Solitary,
        ));

        app.update();

        assert!(
            app.world()
                .resource::<Events<RatPhaseChangeEvent>>()
                .is_empty(),
            "qi gradient without enough rats must not trigger phase transition"
        );
    }

    #[test]
    fn transitioning_phase_promotes_to_gregarious_after_duration() {
        let mut app = App::new();
        app.add_systems(Update, advance_transitioning_phase_system);
        let rat = app
            .world_mut()
            .spawn((
                NpcMarker,
                RatPhase::Transitioning {
                    progress: TRANSITION_DURATION_TICKS - 1,
                },
            ))
            .id();

        app.update();

        assert_eq!(
            app.world().get::<RatPhase>(rat),
            Some(&RatPhase::Gregarious)
        );
    }

    #[test]
    fn rat_phase_visual_name_tracks_phase() {
        let mut app = App::new();
        app.add_systems(Update, apply_rat_phase_visual_system);
        let rat = app
            .world_mut()
            .spawn((
                NpcMarker,
                RatPhase::Transitioning { progress: 1 },
                CustomName(None),
                NameVisible(false),
            ))
            .id();

        app.update();

        assert_eq!(
            app.world()
                .get::<CustomName>(rat)
                .and_then(|name| name.0.clone()),
            Some(rat_phase_display_name(&RatPhase::Transitioning {
                progress: 1
            }))
        );
        assert_eq!(
            app.world().get::<NameVisible>(rat),
            Some(&NameVisible(true))
        );
    }

    #[test]
    fn apply_rat_phase_change_synchronizes_full_chunk_group() {
        let mut app = App::new();
        app.add_event::<RatPhaseChangeEvent>();
        app.add_systems(Update, apply_rat_phase_change_system);
        let same_chunk_a = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([1.0, 64.0, 1.0]),
                RatGroupId(7),
                RatPhase::Solitary,
            ))
            .id();
        let same_chunk_b = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([2.0, 64.0, 2.0]),
                RatGroupId(7),
                RatPhase::Solitary,
            ))
            .id();
        let other_group = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([3.0, 64.0, 3.0]),
                RatGroupId(8),
                RatPhase::Solitary,
            ))
            .id();

        app.world_mut().send_event(RatPhaseChangeEvent {
            chunk: [0, 0],
            zone: "spawn".to_string(),
            group_id: 7,
            from: RatPhase::Solitary,
            to: RatPhase::Gregarious,
            rat_count: 2,
            local_qi: 0.6,
            qi_gradient: 0.4,
            tick: 9,
        });
        app.update();

        assert_eq!(
            app.world().get::<RatPhase>(same_chunk_a),
            Some(&RatPhase::Gregarious)
        );
        assert_eq!(
            app.world().get::<RatPhase>(same_chunk_b),
            Some(&RatPhase::Gregarious)
        );
        assert_eq!(
            app.world().get::<RatPhase>(other_group),
            Some(&RatPhase::Solitary)
        );
    }

    #[test]
    fn gregarious_phase_change_emits_rat_swarm_aura_vfx() {
        let mut app = App::new();
        app.add_event::<RatPhaseChangeEvent>();
        app.add_event::<VfxEventRequest>();
        app.add_systems(Update, emit_rat_swarm_aura_vfx_system);
        app.world_mut()
            .spawn((NpcMarker, Position::new([1.0, 64.0, 1.0]), RatGroupId(7)));
        app.world_mut()
            .spawn((NpcMarker, Position::new([3.0, 64.0, 1.0]), RatGroupId(7)));
        app.world_mut().send_event(RatPhaseChangeEvent {
            chunk: [0, 0],
            zone: "spawn".to_string(),
            group_id: 7,
            from: RatPhase::Transitioning {
                progress: TRANSITION_DURATION_TICKS - 1,
            },
            to: RatPhase::Gregarious,
            rat_count: 2,
            local_qi: 0.8,
            qi_gradient: 0.5,
            tick: 11,
        });

        app.update();

        let events = app.world().resource::<Events<VfxEventRequest>>();
        let event = events
            .iter_current_update_events()
            .next()
            .expect("gregarious rat group should emit swarm aura VFX");
        assert!(matches!(
            &event.payload,
            VfxEventPayloadV1::SpawnParticle { event_id, origin, .. }
                if event_id == "bong:rat_swarm_aura" && origin[0] == 2.0
        ));
    }

    #[test]
    fn apply_rat_phase_change_keeps_rats_that_already_left_source_phase() {
        let mut app = App::new();
        app.add_event::<RatPhaseChangeEvent>();
        app.add_systems(Update, apply_rat_phase_change_system);
        let solitary = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([1.0, 64.0, 1.0]),
                RatGroupId(7),
                RatPhase::Solitary,
            ))
            .id();
        let gregarious = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([2.0, 64.0, 2.0]),
                RatGroupId(7),
                RatPhase::Gregarious,
            ))
            .id();

        app.world_mut().send_event(RatPhaseChangeEvent {
            chunk: [0, 0],
            zone: "spawn".to_string(),
            group_id: 7,
            from: RatPhase::Solitary,
            to: RatPhase::Transitioning { progress: 0 },
            rat_count: 2,
            local_qi: 0.6,
            qi_gradient: 0.4,
            tick: 9,
        });
        app.update();

        assert_eq!(
            app.world().get::<RatPhase>(solitary),
            Some(&RatPhase::Transitioning { progress: 0 })
        );
        assert_eq!(
            app.world().get::<RatPhase>(gregarious),
            Some(&RatPhase::Gregarious),
            "later phases must not regress when another solitary rat emits a transition event"
        );
    }

    #[test]
    fn rat_death_transfers_full_drained_qi_to_zone_account() {
        // §P0 验收抓手 #2（替换旧 1% 归还 pin）——鼠死亡必须把 `npc:rat:<id>` 账户
        // 100% 转入 `zone:<name>` 账户（不再是 1%），并同步写回 `zone.spirit_qi` 字段
        // （§8.1 决议 #3 blocker 修正核心：漏写字段会被下一次覆盖式重同步二次抹掉）。
        //
        // ★promote 博弈 blocker v2 修正后的调值说明：drained_qi 从旧版 100.0 降到 10.0。
        // `QI_ZONE_UNIT_CAPACITY`=50 是 zone 账户的绝对上限（`spirit_qi` 恒 clamp 在
        // [-1,1]），旧版 100.0 在 initial_spirit_qi=0.5（room=25）下必然溢出 75——这正是
        // blocker 本身要修的"无条件全额转账不截断"bug，若不调值，本测试会在 accepted/overflow
        // 分流后自然由绿转红（账户被正确截到 cap=50，不再是旧断言的 125）。调小到 10.0 后
        // room 充足，accepted==full 10.0、overflow==0，专测"非满 zone 全额落袋"场景，不与
        // 下方 `rat_death_near_cap_zone_routes_overflow_conserving` 的满 zone/overflow 场景重叠。
        let mut app = App::new();
        app.add_event::<DeathEvent>();
        app.add_systems(Update, release_drained_qi_on_death_system);

        let initial_spirit_qi = 0.5_f64;
        let mut zones = ZoneRegistry::fallback();
        zones
            .find_zone_mut("spawn")
            .expect("fallback ZoneRegistry must have spawn zone")
            .spirit_qi = initial_spirit_qi;
        app.insert_resource(zones);

        let mut blackboard = rat_blackboard("spawn", ChunkPos::new(0, 0));
        blackboard.drained_qi = 10.0;
        let rat = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([0.0, 64.0, 0.0]),
                RatPhase::Gregarious,
                blackboard,
            ))
            .id();

        let rat_account = QiAccountId::npc(format!("rat:{}", rat.index()));
        let mut ledger = WorldQiAccount::default();
        ledger
            .set_balance(rat_account.clone(), 10.0)
            .expect("seeding the rat ledger balance must succeed");
        app.insert_resource(ledger);

        app.world_mut().send_event(DeathEvent {
            target: rat,
            cause: "test".to_string(),
            attacker: None,
            attacker_player_id: None,
            at_tick: 0,
        });
        app.update();

        let ledger_after = app.world().resource::<WorldQiAccount>();
        assert_eq!(
            ledger_after.balance(&rat_account),
            0.0,
            "鼠死亡后 npc:rat 账户必须清零（100% 转账，不再是只归还 1% 留 99% 僵尸余额）"
        );

        // 转账前，zone 账户先按 field-authority 三段式同步到 `spirit_qi * CAPACITY`
        // 真实值，再吸收 rat 账户全额 10.0（room=25 充足，accepted==full、overflow==0）——
        // 最终账户余额 = 同步基线 + 10.0。
        let zone_account = QiAccountId::zone("spawn");
        let expected_zone_account_balance =
            initial_spirit_qi.max(0.0) * QI_ZONE_UNIT_CAPACITY + 10.0;
        assert!(
            (ledger_after.balance(&zone_account) - expected_zone_account_balance).abs() < 1e-9,
            "鼠死亡必须把 drained_qi 100% 转入 zone:<name> 账户（§8.1 决议 #1/#3），\
             期望 {expected_zone_account_balance}，实际 {}",
            ledger_after.balance(&zone_account)
        );

        // room 充足场景不应产生 overflow，overflow 账户必须为空——确认 accepted/overflow
        // 分流在"非满 zone"场景下退化为纯 100% 落袋，不误伤既有行为。
        let overflow_account = QiAccountId::overflow(format!("rat_bite_drain:{}", rat_account.id));
        assert_eq!(
            ledger_after.balance(&overflow_account),
            0.0,
            "room 充足（25 > drained 10.0）时不应产生 overflow，overflow 账户应保持空账"
        );

        let zone_after = app
            .world()
            .resource::<ZoneRegistry>()
            .find_zone_by_name("spawn")
            .expect("spawn zone must still exist")
            .spirit_qi;
        let expected_spirit_qi =
            (expected_zone_account_balance / QI_ZONE_UNIT_CAPACITY).clamp(-1.0, 1.0);
        assert!(
            (zone_after - expected_spirit_qi).abs() < 1e-9,
            "§8.1 #3 blocker 修正：zone.spirit_qi 字段必须与账户转账同步写回（不能只改账户\
             不改字段），期望 {expected_spirit_qi}，实际 {zone_after}——相等于旧值说明字段被\
             漏写，下一次覆盖式重同步会二次抹掉刚转入的账户余额"
        );
    }

    /// ★守恒红线 blocker 的锁——zone 逼近 cap（0.98）时鼠转入必然溢出（room 只剩 1.0，
    /// drained 20.0 里 19.0 吃不下）。v1 bug：无条件 100% 转账只 clamp **字段**、不 clamp
    /// **账户**——账户越过 cap 后，字段被 clamp 成 1.0，下一次任何覆盖式重同步
    /// （`set_balance(zone_account, spirit_qi.max(0)*CAP)`，本函数被第二只鼠调用时开场即会
    /// 执行，`npc::dormant::apply_dormant_regen_with_multiplier` / `zone_qi_inflow_tick`
    /// 同款）都会把账户覆写回 `1.0*CAP`，无对偿销毁溢出的 19.0——`ledger_qi` 净减、无对应
    /// 增长，撞穿 qi_physics 守恒律。
    ///
    /// 本测试断言 v2 修法后的四点：① rat 账户全额抽干为 0 ② zone 账户被截到 cap（不越界）
    /// ③ overflow 账户吃住溢出量（不蒸发）④ 三账户合计（rat+zone+overflow）转账前后严格
    /// 守恒。随后额外调用第二只鼠（本身就会重新执行"同步 zone 账户到字段值"这条覆盖式
    /// 重同步逻辑）验证第一只鼠的 overflow 账户余额纹丝不动——证明溢出不会被下一次
    /// `set_balance` 覆盖销毁。
    ///
    /// 红→绿证据（本次实测）：把函数体换回 v1"无条件全额转账 + 只 clamp 字段"版本时，本测试
    /// 在 ③ 处撞红——`overflow_account` 余额断言 `19.0` 实际读到 `0.0`（旧版压根没有 overflow
    /// 账户这个概念，溢出的 19.0 单位随字段 clamp 直接从账本蒸发，只是账户余额仍留 125，
    /// 与 `zone_account_after=50.0` 的 cap 断言也对不上）。接回 v2 accepted/overflow 分流后
    /// 全部转绿。
    #[test]
    fn rat_death_near_cap_zone_routes_overflow_conserving() {
        const CAP: f64 = QI_ZONE_UNIT_CAPACITY;

        let mut ledger = WorldQiAccount::default();
        let mut zones = ZoneRegistry::fallback();
        let zone = zones
            .find_zone_mut("spawn")
            .expect("fallback ZoneRegistry must have spawn zone");
        let initial_spirit_qi = 0.98_f64;
        zone.spirit_qi = initial_spirit_qi;

        // 预置 zone 账户已与字段同步（照抄既有 end-to-end pin 的"活服务器已跑过一次
        // inflow tick"前提），让"转账前后三账户合计守恒"这条断言不掺入 mirror-sync 本身
        // 的初始化噪声。
        let zone_account = QiAccountId::zone("spawn");
        ledger
            .set_balance(zone_account.clone(), initial_spirit_qi * CAP)
            .expect("pre-syncing the zone ledger mirror must succeed");

        let rat1_account = QiAccountId::npc("rat:1".to_string());
        let rat1_drained = 20.0_f64;
        ledger
            .set_balance(rat1_account.clone(), rat1_drained)
            .expect("seeding rat1 ledger balance must succeed");

        let before_total = ledger.balance(&rat1_account) + ledger.balance(&zone_account);

        let accounted = transfer_rat_drained_qi_to_zone(&mut ledger, zone, &rat1_account)
            .expect("near-cap transfer must succeed (accepted/overflow split, never an error)");
        assert!(
            (accounted - rat1_drained).abs() < 1e-9,
            "返回值必须报告全部处理掉的 amount（accepted+overflow），期望 {rat1_drained}，实际 {accounted}"
        );

        let overflow_account = QiAccountId::overflow(format!("rat_bite_drain:{}", rat1_account.id));
        let room = (CAP - initial_spirit_qi * CAP).max(0.0); // 1.0
        let expected_accepted = rat1_drained.min(room); // 1.0
        let expected_overflow = rat1_drained - expected_accepted; // 19.0

        // ①rat 账户全额抽干。
        assert_eq!(
            ledger.balance(&rat1_account),
            0.0,
            "rat 账户必须被全额抽干（accepted+overflow 两腿相加 == amount），不留僵尸余额"
        );
        // ②zone 账户被截到 cap，不越界。
        let zone_account_after = ledger.balance(&zone_account);
        assert!(
            (zone_account_after - CAP).abs() < 1e-9,
            "zone 账户吸满 room 后必须精确停在 cap={CAP}（room={room}，accepted={expected_accepted}），\
             实际 {zone_account_after}——越过此值说明账户没有被截断，字段 clamp 只是掩盖了越界"
        );
        assert!(
            zone_account_after <= CAP + 1e-9,
            "zone 账户绝不可越过 cap={CAP}，实际 {zone_account_after}"
        );
        // ③overflow 账户吃住溢出量，不蒸发。
        let overflow_after = ledger.balance(&overflow_account);
        assert!(
            (overflow_after - expected_overflow).abs() < 1e-9,
            "溢出的 {expected_overflow} 必须被记入可审计 overflow 账户，实际 {overflow_after}——\
             不等说明溢出量在 clamp 字段时被无对偿销毁（★blocker 复现的蒸发路径）"
        );
        // ④三账户合计（rat+zone+overflow）转账前后严格守恒。
        let after_total =
            ledger.balance(&rat1_account) + ledger.balance(&zone_account) + overflow_after;
        assert!(
            (before_total - after_total).abs() < 1e-9,
            "worldview §二守恒律：rat+zone+overflow 三账户合计转账前后必须严格相等，\
             before={before_total} after={after_total}——不等说明真元在 clamp 字段/账户失配处\
             凭空蒸发或创生"
        );

        // zone.spirit_qi 字段必须与账户同步写回、且不越 ZONE_SPIRIT_QI_MAX。
        let zone_after_field = zones
            .find_zone_by_name("spawn")
            .expect("spawn zone must still exist")
            .spirit_qi;
        assert!(
            (zone_after_field - ZONE_SPIRIT_QI_MAX).abs() < 1e-9,
            "zone 吸满后字段必须精确停在 ZONE_SPIRIT_QI_MAX={ZONE_SPIRIT_QI_MAX}，实际 {zone_after_field}"
        );

        // 再关键：第二只鼠调用本函数——开场就会执行"同步 zone 账户到字段值"这条覆盖式
        // 重同步逻辑（`set_balance(zone_account, zone.spirit_qi.max(0)*CAP)`），验证它不会
        // 把 rat1 的 overflow 账户覆写销毁。
        let zone = zones
            .find_zone_mut("spawn")
            .expect("spawn zone must still exist for second rat call");
        let rat2_account = QiAccountId::npc("rat:2".to_string());
        let rat2_drained = 5.0_f64;
        ledger
            .set_balance(rat2_account.clone(), rat2_drained)
            .expect("seeding rat2 ledger balance must succeed");

        transfer_rat_drained_qi_to_zone(&mut ledger, zone, &rat2_account)
            .expect("second near-cap transfer must also succeed");

        assert_eq!(
            ledger.balance(&overflow_account),
            overflow_after,
            "第二只鼠触发的覆盖式重同步（set_balance(zone_account, ...)）绝不能动 rat1 的\
             overflow 账户——溢出账户与 zone 账户是不同 key，重同步只应触碰 zone 账户"
        );
        assert_eq!(
            ledger.balance(&rat2_account),
            0.0,
            "zone 已满（room=0）时 rat2 也必须被全额抽干（100% 转入 rat2 自己的 overflow 账户）"
        );
        let rat2_overflow_account =
            QiAccountId::overflow(format!("rat_bite_drain:{}", rat2_account.id));
        assert_eq!(
            ledger.balance(&rat2_overflow_account),
            rat2_drained,
            "zone 已满时 rat2 的 drained_qi 必须 100% 落进它自己的 overflow 账户，不蒸发"
        );
        let zone_account_still_capped = ledger.balance(&zone_account);
        assert!(
            (zone_account_still_capped - CAP).abs() < 1e-9,
            "zone 已在 cap，第二只鼠不应再让账户继续增长，实际 {zone_account_still_capped}"
        );
    }

    #[test]
    fn drained_chunk_avoid_still_works_in_gregarious_phase() {
        let chunk = ChunkPos::new(1, -1);
        let mut blackboard = rat_blackboard("spawn", chunk);
        remember_drained_chunk(&mut blackboard, chunk);

        assert!(is_drained_chunk(&blackboard, chunk));
    }
}
