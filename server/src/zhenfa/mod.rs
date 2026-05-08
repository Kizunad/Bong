use std::collections::{HashMap, HashSet, VecDeque};

use serde::{Deserialize, Serialize};
use valence::prelude::{
    bevy_ecs, App, Client, Commands, Component, Entity, Event, EventReader, EventWriter,
    IntoSystemConfigs, Position, Query, Res, ResMut, Resource, Update, Username, With, Without,
};

use crate::combat::components::{BodyPart, Lifecycle, LifecycleState, Wound, WoundKind, Wounds};
use crate::combat::events::{ApplyStatusEffectIntent, CombatEvent, DeathEvent, StatusEffectKind};
use crate::combat::CombatClock;
use crate::cultivation::color::{record_style_practice, PracticeLog};
use crate::cultivation::components::{
    ColorKind, ContamSource, Contamination, Cultivation, MeridianId, MeridianSystem, QiColor,
};
use crate::cultivation::insight_apply::InsightModifiers;
use crate::inventory::{
    add_item_to_player_inventory, InventoryInstanceIdAllocator, ItemRegistry, PlayerInventory,
};
use crate::player::gameplay::PendingGameplayNarrations;
use crate::player::state::canonical_player_id;
use crate::qi_physics::{CarrierGrade, MediumKind, StyleAttack, StyleDefense};
use crate::schema::common::NarrationStyle;
use crate::schema::realm_vision::{SenseEntryV1, SenseKindV1, SpiritualSenseTargetsV1};

const TICKS_PER_SECOND: u64 = 20;
const MIN_QI_INVEST_RATIO: f64 = 0.05;
const ZHENFA_FLAG_ITEM_ID: &str = "array_flag";
const ZHENFA_PEARL_ITEM_ID: &str = "scattered_qi_pearl";
const CHAIN_DELAY_TICKS: u64 = 6;
const WARD_ALERT_THROTTLE_TICKS: u64 = 60 * TICKS_PER_SECOND;
const DISARM_RANGE: f64 = 4.5;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ZhenfaKind {
    Trap,
    Ward,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ZhenfaCarrierKind {
    CommonStone,
    LingqiBlock,
    NightWitheredVine,
    BeastCoreInlaid,
}

impl Default for ZhenfaCarrierKind {
    fn default() -> Self {
        Self::CommonStone
    }
}

impl ZhenfaCarrierKind {
    fn carrier_grade(self) -> CarrierGrade {
        match self {
            Self::CommonStone | Self::NightWitheredVine => CarrierGrade::PhysicalWeapon,
            Self::LingqiBlock => CarrierGrade::SpiritWeapon,
            Self::BeastCoreInlaid => CarrierGrade::AncientRelic,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ZhenfaDisarmMode {
    Disarm,
    ForceBreak,
}

#[derive(Debug, Clone, Event)]
pub struct ZhenfaPlaceRequest {
    pub player: Entity,
    pub pos: [i32; 3],
    pub kind: ZhenfaKind,
    pub carrier: ZhenfaCarrierKind,
    pub qi_invest_ratio: f64,
    pub trigger: Option<String>,
    pub requested_at_tick: u64,
}

#[derive(Debug, Clone, Event)]
pub struct ZhenfaTriggerRequest {
    pub player: Entity,
    pub instance_id: Option<u64>,
    pub requested_at_tick: u64,
}

#[derive(Debug, Clone, Event)]
pub struct ZhenfaDisarmRequest {
    pub player: Entity,
    pub pos: [i32; 3],
    pub mode: ZhenfaDisarmMode,
    pub requested_at_tick: u64,
}

#[derive(Debug, Clone, Event)]
pub struct ZhenfaSensePulse {
    pub owner: Entity,
    pub kind: SenseKindV1,
    pub pos: [i32; 3],
    pub intensity: f64,
    pub generation: u64,
}

#[derive(Debug, Clone, Copy, Component, PartialEq, Eq)]
pub struct ZhenfaAnchor {
    pub id: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ZhenfaInstance {
    pub id: u64,
    pub kind: ZhenfaKind,
    pub owner: Entity,
    pub owner_player_id: String,
    pub pos: [i32; 3],
    pub carrier: ZhenfaCarrierKind,
    pub qi_invest_ratio: f64,
    pub qi_invest_amount: f64,
    pub effect_radius: u8,
    pub ward_radius: u8,
    pub placed_at_tick: u64,
    pub expires_at_tick: u64,
    pub triggered_at: Option<u64>,
    pub trigger: Option<String>,
    pub color_main: ColorKind,
    pub color_secondary: Option<ColorKind>,
    pub anchor_entity: Entity,
}

impl StyleAttack for ZhenfaInstance {
    fn style_color(&self) -> ColorKind {
        self.color_main
    }

    fn injected_qi(&self) -> f64 {
        self.qi_invest_amount.max(0.0)
    }

    fn purity(&self) -> f64 {
        self.qi_invest_ratio.clamp(0.0, 1.0)
    }

    fn medium(&self) -> MediumKind {
        MediumKind {
            color: self.color_main,
            carrier: self.carrier.carrier_grade(),
        }
    }
}

impl StyleDefense for ZhenfaInstance {
    fn defense_color(&self) -> ColorKind {
        self.color_secondary.unwrap_or(self.color_main)
    }

    fn resistance(&self) -> f64 {
        f64::from(self.ward_radius) / 16.0
    }

    fn drain_affinity(&self) -> f64 {
        self.qi_invest_ratio.clamp(0.0, 1.0) * 0.25
    }
}

#[derive(Debug, Clone, PartialEq)]
struct TriggerSnapshot {
    id: u64,
    owner: Entity,
    owner_player_id: String,
    pos: [i32; 3],
    triggered_at_tick: u64,
    qi_invest_ratio: f64,
    effect_radius: u8,
    color_main: ColorKind,
    color_secondary: Option<ColorKind>,
    anchor_entity: Entity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PendingChainTrigger {
    id: u64,
    due_tick: u64,
}

#[derive(Debug, Default, Resource)]
pub struct ZhenfaRegistry {
    next_id: u64,
    instances: HashMap<u64, ZhenfaInstance>,
    by_pos: HashMap<[i32; 3], u64>,
    pending_chain: VecDeque<PendingChainTrigger>,
    ward_alert_seen: HashMap<(u64, Entity), u64>,
    ward_inside: HashSet<(u64, Entity)>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CarrierSpec {
    pub cap_ratio: f64,
    pub duration_ticks: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZhenfaSpecialistLevel {
    None,
    Novice,
    Expert,
}

pub fn register(app: &mut App) {
    tracing::info!("[bong][zhenfa] registering zhenfa systems");
    app.insert_resource(ZhenfaRegistry::default());
    app.add_event::<ZhenfaPlaceRequest>();
    app.add_event::<ZhenfaTriggerRequest>();
    app.add_event::<ZhenfaDisarmRequest>();
    app.add_event::<ZhenfaSensePulse>();
    app.add_systems(
        Update,
        (
            handle_zhenfa_place_requests,
            handle_zhenfa_trigger_requests,
            handle_zhenfa_disarm_requests,
            tick_zhenfa_registry,
            emit_zhenfa_sense_pulses,
        )
            .chain(),
    );
}

impl ZhenfaRegistry {
    pub fn insert(&mut self, mut instance: ZhenfaInstance) -> Result<u64, String> {
        if self.by_pos.contains_key(&instance.pos) {
            return Err(format!(
                "zhenfa position {:?} already has an array eye",
                instance.pos
            ));
        }

        let id = self.allocate_id();
        instance.id = id;
        self.by_pos.insert(instance.pos, id);
        self.instances.insert(id, instance);
        Ok(id)
    }

    pub fn get(&self, id: u64) -> Option<&ZhenfaInstance> {
        self.instances.get(&id)
    }

    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.instances.len()
    }

    pub fn find_at(&self, pos: [i32; 3]) -> Option<&ZhenfaInstance> {
        self.by_pos.get(&pos).and_then(|id| self.instances.get(id))
    }

    #[allow(dead_code)]
    pub fn find_owned_by(&self, owner: Entity) -> Vec<&ZhenfaInstance> {
        let mut owned = self
            .instances
            .values()
            .filter(|instance| instance.owner == owner)
            .collect::<Vec<_>>();
        owned.sort_by_key(|instance| (instance.placed_at_tick, instance.id));
        owned
    }

    fn allocate_id(&mut self) -> u64 {
        self.next_id = self.next_id.saturating_add(1).max(1);
        self.next_id
    }

    fn active_instances(&self) -> impl Iterator<Item = &ZhenfaInstance> {
        self.instances.values().filter(|instance| {
            instance.triggered_at.is_none()
                && !self.pending_chain.iter().any(|p| p.id == instance.id)
        })
    }

    fn remove(&mut self, id: u64) -> Option<ZhenfaInstance> {
        let removed = self.instances.remove(&id)?;
        self.by_pos.remove(&removed.pos);
        self.pending_chain.retain(|pending| pending.id != id);
        self.ward_alert_seen
            .retain(|(array_id, _), _| *array_id != id);
        self.ward_inside.retain(|(array_id, _)| *array_id != id);
        Some(removed)
    }

    fn expire_at_or_before(&mut self, tick: u64) -> Vec<ZhenfaInstance> {
        let expired = self
            .instances
            .iter()
            .filter_map(|(id, instance)| (instance.expires_at_tick <= tick).then_some(*id))
            .collect::<Vec<_>>();

        expired
            .into_iter()
            .filter_map(|id| self.remove(id))
            .collect()
    }

    fn trigger_now(
        &mut self,
        ids: impl IntoIterator<Item = u64>,
        tick: u64,
    ) -> Vec<TriggerSnapshot> {
        let mut snapshots = Vec::new();
        let mut seen = HashSet::new();
        for id in ids {
            if !seen.insert(id) {
                continue;
            }
            let Some(instance) = self.instances.get_mut(&id) else {
                continue;
            };
            if instance.triggered_at.is_some() {
                continue;
            }
            instance.triggered_at = Some(tick);
            snapshots.push(TriggerSnapshot {
                id: instance.id,
                owner: instance.owner,
                owner_player_id: instance.owner_player_id.clone(),
                pos: instance.pos,
                triggered_at_tick: tick,
                qi_invest_ratio: instance.qi_invest_ratio,
                effect_radius: instance.effect_radius,
                color_main: instance.color_main,
                color_secondary: instance.color_secondary,
                anchor_entity: instance.anchor_entity,
            });
        }

        for snapshot in &snapshots {
            self.schedule_neighbors(snapshot.id, snapshot.pos, tick);
        }
        for snapshot in &snapshots {
            self.remove(snapshot.id);
        }

        snapshots
    }

    fn drain_due_chain_triggers(&mut self, tick: u64) -> Vec<TriggerSnapshot> {
        let mut due_ids = Vec::new();
        let mut kept = VecDeque::new();
        while let Some(pending) = self.pending_chain.pop_front() {
            if pending.due_tick <= tick {
                due_ids.push(pending.id);
            } else {
                kept.push_back(pending);
            }
        }
        self.pending_chain = kept;
        self.trigger_now(due_ids, tick)
    }

    fn schedule_neighbors(&mut self, source_id: u64, source_pos: [i32; 3], tick: u64) {
        let mut neighbors = self
            .instances
            .values()
            .filter(|instance| instance.kind == ZhenfaKind::Trap)
            .filter(|instance| instance.id != source_id)
            .filter(|instance| instance.triggered_at.is_none())
            .filter(|instance| chebyshev_distance(instance.pos, source_pos) <= 1)
            .filter(|instance| {
                !self
                    .pending_chain
                    .iter()
                    .any(|pending| pending.id == instance.id)
            })
            .map(|instance| {
                (
                    squared_distance_i32(instance.pos, source_pos),
                    instance.placed_at_tick,
                    instance.id,
                )
            })
            .collect::<Vec<_>>();

        neighbors.sort_unstable();
        for (_, _, id) in neighbors {
            self.pending_chain.push_back(PendingChainTrigger {
                id,
                due_tick: tick.saturating_add(CHAIN_DELAY_TICKS),
            });
        }
    }
}

pub fn carrier_spec(carrier: ZhenfaCarrierKind) -> CarrierSpec {
    match carrier {
        ZhenfaCarrierKind::CommonStone => CarrierSpec {
            cap_ratio: 0.10,
            duration_ticks: 30 * 60 * TICKS_PER_SECOND,
        },
        ZhenfaCarrierKind::LingqiBlock => CarrierSpec {
            cap_ratio: 0.20,
            duration_ticks: 2 * 60 * 60 * TICKS_PER_SECOND,
        },
        ZhenfaCarrierKind::NightWitheredVine => CarrierSpec {
            cap_ratio: 0.30,
            duration_ticks: 12 * 60 * 60 * TICKS_PER_SECOND,
        },
        ZhenfaCarrierKind::BeastCoreInlaid => CarrierSpec {
            cap_ratio: 0.50,
            duration_ticks: 24 * 60 * 60 * TICKS_PER_SECOND,
        },
    }
}

pub fn zhenfa_specialist_level(modifiers: Option<&InsightModifiers>) -> ZhenfaSpecialistLevel {
    let score = modifiers
        .map(|m| m.zhenfa_concealment + m.zhenfa_disenchant)
        .unwrap_or(0.0);
    if score >= 3.0 {
        ZhenfaSpecialistLevel::Expert
    } else if score > 0.0 {
        ZhenfaSpecialistLevel::Novice
    } else {
        ZhenfaSpecialistLevel::None
    }
}

pub fn zhenfa_disarm_chance(modifiers: Option<&InsightModifiers>) -> f64 {
    let bonus = modifiers.map(|m| m.zhenfa_disenchant).unwrap_or(0.0) * 0.10;
    (0.30 + bonus).clamp(0.30, 0.80)
}

fn handle_zhenfa_place_requests(
    mut requests: EventReader<ZhenfaPlaceRequest>,
    mut registry: ResMut<ZhenfaRegistry>,
    mut commands: Commands,
    mut players: Query<ZhenfaPlacePlayer<'_>>,
) {
    for req in requests.read() {
        if registry.find_at(req.pos).is_some() {
            tracing::warn!(
                "[bong][zhenfa] place rejected: pos={:?} already has an array eye",
                req.pos
            );
            continue;
        }

        let Ok((username, mut cultivation, qi_color, modifiers, inventory)) =
            players.get_mut(req.player)
        else {
            tracing::warn!(
                "[bong][zhenfa] place rejected: player {:?} missing cultivation bundle",
                req.player
            );
            continue;
        };
        if !has_zhenfa_flag(inventory) {
            tracing::warn!(
                "[bong][zhenfa] place rejected: player {:?} has no array flag",
                req.player
            );
            continue;
        }

        let spec = carrier_spec(req.carrier);
        let invest_ratio = sanitize_invest_ratio(req.qi_invest_ratio, spec.cap_ratio);
        let qi_cost = cultivation.qi_max.max(1.0) * invest_ratio;
        if cultivation.qi_current + f64::EPSILON < qi_cost {
            tracing::warn!(
                "[bong][zhenfa] place rejected: player {:?} qi_current {:.3} < cost {:.3}",
                req.player,
                cultivation.qi_current,
                qi_cost
            );
            continue;
        }

        cultivation.qi_current = (cultivation.qi_current - qi_cost).max(0.0);
        let specialist = zhenfa_specialist_level(modifiers);
        let duration_ticks = effective_duration_ticks(spec.duration_ticks, qi_color, specialist);
        let anchor_entity = commands
            .spawn((
                ZhenfaAnchor { id: 0 },
                Position::new([
                    req.pos[0] as f64 + 0.5,
                    req.pos[1] as f64,
                    req.pos[2] as f64 + 0.5,
                ]),
            ))
            .id();

        let instance = ZhenfaInstance {
            id: 0,
            kind: req.kind,
            owner: req.player,
            owner_player_id: canonical_player_id(username.0.as_str()),
            pos: req.pos,
            carrier: req.carrier,
            qi_invest_ratio: invest_ratio,
            qi_invest_amount: qi_cost,
            effect_radius: trap_effect_radius(invest_ratio),
            ward_radius: ward_radius(invest_ratio, specialist),
            placed_at_tick: req.requested_at_tick,
            expires_at_tick: req.requested_at_tick.saturating_add(duration_ticks),
            triggered_at: None,
            trigger: req.trigger.clone(),
            color_main: qi_color.main,
            color_secondary: qi_color.secondary,
            anchor_entity,
        };

        match registry.insert(instance) {
            Ok(id) => {
                commands.entity(anchor_entity).insert(ZhenfaAnchor { id });
                tracing::info!(
                    "[bong][zhenfa] placed {:?} id={} owner={:?} pos={:?} ratio={:.3}",
                    req.kind,
                    id,
                    req.player,
                    req.pos,
                    invest_ratio
                );
            }
            Err(error) => {
                tracing::warn!("[bong][zhenfa] place failed after qi debit: {error}");
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_zhenfa_trigger_requests(
    mut requests: EventReader<ZhenfaTriggerRequest>,
    mut registry: ResMut<ZhenfaRegistry>,
    mut commands: Commands,
    players: Query<ZhenfaTriggerPlayer<'_>>,
    mut targets: Query<ZhenfaDamageTarget<'_>>,
    mut practice_logs: Query<&mut PracticeLog>,
    mut combat_events: EventWriter<CombatEvent>,
    mut death_events: EventWriter<DeathEvent>,
    mut status_effects: EventWriter<ApplyStatusEffectIntent>,
    mut sense_pulses: EventWriter<ZhenfaSensePulse>,
) {
    for req in requests.read() {
        let Ok((position, cultivation, qi_color, inventory)) = players.get(req.player) else {
            tracing::warn!(
                "[bong][zhenfa] active trigger rejected: player {:?} missing position/cultivation",
                req.player
            );
            continue;
        };
        if !has_zhenfa_flag(inventory) {
            tracing::warn!(
                "[bong][zhenfa] active trigger rejected: player {:?} has no array flag",
                req.player
            );
            continue;
        }

        let player_pos = position.get();
        let sense_range = active_trigger_range(cultivation, qi_color);
        let selected = match req.instance_id {
            Some(id) => registry.get(id).and_then(|instance| {
                let in_range = distance_to_block(player_pos, instance.pos) <= sense_range;
                (instance.owner == req.player
                    && instance.kind == ZhenfaKind::Trap
                    && instance.triggered_at.is_none()
                    && in_range)
                    .then_some(id)
            }),
            None => registry
                .active_instances()
                .filter(|instance| instance.owner == req.player)
                .filter(|instance| instance.kind == ZhenfaKind::Trap)
                .filter(|instance| distance_to_block(player_pos, instance.pos) <= sense_range)
                .map(|instance| {
                    (
                        ordered_distance_to_block(player_pos, instance.pos),
                        instance.placed_at_tick,
                        instance.id,
                    )
                })
                .min()
                .map(|(_, _, id)| id),
        };

        let Some(id) = selected else {
            tracing::debug!(
                "[bong][zhenfa] active trigger found no owned trap for player {:?}",
                req.player
            );
            continue;
        };

        let snapshots = registry.trigger_now([id], req.requested_at_tick);
        despawn_triggered_anchors(&mut commands, &snapshots);
        apply_trigger_snapshots(
            snapshots,
            &mut targets,
            &mut practice_logs,
            &mut combat_events,
            &mut death_events,
            &mut status_effects,
            &mut sense_pulses,
        );
    }
}

type ZhenfaDamageTarget<'a> = (
    Entity,
    &'a Position,
    &'a mut Wounds,
    Option<&'a Lifecycle>,
    Option<&'a Username>,
    Option<&'a mut Contamination>,
    Option<&'a mut MeridianSystem>,
);

type ZhenfaPlacePlayer<'a> = (
    &'a Username,
    &'a mut Cultivation,
    &'a QiColor,
    Option<&'a InsightModifiers>,
    Option<&'a PlayerInventory>,
);

type ZhenfaTriggerPlayer<'a> = (
    &'a Position,
    &'a Cultivation,
    &'a QiColor,
    Option<&'a PlayerInventory>,
);

type ZhenfaDisarmPlayer<'a> = (
    &'a Position,
    &'a mut Wounds,
    Option<&'a mut Contamination>,
    Option<&'a mut MeridianSystem>,
    Option<&'a InsightModifiers>,
    Option<&'a mut PlayerInventory>,
);

#[allow(clippy::too_many_arguments)]
fn tick_zhenfa_registry(
    clock: Res<CombatClock>,
    mut registry: ResMut<ZhenfaRegistry>,
    mut commands: Commands,
    mut targets: Query<ZhenfaDamageTarget<'_>>,
    mut practice_logs: Query<&mut PracticeLog>,
    ward_positions: Query<(Entity, &Position), Without<ZhenfaAnchor>>,
    mut combat_events: EventWriter<CombatEvent>,
    mut death_events: EventWriter<DeathEvent>,
    mut status_effects: EventWriter<ApplyStatusEffectIntent>,
    mut sense_pulses: EventWriter<ZhenfaSensePulse>,
    mut pending_narrations: Option<ResMut<PendingGameplayNarrations>>,
) {
    let now = clock.tick;
    let expired = registry.expire_at_or_before(now);
    if !expired.is_empty() {
        tracing::debug!("[bong][zhenfa] expired {} array eye(s)", expired.len());
    }
    for instance in &expired {
        commands.entity(instance.anchor_entity).despawn();
    }

    let mut passive_triggers = Vec::new();
    let mut ward_alerts = Vec::new();
    let mut current_ward_inside = HashSet::new();
    for instance in registry
        .active_instances()
        .filter(|instance| instance.placed_at_tick < now)
    {
        match instance.kind {
            ZhenfaKind::Trap => {
                for (target, position, ..) in &mut targets {
                    if target == instance.owner {
                        continue;
                    }
                    let pos = position.get();
                    if in_horizontal_radius(pos, instance.pos, instance.effect_radius) {
                        passive_triggers.push(instance.id);
                        break;
                    }
                }
            }
            ZhenfaKind::Ward => {
                for (target, position) in &ward_positions {
                    if target == instance.owner {
                        continue;
                    }
                    let pos = position.get();
                    if in_horizontal_radius(pos, instance.pos, instance.ward_radius) {
                        let key = (instance.id, target);
                        current_ward_inside.insert(key);
                        if registry.ward_inside.contains(&key) {
                            continue;
                        }
                        let last = registry.ward_alert_seen.get(&key).copied();
                        if last.is_none_or(|tick| {
                            now.saturating_sub(tick) >= WARD_ALERT_THROTTLE_TICKS
                        }) {
                            ward_alerts.push((
                                instance.id,
                                target,
                                instance.owner,
                                instance.owner_player_id.clone(),
                                instance.pos,
                            ));
                        }
                    }
                }
            }
        }
    }
    registry
        .ward_inside
        .retain(|key| current_ward_inside.contains(key));
    registry.ward_inside.extend(current_ward_inside);

    for (id, intruder, owner, owner_player_id, pos) in ward_alerts {
        registry.ward_alert_seen.insert((id, intruder), now);
        if let Some(pending_narrations) = pending_narrations.as_deref_mut() {
            pending_narrations.push_player(
                owner_player_id.as_str(),
                "你心头一颤，布下的警戒场传回一缕陌生气机。",
                NarrationStyle::Perception,
            );
        }
        sense_pulses.send(ZhenfaSensePulse {
            owner,
            kind: SenseKindV1::ZhenfaWardAlert,
            pos,
            intensity: 1.0,
            generation: now,
        });
    }

    let mut snapshots = registry.trigger_now(passive_triggers, now);
    snapshots.extend(registry.drain_due_chain_triggers(now));
    despawn_triggered_anchors(&mut commands, &snapshots);
    apply_trigger_snapshots(
        snapshots,
        &mut targets,
        &mut practice_logs,
        &mut combat_events,
        &mut death_events,
        &mut status_effects,
        &mut sense_pulses,
    );
}

#[allow(clippy::too_many_arguments)]
fn handle_zhenfa_disarm_requests(
    mut requests: EventReader<ZhenfaDisarmRequest>,
    mut registry: ResMut<ZhenfaRegistry>,
    mut commands: Commands,
    mut players: Query<ZhenfaDisarmPlayer<'_>>,
    item_registry: Option<Res<ItemRegistry>>,
    mut allocator: Option<ResMut<InventoryInstanceIdAllocator>>,
) {
    for req in requests.read() {
        let Ok((position, mut wounds, contamination, meridians, modifiers, inventory)) =
            players.get_mut(req.player)
        else {
            tracing::warn!(
                "[bong][zhenfa] disarm rejected: player {:?} missing required components",
                req.player
            );
            continue;
        };
        if distance_to_block(position.get(), req.pos) > DISARM_RANGE {
            tracing::warn!(
                "[bong][zhenfa] disarm rejected: player {:?} too far from {:?}",
                req.player,
                req.pos
            );
            continue;
        }

        let Some(instance_id) = registry.find_at(req.pos).map(|instance| instance.id) else {
            tracing::debug!(
                "[bong][zhenfa] disarm ignored: no array eye at {:?}",
                req.pos
            );
            continue;
        };
        let Some(instance) = registry.remove(instance_id) else {
            continue;
        };
        commands.entity(instance.anchor_entity).despawn();

        match req.mode {
            ZhenfaDisarmMode::ForceBreak => {
                apply_backlash(
                    req.player,
                    &mut wounds,
                    contamination,
                    meridians,
                    req.requested_at_tick,
                    backlash_contam_delta(instance.kind),
                );
            }
            ZhenfaDisarmMode::Disarm => {
                let chance = zhenfa_disarm_chance(modifiers);
                let roll = deterministic_roll(req.player, instance.id, instance.pos);
                if roll <= chance {
                    if let (Some(mut inventory), Some(registry), Some(allocator)) = (
                        inventory,
                        item_registry.as_deref(),
                        allocator.as_deref_mut(),
                    ) {
                        if let Err(error) = add_item_to_player_inventory(
                            &mut inventory,
                            registry,
                            allocator,
                            ZHENFA_PEARL_ITEM_ID,
                            1,
                        ) {
                            tracing::warn!(
                                "[bong][zhenfa] disarm succeeded but pearl grant failed: {error}"
                            );
                        }
                    }
                }
            }
        }
    }
}

fn emit_zhenfa_sense_pulses(
    mut pulses: EventReader<ZhenfaSensePulse>,
    mut clients: Query<(Entity, &mut Client), With<Client>>,
) {
    for pulse in pulses.read() {
        let Ok((_, mut client)) = clients.get_mut(pulse.owner) else {
            continue;
        };
        crate::cultivation::spiritual_sense::push::send_spiritual_sense_targets(
            &mut client,
            SpiritualSenseTargetsV1 {
                entries: vec![SenseEntryV1 {
                    kind: pulse.kind,
                    x: f64::from(pulse.pos[0]) + 0.5,
                    y: f64::from(pulse.pos[1]),
                    z: f64::from(pulse.pos[2]) + 0.5,
                    intensity: pulse.intensity.clamp(0.0, 1.0),
                }],
                generation: pulse.generation,
            },
        );
    }
}

fn apply_trigger_snapshots(
    snapshots: Vec<TriggerSnapshot>,
    targets: &mut Query<ZhenfaDamageTarget<'_>>,
    practice_logs: &mut Query<&mut PracticeLog>,
    combat_events: &mut EventWriter<CombatEvent>,
    death_events: &mut EventWriter<DeathEvent>,
    status_effects: &mut EventWriter<ApplyStatusEffectIntent>,
    sense_pulses: &mut EventWriter<ZhenfaSensePulse>,
) {
    for snapshot in snapshots {
        let tick = snapshot.triggered_at_tick;
        sense_pulses.send(ZhenfaSensePulse {
            owner: snapshot.owner,
            kind: SenseKindV1::ZhenfaArray,
            pos: snapshot.pos,
            intensity: 1.0,
            generation: tick,
        });

        let damage_profile = damage_profile(snapshot.qi_invest_ratio);
        let mut hit_any = false;
        for (target, position, mut wounds, lifecycle, username, contamination, meridians) in
            targets.iter_mut()
        {
            if target == snapshot.owner {
                continue;
            }
            if !in_horizontal_radius(position.get(), snapshot.pos, snapshot.effect_radius) {
                continue;
            }
            hit_any = true;

            let was_alive = wounds.health_current > 0.0;
            wounds.health_current =
                (wounds.health_current - damage_profile.damage).clamp(0.0, wounds.health_max);
            for leg in [BodyPart::LegL, BodyPart::LegR] {
                wounds.entries.push(Wound {
                    location: leg,
                    kind: WoundKind::Pierce,
                    severity: damage_profile.severity,
                    bleeding_per_sec: damage_profile.bleeding_per_sec,
                    created_at_tick: tick,
                    inflicted_by: Some(format!("zhenfa_trap:{}", snapshot.id)),
                });
            }

            if let Some(mut meridians) = meridians {
                for id in [MeridianId::Bladder, MeridianId::Kidney] {
                    let meridian = meridians.get_mut(id);
                    meridian.integrity =
                        (meridian.integrity - damage_profile.meridian_integrity_loss).max(0.0);
                }
            }

            let contam_delta = trap_contam_delta(snapshot.color_main, snapshot.color_secondary);
            if contam_delta > 0.0 {
                if let Some(mut contamination) = contamination {
                    contamination.entries.push(ContamSource {
                        amount: contam_delta,
                        color: snapshot.color_main,
                        attacker_id: Some(snapshot.owner_player_id.clone()),
                        introduced_at: tick,
                    });
                }
            }

            if color_matches(
                snapshot.color_main,
                snapshot.color_secondary,
                ColorKind::Violent,
            ) {
                status_effects.send(ApplyStatusEffectIntent {
                    target,
                    kind: StatusEffectKind::Stunned,
                    magnitude: 0.35,
                    duration_ticks: TICKS_PER_SECOND,
                    issued_at_tick: tick,
                });
            }

            combat_events.send(CombatEvent {
                attacker: snapshot.owner,
                target,
                resolved_at_tick: tick,
                body_part: BodyPart::LegL,
                wound_kind: WoundKind::Pierce,
                source: crate::combat::events::AttackSource::Melee,
                damage: damage_profile.damage,
                contam_delta,
                description: format!(
                    "zhenfa_trap {} -> {:?} ratio {:.3}",
                    snapshot.id, target, snapshot.qi_invest_ratio
                ),
                defense_kind: None,
                defense_effectiveness: None,
                defense_contam_reduced: None,
                defense_wound_severity: None,
            });

            if was_alive
                && wounds.health_current <= 0.0
                && !lifecycle.is_some_and(|lifecycle| {
                    matches!(
                        lifecycle.state,
                        LifecycleState::NearDeath | LifecycleState::Terminated
                    )
                })
            {
                let attacker_player_id = Some(snapshot.owner_player_id.clone());
                let cause_target = username
                    .map(|username| canonical_player_id(username.0.as_str()))
                    .unwrap_or_else(|| format!("entity:{:?}", target));
                death_events.send(DeathEvent {
                    target,
                    cause: format!("zhenfa_trap:{cause_target}"),
                    attacker: Some(snapshot.owner),
                    attacker_player_id,
                    at_tick: tick,
                });
            }
        }
        if hit_any {
            if let Ok(mut practice_log) = practice_logs.get_mut(snapshot.owner) {
                record_style_practice(&mut practice_log, ColorKind::Intricate);
            }
        }
    }
}

fn despawn_triggered_anchors(commands: &mut Commands, snapshots: &[TriggerSnapshot]) {
    for snapshot in snapshots {
        commands.entity(snapshot.anchor_entity).despawn();
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct DamageProfile {
    damage: f32,
    severity: f32,
    bleeding_per_sec: f32,
    meridian_integrity_loss: f64,
}

fn damage_profile(ratio: f64) -> DamageProfile {
    if ratio > 0.30 {
        DamageProfile {
            damage: 42.0,
            severity: 0.95,
            bleeding_per_sec: 0.25,
            meridian_integrity_loss: 0.35,
        }
    } else if ratio >= 0.15 {
        DamageProfile {
            damage: 28.0,
            severity: 0.65,
            bleeding_per_sec: 0.18,
            meridian_integrity_loss: 0.18,
        }
    } else if ratio >= 0.05 {
        DamageProfile {
            damage: 14.0,
            severity: 0.35,
            bleeding_per_sec: 0.08,
            meridian_integrity_loss: 0.08,
        }
    } else {
        DamageProfile {
            damage: 5.0,
            severity: 0.15,
            bleeding_per_sec: 0.02,
            meridian_integrity_loss: 0.02,
        }
    }
}

fn sanitize_invest_ratio(requested: f64, cap: f64) -> f64 {
    if !requested.is_finite() {
        return MIN_QI_INVEST_RATIO.min(cap);
    }
    requested.clamp(MIN_QI_INVEST_RATIO, cap)
}

fn trap_effect_radius(ratio: f64) -> u8 {
    if ratio > 0.30 {
        2
    } else if ratio >= 0.15 {
        1
    } else {
        0
    }
}

fn ward_radius(ratio: f64, specialist: ZhenfaSpecialistLevel) -> u8 {
    let base = if ratio > 0.30 {
        12.0
    } else if ratio >= 0.15 {
        8.0
    } else if ratio >= 0.05 {
        5.0
    } else {
        3.0
    };
    let factor = match specialist {
        ZhenfaSpecialistLevel::None => 0.5,
        ZhenfaSpecialistLevel::Novice => 0.75,
        ZhenfaSpecialistLevel::Expert => 1.0,
    };
    let radius: f64 = base * factor;
    radius.round().max(1.0) as u8
}

fn effective_duration_ticks(
    base_ticks: u64,
    qi_color: &QiColor,
    specialist: ZhenfaSpecialistLevel,
) -> u64 {
    let specialist_factor = match specialist {
        ZhenfaSpecialistLevel::None => 1.0 / 1.5,
        ZhenfaSpecialistLevel::Novice => 1.0 / 1.25,
        ZhenfaSpecialistLevel::Expert => 1.0,
    };
    let color_factor = if color_matches(qi_color.main, qi_color.secondary, ColorKind::Solid) {
        2.0
    } else {
        1.0
    };
    ((base_ticks as f64) * specialist_factor * color_factor).round() as u64
}

fn active_trigger_range(cultivation: &Cultivation, qi_color: &QiColor) -> f64 {
    let base =
        crate::cultivation::spiritual_sense::scanner::scan_radius_for_realm(cultivation.realm);
    let base = if base <= 0.0 { 16.0 } else { base };
    if color_matches(qi_color.main, qi_color.secondary, ColorKind::Intricate) {
        base * 1.5
    } else {
        base
    }
}

fn color_matches(main: ColorKind, secondary: Option<ColorKind>, target: ColorKind) -> bool {
    main == target || secondary == Some(target)
}

fn trap_contam_delta(main: ColorKind, secondary: Option<ColorKind>) -> f64 {
    if color_matches(main, secondary, ColorKind::Turbid) {
        0.15
    } else {
        0.0
    }
}

fn has_zhenfa_flag(inventory: Option<&PlayerInventory>) -> bool {
    let Some(inventory) = inventory else {
        return false;
    };
    inventory
        .equipped
        .values()
        .chain(inventory.hotbar.iter().flatten())
        .any(|item| item.template_id == ZHENFA_FLAG_ITEM_ID)
}

fn backlash_contam_delta(kind: ZhenfaKind) -> f64 {
    match kind {
        ZhenfaKind::Trap => 0.5,
        ZhenfaKind::Ward => 0.3,
    }
}

fn apply_backlash(
    player: Entity,
    wounds: &mut Wounds,
    contamination: Option<bevy_ecs::change_detection::Mut<'_, Contamination>>,
    meridians: Option<bevy_ecs::change_detection::Mut<'_, MeridianSystem>>,
    tick: u64,
    contam_delta: f64,
) {
    wounds.entries.push(Wound {
        location: BodyPart::ArmR,
        kind: WoundKind::Concussion,
        severity: 0.25,
        bleeding_per_sec: 0.0,
        created_at_tick: tick,
        inflicted_by: Some("zhenfa_backlash".to_string()),
    });
    wounds.health_current = (wounds.health_current - 6.0).clamp(0.0, wounds.health_max);

    if let Some(mut contamination) = contamination {
        contamination.entries.push(ContamSource {
            amount: contam_delta,
            color: ColorKind::Turbid,
            attacker_id: Some(format!("zhenfa_backlash:{:?}", player)),
            introduced_at: tick,
        });
    }
    if let Some(mut meridians) = meridians {
        let meridian = meridians.get_mut(MeridianId::Lung);
        meridian.integrity = (meridian.integrity - 0.05).max(0.0);
    }
}

fn deterministic_roll(player: Entity, instance_id: u64, pos: [i32; 3]) -> f64 {
    let mut x = player.to_bits() ^ instance_id.rotate_left(13);
    x ^= (pos[0] as i64 as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    x ^= (pos[1] as i64 as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= (pos[2] as i64 as u64).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^= x >> 30;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^= x >> 31;
    (x as f64) / (u64::MAX as f64)
}

fn in_horizontal_radius(position: valence::math::DVec3, center: [i32; 3], radius: u8) -> bool {
    let dx = position.x - (f64::from(center[0]) + 0.5);
    let dz = position.z - (f64::from(center[2]) + 0.5);
    let horizontal = (dx * dx + dz * dz).sqrt();
    horizontal <= f64::from(radius) + 0.75 && (position.y - f64::from(center[1])).abs() <= 3.0
}

fn distance_to_block(position: valence::math::DVec3, center: [i32; 3]) -> f64 {
    let dx = position.x - (f64::from(center[0]) + 0.5);
    let dy = position.y - f64::from(center[1]);
    let dz = position.z - (f64::from(center[2]) + 0.5);
    (dx * dx + dy * dy + dz * dz).sqrt()
}

fn ordered_distance_to_block(position: valence::math::DVec3, center: [i32; 3]) -> u64 {
    (distance_to_block(position, center) * 1000.0).round() as u64
}

fn chebyshev_distance(left: [i32; 3], right: [i32; 3]) -> i32 {
    (left[0] - right[0])
        .abs()
        .max((left[1] - right[1]).abs())
        .max((left[2] - right[2]).abs())
}

fn squared_distance_i32(left: [i32; 3], right: [i32; 3]) -> i32 {
    let dx = left[0] - right[0];
    let dy = left[1] - right[1];
    let dz = left[2] - right[2];
    dx * dx + dy * dy + dz * dz
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::events::{CombatEvent, DeathEvent};
    use crate::cultivation::components::{QiColor, Realm};
    use crate::inventory::{
        ContainerState, InventoryRevision, ItemCategory, ItemInstance, ItemRarity, ItemTemplate,
        PlayerInventory, EQUIP_SLOT_MAIN_HAND,
    };
    use valence::prelude::{App, DVec3, Events};

    fn app_with_zhenfa() -> App {
        let mut app = App::new();
        app.insert_resource(CombatClock::default());
        app.insert_resource(PendingGameplayNarrations::default());
        app.add_event::<ZhenfaPlaceRequest>();
        app.add_event::<ZhenfaTriggerRequest>();
        app.add_event::<ZhenfaDisarmRequest>();
        app.add_event::<ZhenfaSensePulse>();
        app.add_event::<CombatEvent>();
        app.add_event::<DeathEvent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.insert_resource(ZhenfaRegistry::default());
        app.add_systems(
            Update,
            (
                handle_zhenfa_place_requests,
                handle_zhenfa_trigger_requests,
                handle_zhenfa_disarm_requests,
                tick_zhenfa_registry,
            )
                .chain(),
        );
        app
    }

    fn spawn_player(app: &mut App, name: &str, pos: [f64; 3]) -> Entity {
        app.world_mut()
            .spawn((
                Username(name.to_string()),
                Position::new(pos),
                Cultivation {
                    realm: Realm::Induce,
                    qi_current: 100.0,
                    qi_max: 100.0,
                    ..Default::default()
                },
                QiColor::default(),
                PracticeLog::default(),
                Wounds::default(),
                Contamination::default(),
                MeridianSystem::default(),
                zhenfa_flag_inventory(),
            ))
            .id()
    }

    fn array_flag_item(instance_id: u64) -> ItemInstance {
        ItemInstance {
            instance_id,
            template_id: ZHENFA_FLAG_ITEM_ID.to_string(),
            display_name: "阵旗".to_string(),
            grid_w: 1,
            grid_h: 2,
            weight: 0.6,
            rarity: ItemRarity::Uncommon,
            description: "地师用来牵引阵眼气机的短旗。".to_string(),
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
        }
    }

    fn zhenfa_flag_inventory() -> PlayerInventory {
        let mut inventory = empty_inventory();
        inventory
            .equipped
            .insert(EQUIP_SLOT_MAIN_HAND.to_string(), array_flag_item(9001));
        inventory
    }

    fn pearl_registry() -> ItemRegistry {
        let template = ItemTemplate {
            id: ZHENFA_PEARL_ITEM_ID.to_string(),
            display_name: "散逸真元珠".to_string(),
            category: ItemCategory::Misc,
            max_stack_count: 1,
            grid_w: 1,
            grid_h: 1,
            base_weight: 0.05,
            rarity: ItemRarity::Uncommon,
            spirit_quality_initial: 0.6,
            description: "破阵后凝住的一小粒散逸真元。".to_string(),
            effect: None,
            cast_duration_ms: 1500,
            cooldown_ms: 1500,
            weapon_spec: None,
            forge_station_spec: None,
            blueprint_scroll_spec: None,
            inscription_scroll_spec: None,
        };
        ItemRegistry::from_map(HashMap::from([(
            ZHENFA_PEARL_ITEM_ID.to_string(),
            template,
        )]))
    }

    fn empty_inventory() -> PlayerInventory {
        PlayerInventory {
            revision: InventoryRevision(0),
            containers: vec![ContainerState {
                id: crate::inventory::MAIN_PACK_CONTAINER_ID.to_string(),
                name: "main".to_string(),
                rows: 4,
                cols: 6,
                items: Vec::new(),
            }],
            equipped: HashMap::new(),
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 45.0,
        }
    }

    #[test]
    fn placement_clamps_to_carrier_cap_and_debits_qi() {
        let mut app = app_with_zhenfa();
        let owner = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);

        app.world_mut().send_event(ZhenfaPlaceRequest {
            player: owner,
            pos: [1, 64, 1],
            kind: ZhenfaKind::Trap,
            carrier: ZhenfaCarrierKind::CommonStone,
            qi_invest_ratio: 0.80,
            trigger: Some("proximity".to_string()),
            requested_at_tick: 10,
        });
        app.update();

        let cultivation = app.world().get::<Cultivation>(owner).unwrap();
        assert_eq!(cultivation.qi_current, 90.0);
        let registry = app.world().resource::<ZhenfaRegistry>();
        let instance = registry.find_at([1, 64, 1]).unwrap();
        assert_eq!(instance.qi_invest_ratio, 0.10);
        assert_eq!(instance.effect_radius, 0);
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn duplicate_same_block_is_rejected_without_second_qi_debit() {
        let mut app = app_with_zhenfa();
        let owner = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);

        for tick in [1, 2] {
            app.world_mut().send_event(ZhenfaPlaceRequest {
                player: owner,
                pos: [2, 64, 2],
                kind: ZhenfaKind::Trap,
                carrier: ZhenfaCarrierKind::LingqiBlock,
                qi_invest_ratio: 0.10,
                trigger: None,
                requested_at_tick: tick,
            });
        }
        app.update();

        assert_eq!(app.world().resource::<ZhenfaRegistry>().len(), 1);
        assert_eq!(
            app.world().get::<Cultivation>(owner).unwrap().qi_current,
            90.0
        );
    }

    #[test]
    fn placement_requires_array_flag() {
        let mut app = app_with_zhenfa();
        let owner = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);
        app.world_mut().entity_mut(owner).insert(empty_inventory());

        app.world_mut().send_event(ZhenfaPlaceRequest {
            player: owner,
            pos: [2, 64, 2],
            kind: ZhenfaKind::Trap,
            carrier: ZhenfaCarrierKind::LingqiBlock,
            qi_invest_ratio: 0.10,
            trigger: None,
            requested_at_tick: 1,
        });
        app.update();

        assert_eq!(app.world().resource::<ZhenfaRegistry>().len(), 0);
        assert_eq!(
            app.world().get::<Cultivation>(owner).unwrap().qi_current,
            100.0
        );
    }

    #[test]
    fn decay_removes_expired_array_eye() {
        let mut app = app_with_zhenfa();
        let owner = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);
        app.world_mut().send_event(ZhenfaPlaceRequest {
            player: owner,
            pos: [3, 64, 3],
            kind: ZhenfaKind::Trap,
            carrier: ZhenfaCarrierKind::CommonStone,
            qi_invest_ratio: 0.10,
            trigger: None,
            requested_at_tick: 0,
        });
        app.update();

        let anchor_entity = app
            .world()
            .resource::<ZhenfaRegistry>()
            .find_at([3, 64, 3])
            .unwrap()
            .anchor_entity;
        app.world_mut().resource_mut::<CombatClock>().tick =
            carrier_spec(ZhenfaCarrierKind::CommonStone).duration_ticks + 1;
        app.update();

        assert!(app
            .world()
            .resource::<ZhenfaRegistry>()
            .find_at([3, 64, 3])
            .is_none());
        assert!(app.world().get_entity(anchor_entity).is_none());
    }

    #[test]
    fn passive_trap_trigger_damages_legs_and_frees_array_eye() {
        let mut app = app_with_zhenfa();
        let owner = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);
        let intruder = spawn_player(&mut app, "Bob", [5.5, 64.0, 5.5]);
        app.world_mut().send_event(ZhenfaPlaceRequest {
            player: owner,
            pos: [5, 64, 5],
            kind: ZhenfaKind::Trap,
            carrier: ZhenfaCarrierKind::LingqiBlock,
            qi_invest_ratio: 0.20,
            trigger: None,
            requested_at_tick: 10,
        });
        app.update();

        let id = app
            .world()
            .resource::<ZhenfaRegistry>()
            .find_at([5, 64, 5])
            .unwrap()
            .id;
        let anchor_entity = app
            .world()
            .resource::<ZhenfaRegistry>()
            .find_at([5, 64, 5])
            .unwrap()
            .anchor_entity;
        app.world_mut().resource_mut::<CombatClock>().tick = 11;
        app.update();

        let registry = app.world().resource::<ZhenfaRegistry>();
        assert!(registry.get(id).is_none());
        assert!(registry.find_at([5, 64, 5]).is_none());
        assert!(app.world().get_entity(anchor_entity).is_none());
        let wounds = app.world().get::<Wounds>(intruder).unwrap();
        assert_eq!(
            wounds
                .entries
                .iter()
                .filter(|w| w.location == BodyPart::LegL || w.location == BodyPart::LegR)
                .count(),
            2
        );
        assert!(wounds.health_current < wounds.health_max);
        assert!(!app.world().resource::<Events<CombatEvent>>().is_empty());
        assert_eq!(
            app.world()
                .get::<PracticeLog>(owner)
                .unwrap()
                .weights
                .get(&ColorKind::Intricate)
                .copied(),
            Some(crate::cultivation::color::STYLE_PRACTICE_AMOUNT)
        );
    }

    #[test]
    fn chain_trigger_waits_six_ticks_and_does_not_loop() {
        let mut app = app_with_zhenfa();
        let owner = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);
        let _intruder = spawn_player(&mut app, "Bob", [5.5, 64.0, 5.5]);
        for (idx, pos) in [[5, 64, 5], [6, 64, 5], [7, 64, 5]].into_iter().enumerate() {
            app.world_mut().send_event(ZhenfaPlaceRequest {
                player: owner,
                pos,
                kind: ZhenfaKind::Trap,
                carrier: ZhenfaCarrierKind::LingqiBlock,
                qi_invest_ratio: 0.10,
                trigger: None,
                requested_at_tick: idx as u64,
            });
        }
        app.update();

        app.world_mut().resource_mut::<CombatClock>().tick = 10;
        app.update();
        assert_eq!(app.world().resource::<ZhenfaRegistry>().len(), 2);

        app.world_mut().resource_mut::<CombatClock>().tick = 16;
        app.update();
        assert_eq!(app.world().resource::<ZhenfaRegistry>().len(), 1);

        app.world_mut().resource_mut::<CombatClock>().tick = 22;
        app.update();
        assert_eq!(app.world().resource::<ZhenfaRegistry>().len(), 0);
    }

    #[test]
    fn active_trigger_picks_nearest_owned_untriggered_trap() {
        let mut app = app_with_zhenfa();
        let owner = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);
        for (tick, pos) in [(1, [10, 64, 0]), (2, [3, 64, 0])] {
            app.world_mut().send_event(ZhenfaPlaceRequest {
                player: owner,
                pos,
                kind: ZhenfaKind::Trap,
                carrier: ZhenfaCarrierKind::LingqiBlock,
                qi_invest_ratio: 0.10,
                trigger: None,
                requested_at_tick: tick,
            });
        }
        app.update();
        app.world_mut().send_event(ZhenfaTriggerRequest {
            player: owner,
            instance_id: None,
            requested_at_tick: 20,
        });
        app.update();

        let registry = app.world().resource::<ZhenfaRegistry>();
        assert!(registry.find_at([3, 64, 0]).is_none());
        assert!(registry.find_at([10, 64, 0]).is_some());
    }

    #[test]
    fn ward_alert_fires_on_entry_for_position_only_entities() {
        let mut app = app_with_zhenfa();
        let owner = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);
        let intruder = app.world_mut().spawn(Position::new([4.5, 64.0, 0.5])).id();
        app.world_mut().send_event(ZhenfaPlaceRequest {
            player: owner,
            pos: [0, 64, 0],
            kind: ZhenfaKind::Ward,
            carrier: ZhenfaCarrierKind::LingqiBlock,
            qi_invest_ratio: 0.20,
            trigger: None,
            requested_at_tick: 1,
        });
        app.update();
        app.world_mut().resource_mut::<CombatClock>().tick = 2;
        app.update();
        app.world_mut().resource_mut::<CombatClock>().tick = 3;
        app.update();

        let narrations = app
            .world_mut()
            .resource_mut::<PendingGameplayNarrations>()
            .drain();
        assert_eq!(narrations.len(), 1);
        assert_eq!(narrations[0].target.as_deref(), Some("offline:Alice"));

        app.world_mut().resource_mut::<CombatClock>().tick =
            WARD_ALERT_THROTTLE_TICKS.saturating_add(5);
        app.update();
        assert!(app
            .world_mut()
            .resource_mut::<PendingGameplayNarrations>()
            .drain()
            .is_empty());

        app.world_mut()
            .entity_mut(intruder)
            .insert(Position::new([30.0, 64.0, 0.0]));
        app.world_mut().resource_mut::<CombatClock>().tick =
            WARD_ALERT_THROTTLE_TICKS.saturating_add(6);
        app.update();
        assert!(app
            .world_mut()
            .resource_mut::<PendingGameplayNarrations>()
            .drain()
            .is_empty());

        app.world_mut()
            .entity_mut(intruder)
            .insert(Position::new([4.5, 64.0, 0.5]));
        app.world_mut().resource_mut::<CombatClock>().tick =
            WARD_ALERT_THROTTLE_TICKS.saturating_add(7);
        app.update();
        assert_eq!(
            app.world_mut()
                .resource_mut::<PendingGameplayNarrations>()
                .drain()
                .len(),
            1
        );
    }

    #[test]
    fn force_break_applies_backlash_and_removes_eye() {
        let mut app = app_with_zhenfa();
        let owner = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);
        let breaker = spawn_player(&mut app, "Bob", [1.5, 64.0, 1.5]);
        app.world_mut().send_event(ZhenfaPlaceRequest {
            player: owner,
            pos: [1, 64, 1],
            kind: ZhenfaKind::Trap,
            carrier: ZhenfaCarrierKind::LingqiBlock,
            qi_invest_ratio: 0.10,
            trigger: None,
            requested_at_tick: 1,
        });
        app.update();

        let anchor_entity = app
            .world()
            .resource::<ZhenfaRegistry>()
            .find_at([1, 64, 1])
            .unwrap()
            .anchor_entity;
        app.world_mut().send_event(ZhenfaDisarmRequest {
            player: breaker,
            pos: [1, 64, 1],
            mode: ZhenfaDisarmMode::ForceBreak,
            requested_at_tick: 2,
        });
        app.update();

        assert!(app
            .world()
            .resource::<ZhenfaRegistry>()
            .find_at([1, 64, 1])
            .is_none());
        assert!(app.world().get_entity(anchor_entity).is_none());
        assert!(!app
            .world()
            .get::<Wounds>(breaker)
            .unwrap()
            .entries
            .is_empty());
        assert_eq!(
            app.world()
                .get::<Contamination>(breaker)
                .unwrap()
                .entries
                .first()
                .unwrap()
                .amount,
            0.5
        );
    }

    #[test]
    fn expert_disarm_grants_scattered_qi_pearl() {
        let mut app = app_with_zhenfa();
        app.insert_resource(pearl_registry());
        app.insert_resource(InventoryInstanceIdAllocator::default());
        let owner = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);
        let breaker = spawn_player(&mut app, "Bob", [1.5, 64.0, 1.5]);
        app.world_mut().entity_mut(breaker).insert((
            InsightModifiers {
                zhenfa_disenchant: 5.0,
                ..InsightModifiers::new()
            },
            empty_inventory(),
        ));
        app.world_mut().send_event(ZhenfaPlaceRequest {
            player: owner,
            pos: [1, 64, 1],
            kind: ZhenfaKind::Trap,
            carrier: ZhenfaCarrierKind::LingqiBlock,
            qi_invest_ratio: 0.10,
            trigger: None,
            requested_at_tick: 1,
        });
        app.update();

        let anchor_entity = app
            .world()
            .resource::<ZhenfaRegistry>()
            .find_at([1, 64, 1])
            .unwrap()
            .anchor_entity;
        app.world_mut().send_event(ZhenfaDisarmRequest {
            player: breaker,
            pos: [1, 64, 1],
            mode: ZhenfaDisarmMode::Disarm,
            requested_at_tick: 2,
        });
        app.update();

        let inventory = app.world().get::<PlayerInventory>(breaker).unwrap();
        let main_pack = inventory
            .containers
            .iter()
            .find(|container| container.id == crate::inventory::MAIN_PACK_CONTAINER_ID)
            .unwrap();
        assert_eq!(main_pack.items.len(), 1);
        assert_eq!(
            main_pack.items[0].instance.template_id,
            ZHENFA_PEARL_ITEM_ID
        );
        assert!(app.world().get_entity(anchor_entity).is_none());
    }

    #[test]
    fn helper_ranges_follow_plan_thresholds() {
        assert_eq!(trap_effect_radius(0.10), 0);
        assert_eq!(trap_effect_radius(0.20), 1);
        assert_eq!(trap_effect_radius(0.50), 2);
        assert_eq!(ward_radius(0.20, ZhenfaSpecialistLevel::None), 4);
        assert_eq!(ward_radius(0.20, ZhenfaSpecialistLevel::Expert), 8);
        assert!(in_horizontal_radius(
            DVec3::new(1.5, 64.0, 1.5),
            [1, 64, 1],
            0
        ));
    }

    #[test]
    fn zhenfa_instance_exposes_style_attack_and_defense() {
        let instance = ZhenfaInstance {
            id: 1,
            kind: ZhenfaKind::Ward,
            owner: Entity::from_raw(1),
            owner_player_id: "offline:Azure".to_string(),
            pos: [1, 64, 1],
            carrier: ZhenfaCarrierKind::LingqiBlock,
            qi_invest_ratio: 0.5,
            qi_invest_amount: 25.0,
            effect_radius: 2,
            ward_radius: 8,
            placed_at_tick: 1,
            expires_at_tick: 100,
            triggered_at: None,
            trigger: None,
            color_main: ColorKind::Intricate,
            color_secondary: Some(ColorKind::Solid),
            anchor_entity: Entity::from_raw(2),
        };

        assert_eq!(instance.style_color(), ColorKind::Intricate);
        assert_eq!(instance.injected_qi(), 25.0);
        assert_eq!(instance.medium().carrier, CarrierGrade::SpiritWeapon);
        assert_eq!(instance.defense_color(), ColorKind::Solid);
        assert_eq!(instance.resistance(), 0.5);
    }
}
