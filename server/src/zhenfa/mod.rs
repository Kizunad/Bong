use std::collections::{HashMap, HashSet, VecDeque};

use serde::{Deserialize, Serialize};
use valence::prelude::{
    bevy_ecs, App, BlockPos, BlockState, ChunkLayer, Client, Commands, Component, Entity, Event,
    EventReader, EventWriter, Events, IntoSystemConfigs, Position, PropName, PropValue, Query, Res,
    ResMut, Resource, SystemSet, Update, Username, With, Without,
};

use crate::combat::components::{BodyPart, Lifecycle, LifecycleState, Wound, WoundKind, Wounds};
use crate::combat::events::{ApplyStatusEffectIntent, CombatEvent, DeathEvent, StatusEffectKind};
use crate::combat::CombatClock;
use crate::cultivation::color::{record_style_practice, PracticeLog};
use crate::cultivation::components::{
    ColorKind, ContamSource, Contamination, Cultivation, MeridianId, MeridianSystem, QiColor, Realm,
};
use crate::cultivation::insight_apply::InsightModifiers;
use crate::cultivation::meridian::severed::{
    check_meridian_dependencies, MeridianSeveredPermanent,
};
use crate::cultivation::tribulation::{JueBiTriggerEvent, JueBiTriggerSource};
use crate::inventory::{
    add_item_to_player_inventory, InventoryInstanceIdAllocator, ItemRegistry, PlayerInventory,
};
use crate::network::{gameplay_vfx, vfx_event_emit::VfxEventRequest};
use crate::player::gameplay::PendingGameplayNarrations;
use crate::player::state::canonical_player_id;
use crate::qi_physics::{CarrierGrade, MediumKind, StyleAttack, StyleDefense};
use crate::schema::common::NarrationStyle;
use crate::schema::realm_vision::{SenseEntryV1, SenseKindV1, SpiritualSenseTargetsV1};
use crate::schema::social::RelationshipKindV1;
use crate::social::components::{Relationships, Renown};
use crate::world::{
    bong_blocks::{place_bong_block, remove_bong_block},
    dimension::OverworldLayer,
};

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
    ShrineWard,
    Lingju,
    DeceiveHeaven,
    Illusion,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemSet)]
pub enum ZhenfaSystemSet {
    Runtime,
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

#[derive(Debug, Clone, Event, PartialEq)]
pub struct WardArrayDeployEvent {
    pub owner: Entity,
    pub owner_player_id: String,
    pub array_id: u64,
    pub pos: [i32; 3],
    pub radius: u8,
    pub reflect_ratio: f64,
    pub placed_at_tick: u64,
}

#[derive(Debug, Clone, Event, PartialEq)]
pub struct LingArrayDeployEvent {
    pub owner: Entity,
    pub owner_player_id: String,
    pub array_id: u64,
    pub pos: [i32; 3],
    pub radius: u8,
    pub density_multiplier: f64,
    pub tiandao_gaze_weight: f64,
    pub placed_at_tick: u64,
}

#[derive(Debug, Clone, Event, PartialEq)]
pub struct DeceiveHeavenEvent {
    pub owner: Entity,
    pub owner_player_id: String,
    pub array_id: u64,
    pub pos: [i32; 3],
    pub self_weight_multiplier: f64,
    pub target_weight_multiplier: f64,
    pub reveal_chance_per_tick: f64,
    pub placed_at_tick: u64,
}

#[derive(Debug, Clone, Event, PartialEq)]
pub struct DeceiveHeavenExposedEvent {
    pub owner: Entity,
    pub owner_player_id: String,
    pub array_id: u64,
    pub pos: [i32; 3],
    pub self_weight_multiplier: f64,
    pub target_weight_multiplier: f64,
    pub reveal_chance_per_tick: f64,
    pub exposed_at_tick: u64,
}

#[derive(Debug, Clone, Event, PartialEq)]
pub struct IllusionArrayDeployEvent {
    pub owner: Entity,
    pub owner_player_id: String,
    pub array_id: u64,
    pub pos: [i32; 3],
    pub reveal_threshold: f64,
    pub placed_at_tick: u64,
}

#[derive(Debug, Clone, Event, PartialEq)]
pub struct ArrayDecayEvent {
    pub owner: Entity,
    pub owner_player_id: String,
    pub array_id: u64,
    pub kind: ZhenfaKind,
    pub pos: [i32; 3],
    pub decayed_at_tick: u64,
}

#[derive(Debug, Clone, Event, PartialEq)]
pub struct ArrayBreakthroughEvent {
    pub breaker: Entity,
    pub breaker_player_id: String,
    pub owner: Entity,
    pub owner_player_id: String,
    pub array_id: u64,
    pub kind: ZhenfaKind,
    pub pos: [i32; 3],
    pub force_break: bool,
    pub broken_at_tick: u64,
}

#[derive(Debug, Clone, Copy, Component, PartialEq, Eq)]
pub struct ZhenfaAnchor {
    pub id: u64,
}

#[derive(Debug, Clone, Component, PartialEq)]
pub struct ArrayImprint {
    pub kind: ZhenfaKind,
    pub dimension_target: Option<String>,
    pub tribulation_broadcast: bool,
}

#[derive(Debug, Clone, Component, PartialEq)]
pub struct ArrayMastery {
    pub trap: f64,
    pub ward: f64,
    pub shrine_ward: f64,
    pub lingju: f64,
    pub deceive_heaven: f64,
    pub illusion: f64,
}

impl Default for ArrayMastery {
    fn default() -> Self {
        Self {
            trap: 0.0,
            ward: 0.0,
            shrine_ward: 0.0,
            lingju: 0.0,
            deceive_heaven: 0.0,
            illusion: 0.0,
        }
    }
}

impl ArrayMastery {
    pub fn value(&self, kind: ZhenfaKind) -> f64 {
        match kind {
            ZhenfaKind::Trap => self.trap,
            ZhenfaKind::Ward => self.ward,
            ZhenfaKind::ShrineWard => self.shrine_ward,
            ZhenfaKind::Lingju => self.lingju,
            ZhenfaKind::DeceiveHeaven => self.deceive_heaven,
            ZhenfaKind::Illusion => self.illusion,
        }
    }

    pub fn add_cast(&mut self, kind: ZhenfaKind) {
        self.add(kind, 0.3);
    }

    pub fn add_trigger(&mut self, kind: ZhenfaKind) {
        self.add(kind, 1.0);
    }

    fn add(&mut self, kind: ZhenfaKind, amount: f64) {
        let slot = match kind {
            ZhenfaKind::Trap => &mut self.trap,
            ZhenfaKind::Ward => &mut self.ward,
            ZhenfaKind::ShrineWard => &mut self.shrine_ward,
            ZhenfaKind::Lingju => &mut self.lingju,
            ZhenfaKind::DeceiveHeaven => &mut self.deceive_heaven,
            ZhenfaKind::Illusion => &mut self.illusion,
        };
        *slot = (*slot + amount).clamp(0.0, 100.0);
    }
}

pub const ZHENFA_VISUAL_STATE_INACTIVE: u8 = 0;
pub const ZHENFA_VISUAL_STATE_ACTIVE: u8 = 1;
pub const ZHENFA_VISUAL_STATE_EXHAUSTED: u8 = 2;

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
    pub realm_at_cast: Realm,
    pub mastery_at_cast: f64,
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

    fn rejection_rate(&self) -> f64 {
        0.35
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

impl ZhenfaInstance {
    fn reflect_ratio(&self) -> f32 {
        if self.kind == ZhenfaKind::ShrineWard && self.realm_at_cast == Realm::Void {
            0.80
        } else if self.kind == ZhenfaKind::ShrineWard {
            0.50
        } else {
            0.0
        }
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
    app.add_event::<WardArrayDeployEvent>();
    app.add_event::<LingArrayDeployEvent>();
    app.add_event::<DeceiveHeavenEvent>();
    app.add_event::<DeceiveHeavenExposedEvent>();
    app.add_event::<IllusionArrayDeployEvent>();
    app.add_event::<ArrayDecayEvent>();
    app.add_event::<ArrayBreakthroughEvent>();
    app.add_systems(
        Update,
        (
            handle_zhenfa_place_requests,
            handle_zhenfa_trigger_requests,
            handle_zhenfa_disarm_requests,
            tick_zhenfa_registry,
            emit_zhenfa_sense_pulses,
        )
            .chain()
            .in_set(ZhenfaSystemSet::Runtime),
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

    pub fn anchor_visual_state(&self, anchor: &ZhenfaAnchor) -> u8 {
        if anchor.id == 0 {
            return ZHENFA_VISUAL_STATE_INACTIVE;
        }
        let Some(instance) = self.instances.get(&anchor.id) else {
            return ZHENFA_VISUAL_STATE_EXHAUSTED;
        };
        if instance.triggered_at.is_some() || self.pending_chain.iter().any(|p| p.id == anchor.id) {
            ZHENFA_VISUAL_STATE_EXHAUSTED
        } else {
            ZHENFA_VISUAL_STATE_ACTIVE
        }
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

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ZhenfaKindProfile {
    pub min_invest_ratio: f64,
    pub cap_invest_ratio: f64,
    pub cast_time_ticks: u64,
    pub duration_ticks: u64,
    pub radius: u8,
    pub density_multiplier: f64,
    pub tiandao_gaze_weight: f64,
    pub reveal_threshold: f64,
    pub reveal_chance_per_tick: f64,
    pub reflect_ratio: f64,
}

pub fn zhenfa_kind_profile(
    kind: ZhenfaKind,
    realm: Realm,
    mastery: f64,
    carrier: ZhenfaCarrierKind,
) -> ZhenfaKindProfile {
    let mastery_ratio = mastery_ratio(mastery);
    let cap = carrier_spec(carrier).cap_ratio;
    match kind {
        ZhenfaKind::Trap => ZhenfaKindProfile {
            min_invest_ratio: MIN_QI_INVEST_RATIO,
            cap_invest_ratio: cap,
            cast_time_ticks: cast_time_between(3, 1, mastery_ratio),
            duration_ticks: carrier_spec(carrier).duration_ticks,
            radius: 0,
            density_multiplier: 1.0,
            tiandao_gaze_weight: 0.0,
            reveal_threshold: 30.0,
            reveal_chance_per_tick: 0.0,
            reflect_ratio: 0.0,
        },
        ZhenfaKind::Ward => ZhenfaKindProfile {
            min_invest_ratio: MIN_QI_INVEST_RATIO,
            cap_invest_ratio: cap,
            cast_time_ticks: cast_time_between(5, 2, mastery_ratio),
            duration_ticks: carrier_spec(carrier).duration_ticks,
            radius: 8,
            density_multiplier: 1.0,
            tiandao_gaze_weight: 0.0,
            reveal_threshold: 30.0,
            reveal_chance_per_tick: 0.0,
            reflect_ratio: 0.0,
        },
        ZhenfaKind::ShrineWard => {
            let void_bonus = if realm == Realm::Void { 10 } else { 0 };
            ZhenfaKindProfile {
                min_invest_ratio: 0.05,
                cap_invest_ratio: cap.max(0.50),
                cast_time_ticks: cast_time_between(8, 3, mastery_ratio),
                duration_ticks: duration_with_mastery(
                    12 * 60 * 60 * TICKS_PER_SECOND,
                    mastery_ratio,
                ),
                radius: 5 + void_bonus,
                density_multiplier: 1.0,
                tiandao_gaze_weight: 0.0,
                reveal_threshold: 30.0,
                reveal_chance_per_tick: 0.0,
                reflect_ratio: if realm == Realm::Void { 0.80 } else { 0.50 },
            }
        }
        ZhenfaKind::Lingju => {
            let void_bonus = if realm == Realm::Void { 2.0 } else { 0.0 };
            ZhenfaKindProfile {
                min_invest_ratio: 0.30,
                cap_invest_ratio: cap.max(0.50),
                cast_time_ticks: cast_time_between(30, 12, mastery_ratio),
                duration_ticks: duration_with_mastery(
                    6 * 60 * 60 * TICKS_PER_SECOND,
                    mastery_ratio,
                ),
                radius: if realm == Realm::Void { 60 } else { 20 },
                density_multiplier: 1.5 + void_bonus,
                tiandao_gaze_weight: if realm == Realm::Void { 5.0 } else { 1.0 },
                reveal_threshold: 30.0,
                reveal_chance_per_tick: 0.0,
                reflect_ratio: 0.0,
            }
        }
        ZhenfaKind::DeceiveHeaven => ZhenfaKindProfile {
            min_invest_ratio: 0.80,
            cap_invest_ratio: 1.0,
            cast_time_ticks: cast_time_between(300, 120, mastery_ratio),
            duration_ticks: 60 * TICKS_PER_SECOND,
            radius: if realm == Realm::Void { 24 } else { 16 },
            density_multiplier: 0.25,
            tiandao_gaze_weight: 1.5,
            reveal_threshold: 50.0,
            reveal_chance_per_tick: deceive_heaven_reveal_chance(realm),
            reflect_ratio: 0.0,
        },
        ZhenfaKind::Illusion => ZhenfaKindProfile {
            min_invest_ratio: 0.10,
            cap_invest_ratio: cap.max(0.20),
            cast_time_ticks: cast_time_between(5, 2, mastery_ratio),
            duration_ticks: duration_with_mastery(
                carrier_spec(carrier).duration_ticks,
                mastery_ratio,
            ),
            radius: 8,
            density_multiplier: 1.0,
            tiandao_gaze_weight: 0.0,
            reveal_threshold: if realm == Realm::Void { 50.0 } else { 30.0 },
            reveal_chance_per_tick: 0.0,
            reflect_ratio: 0.0,
        },
    }
}

pub fn zhenfa_meridian_dependencies(kind: ZhenfaKind) -> &'static [MeridianId] {
    match kind {
        ZhenfaKind::Trap | ZhenfaKind::Ward => &[MeridianId::Ren],
        ZhenfaKind::ShrineWard => &[MeridianId::Ren, MeridianId::Du],
        ZhenfaKind::Lingju => &[MeridianId::Ren, MeridianId::Du, MeridianId::Kidney],
        ZhenfaKind::DeceiveHeaven => &[
            MeridianId::Ren,
            MeridianId::Du,
            MeridianId::Kidney,
            MeridianId::Heart,
        ],
        ZhenfaKind::Illusion => &[MeridianId::Kidney],
    }
}

pub fn realm_allows_zhenfa_kind(kind: ZhenfaKind, realm: Realm) -> bool {
    kind != ZhenfaKind::DeceiveHeaven
        || matches!(realm, Realm::Solidify | Realm::Spirit | Realm::Void)
}

fn mastery_ratio(mastery: f64) -> f64 {
    if mastery.is_finite() {
        (mastery / 100.0).clamp(0.0, 1.0)
    } else {
        0.0
    }
}

fn cast_time_between(max_seconds: u64, min_seconds: u64, mastery_ratio: f64) -> u64 {
    let max_ticks = max_seconds * TICKS_PER_SECOND;
    let min_ticks = min_seconds * TICKS_PER_SECOND;
    ((max_ticks as f64) - ((max_ticks - min_ticks) as f64 * mastery_ratio)).round() as u64
}

fn duration_with_mastery(base_ticks: u64, mastery_ratio: f64) -> u64 {
    ((base_ticks as f64) * (1.0 + 2.0 * mastery_ratio)).round() as u64
}

#[allow(clippy::too_many_arguments)]
fn handle_zhenfa_place_requests(
    mut requests: EventReader<ZhenfaPlaceRequest>,
    mut registry: ResMut<ZhenfaRegistry>,
    mut commands: Commands,
    mut players: Query<ZhenfaPlacePlayer<'_>>,
    mut layers: Query<&mut ChunkLayer, With<OverworldLayer>>,
    mut ward_events: EventWriter<WardArrayDeployEvent>,
    mut ling_events: EventWriter<LingArrayDeployEvent>,
    mut deceive_events: EventWriter<DeceiveHeavenEvent>,
    mut illusion_events: EventWriter<IllusionArrayDeployEvent>,
) {
    for req in requests.read() {
        if registry.find_at(req.pos).is_some() {
            tracing::warn!(
                "[bong][zhenfa] place rejected: pos={:?} already has an array eye",
                req.pos
            );
            continue;
        }

        let Ok((username, mut cultivation, qi_color, modifiers, inventory, severed, mastery)) =
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
        if !realm_allows_zhenfa_kind(req.kind, cultivation.realm) {
            tracing::warn!(
                "[bong][zhenfa] place rejected: {:?} requires Solidify+ realm, got {:?}",
                req.kind,
                cultivation.realm
            );
            continue;
        }
        if let Err(blocked) =
            check_meridian_dependencies(zhenfa_meridian_dependencies(req.kind), severed)
        {
            tracing::warn!(
                "[bong][zhenfa] place rejected: {:?} blocked by severed meridian {:?}",
                req.kind,
                blocked
            );
            continue;
        }

        let mastery_at_cast = mastery
            .as_deref()
            .map(|m| m.value(req.kind))
            .unwrap_or_default();
        let profile =
            zhenfa_kind_profile(req.kind, cultivation.realm, mastery_at_cast, req.carrier);
        let invest_ratio = sanitize_invest_ratio(
            req.qi_invest_ratio,
            profile.min_invest_ratio,
            profile.cap_invest_ratio,
        );
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

        match place_zhenfa_anchor_block(&mut layers, req.pos, zhenfa_anchor_block_state(req.kind)) {
            Ok(true) => {}
            Ok(false) => {
                tracing::warn!(
                    "[bong][zhenfa] place rejected: no overworld layer for custom block at {:?}",
                    req.pos
                );
                continue;
            }
            Err(error) => {
                tracing::warn!(
                    "[bong][zhenfa] place rejected: failed to write custom block at {:?}: {error}",
                    req.pos
                );
                continue;
            }
        }

        let realm_at_cast = cultivation.realm;
        let specialist = zhenfa_specialist_level(modifiers);
        let duration_ticks = effective_duration_ticks(profile.duration_ticks, qi_color, specialist);
        let owner_player_id = canonical_player_id(username.0.as_str());
        let anchor_entity = commands
            .spawn((
                ZhenfaAnchor { id: 0 },
                ArrayImprint {
                    kind: req.kind,
                    dimension_target: None,
                    tribulation_broadcast: req.kind == ZhenfaKind::DeceiveHeaven,
                },
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
            owner_player_id: owner_player_id.clone(),
            pos: req.pos,
            carrier: req.carrier,
            qi_invest_ratio: invest_ratio,
            qi_invest_amount: qi_cost,
            realm_at_cast,
            mastery_at_cast,
            effect_radius: trap_effect_radius(invest_ratio),
            ward_radius: ward_radius(req.kind, invest_ratio, profile.radius, specialist),
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
                cultivation.qi_current = (cultivation.qi_current - qi_cost).max(0.0);
                commands.entity(anchor_entity).insert(ZhenfaAnchor { id });
                if let Some(mut mastery) = mastery {
                    mastery.add_cast(req.kind);
                }
                emit_deploy_event(
                    req.kind,
                    req.player,
                    owner_player_id,
                    id,
                    req.pos,
                    &profile,
                    req.requested_at_tick,
                    &mut ward_events,
                    &mut ling_events,
                    &mut deceive_events,
                    &mut illusion_events,
                );
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
                remove_zhenfa_anchor_block(&mut layers, req.pos);
                commands.entity(anchor_entity).despawn();
                tracing::warn!(
                    "[bong][zhenfa] place failed before registry insert completed: {error}"
                );
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_zhenfa_trigger_requests(
    mut requests: EventReader<ZhenfaTriggerRequest>,
    mut registry: ResMut<ZhenfaRegistry>,
    mut commands: Commands,
    mut players: Query<ZhenfaTriggerPlayer<'_>>,
    mut layers: Query<&mut ChunkLayer, With<OverworldLayer>>,
    mut targets: Query<ZhenfaDamageTarget<'_>>,
    mut practice_logs: Query<&mut PracticeLog>,
    mut combat_events: EventWriter<CombatEvent>,
    mut death_events: EventWriter<DeathEvent>,
    mut status_effects: EventWriter<ApplyStatusEffectIntent>,
    mut sense_pulses: EventWriter<ZhenfaSensePulse>,
    mut vfx_events: Option<ResMut<Events<VfxEventRequest>>>,
) {
    for req in requests.read() {
        let Ok((position, cultivation, qi_color, inventory, mastery)) = players.get_mut(req.player)
        else {
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
        if let Some(mut mastery) = mastery {
            mastery.add_trigger(ZhenfaKind::Trap);
        }
        remove_zhenfa_anchor_blocks(&mut layers, snapshots.iter().map(|snapshot| snapshot.pos));
        despawn_triggered_anchors(&mut commands, &snapshots);
        apply_trigger_snapshots(
            snapshots,
            &mut targets,
            &mut practice_logs,
            &mut combat_events,
            &mut death_events,
            &mut status_effects,
            &mut sense_pulses,
            vfx_events.as_deref_mut(),
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn emit_deploy_event(
    kind: ZhenfaKind,
    owner: Entity,
    owner_player_id: String,
    array_id: u64,
    pos: [i32; 3],
    profile: &ZhenfaKindProfile,
    placed_at_tick: u64,
    ward_events: &mut EventWriter<WardArrayDeployEvent>,
    ling_events: &mut EventWriter<LingArrayDeployEvent>,
    deceive_events: &mut EventWriter<DeceiveHeavenEvent>,
    illusion_events: &mut EventWriter<IllusionArrayDeployEvent>,
) {
    match kind {
        ZhenfaKind::ShrineWard => {
            ward_events.send(WardArrayDeployEvent {
                owner,
                owner_player_id,
                array_id,
                pos,
                radius: profile.radius,
                reflect_ratio: profile.reflect_ratio,
                placed_at_tick,
            });
        }
        ZhenfaKind::Lingju => {
            ling_events.send(LingArrayDeployEvent {
                owner,
                owner_player_id,
                array_id,
                pos,
                radius: profile.radius,
                density_multiplier: profile.density_multiplier,
                tiandao_gaze_weight: profile.tiandao_gaze_weight,
                placed_at_tick,
            });
        }
        ZhenfaKind::DeceiveHeaven => {
            deceive_events.send(DeceiveHeavenEvent {
                owner,
                owner_player_id,
                array_id,
                pos,
                self_weight_multiplier: 0.5,
                target_weight_multiplier: 1.5,
                reveal_chance_per_tick: profile.reveal_chance_per_tick,
                placed_at_tick,
            });
        }
        ZhenfaKind::Illusion => {
            illusion_events.send(IllusionArrayDeployEvent {
                owner,
                owner_player_id,
                array_id,
                pos,
                reveal_threshold: profile.reveal_threshold,
                placed_at_tick,
            });
        }
        ZhenfaKind::Trap | ZhenfaKind::Ward => {}
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
    Option<&'a Relationships>,
    Option<&'a Renown>,
);

type ZhenfaPlacePlayer<'a> = (
    &'a Username,
    &'a mut Cultivation,
    &'a QiColor,
    Option<&'a InsightModifiers>,
    Option<&'a PlayerInventory>,
    Option<&'a MeridianSeveredPermanent>,
    Option<&'a mut ArrayMastery>,
);

type ZhenfaTriggerPlayer<'a> = (
    &'a Position,
    &'a Cultivation,
    &'a QiColor,
    Option<&'a PlayerInventory>,
    Option<&'a mut ArrayMastery>,
);

type ZhenfaDisarmPlayer<'a> = (
    &'a Position,
    Option<&'a Username>,
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
    mut layers: Query<&mut ChunkLayer, With<OverworldLayer>>,
    mut targets: Query<ZhenfaDamageTarget<'_>>,
    mut practice_logs: Query<&mut PracticeLog>,
    ward_positions: Query<(Entity, &Position), Without<ZhenfaAnchor>>,
    mut combat_events: EventWriter<CombatEvent>,
    mut death_events: EventWriter<DeathEvent>,
    mut status_effects: EventWriter<ApplyStatusEffectIntent>,
    mut sense_pulses: EventWriter<ZhenfaSensePulse>,
    mut decay_events: EventWriter<ArrayDecayEvent>,
    mut deceive_exposed_events: EventWriter<DeceiveHeavenExposedEvent>,
    mut juebi_events: EventWriter<JueBiTriggerEvent>,
    mut pending_narrations: Option<ResMut<PendingGameplayNarrations>>,
    mut vfx_events: Option<ResMut<Events<VfxEventRequest>>>,
) {
    let now = clock.tick;
    let expired = registry.expire_at_or_before(now);
    if !expired.is_empty() {
        tracing::debug!("[bong][zhenfa] expired {} array eye(s)", expired.len());
    }
    for instance in &expired {
        remove_zhenfa_anchor_block(&mut layers, instance.pos);
        decay_events.send(ArrayDecayEvent {
            owner: instance.owner,
            owner_player_id: instance.owner_player_id.clone(),
            array_id: instance.id,
            kind: instance.kind,
            pos: instance.pos,
            decayed_at_tick: now,
        });
        commands.entity(instance.anchor_entity).despawn();
        emit_zhenfa_vfx(
            vfx_events.as_deref_mut(),
            gameplay_vfx::ZHENFA_DEPLETE,
            instance.pos,
            "#888888",
            0.45,
            8,
            30,
        );
    }

    let mut passive_triggers = Vec::new();
    let mut ward_alerts = Vec::new();
    let mut deceived_exposed = Vec::new();
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
            ZhenfaKind::ShrineWard => {
                apply_shrine_ward_pressure(
                    instance,
                    now,
                    &mut targets,
                    &mut combat_events,
                    &mut death_events,
                    &mut status_effects,
                );
            }
            ZhenfaKind::Lingju => {}
            ZhenfaKind::DeceiveHeaven => {
                if deceive_heaven_detected(instance.id, now, instance.realm_at_cast) {
                    deceived_exposed.push((
                        instance.id,
                        instance.owner,
                        instance.pos,
                        instance.anchor_entity,
                    ));
                }
            }
            ZhenfaKind::Illusion => {}
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
        emit_zhenfa_vfx(
            vfx_events.as_deref_mut(),
            gameplay_vfx::ZHENFA_WARD,
            pos,
            "#4488FF",
            0.7,
            20,
            60,
        );
    }

    for (id, owner, pos, anchor_entity) in deceived_exposed {
        if let Some(instance) = registry.remove(id) {
            juebi_events.send(JueBiTriggerEvent {
                entity: owner,
                source: JueBiTriggerSource::ZhenfaDeceptionExposed,
                delay_ticks: 0,
                triggered_at_tick: now,
                epicenter: Some([
                    f64::from(pos[0]) + 0.5,
                    f64::from(pos[1]),
                    f64::from(pos[2]) + 0.5,
                ]),
            });
            deceive_exposed_events.send(DeceiveHeavenExposedEvent {
                owner: instance.owner,
                owner_player_id: instance.owner_player_id.clone(),
                array_id: instance.id,
                pos: instance.pos,
                self_weight_multiplier: 0.5,
                target_weight_multiplier: 1.5,
                reveal_chance_per_tick: deceive_heaven_reveal_chance(instance.realm_at_cast),
                exposed_at_tick: now,
            });
        }
        commands.entity(anchor_entity).despawn();
        remove_zhenfa_anchor_block(&mut layers, pos);
    }

    let mut snapshots = registry.trigger_now(passive_triggers, now);
    snapshots.extend(registry.drain_due_chain_triggers(now));
    remove_zhenfa_anchor_blocks(&mut layers, snapshots.iter().map(|snapshot| snapshot.pos));
    despawn_triggered_anchors(&mut commands, &snapshots);
    apply_trigger_snapshots(
        snapshots,
        &mut targets,
        &mut practice_logs,
        &mut combat_events,
        &mut death_events,
        &mut status_effects,
        &mut sense_pulses,
        vfx_events.as_deref_mut(),
    );
}

#[allow(clippy::too_many_arguments)]
fn handle_zhenfa_disarm_requests(
    mut requests: EventReader<ZhenfaDisarmRequest>,
    mut registry: ResMut<ZhenfaRegistry>,
    mut commands: Commands,
    mut players: Query<ZhenfaDisarmPlayer<'_>>,
    mut layers: Query<&mut ChunkLayer, With<OverworldLayer>>,
    item_registry: Option<Res<ItemRegistry>>,
    mut allocator: Option<ResMut<InventoryInstanceIdAllocator>>,
    mut breakthrough_events: EventWriter<ArrayBreakthroughEvent>,
) {
    for req in requests.read() {
        let Ok((position, username, mut wounds, contamination, meridians, modifiers, inventory)) =
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
        remove_zhenfa_anchor_block(&mut layers, instance.pos);
        commands.entity(instance.anchor_entity).despawn();
        breakthrough_events.send(ArrayBreakthroughEvent {
            breaker: req.player,
            breaker_player_id: username
                .map(|username| canonical_player_id(username.0.as_str()))
                .unwrap_or_else(|| format!("entity_bits:{}", req.player.to_bits())),
            owner: instance.owner,
            owner_player_id: instance.owner_player_id.clone(),
            array_id: instance.id,
            kind: instance.kind,
            pos: instance.pos,
            force_break: req.mode == ZhenfaDisarmMode::ForceBreak,
            broken_at_tick: req.requested_at_tick,
        });

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

fn apply_shrine_ward_pressure(
    instance: &ZhenfaInstance,
    tick: u64,
    targets: &mut Query<ZhenfaDamageTarget<'_>>,
    combat_events: &mut EventWriter<CombatEvent>,
    death_events: &mut EventWriter<DeathEvent>,
    status_effects: &mut EventWriter<ApplyStatusEffectIntent>,
) {
    for (
        target,
        position,
        mut wounds,
        lifecycle,
        username,
        _contamination,
        _meridians,
        relationships,
        renown,
    ) in targets.iter_mut()
    {
        if !in_horizontal_radius(position.get(), instance.pos, instance.ward_radius)
            || shrine_ward_allows_target(instance, target, lifecycle, relationships, renown)
        {
            continue;
        }
        let was_alive = wounds.health_current > 0.0;
        let damage = shrine_ward_damage_per_tick(instance.realm_at_cast, instance.mastery_at_cast);
        wounds.health_current = (wounds.health_current - damage).clamp(0.0, wounds.health_max);
        wounds.entries.push(Wound {
            location: BodyPart::Chest,
            kind: WoundKind::Concussion,
            severity: 0.12,
            bleeding_per_sec: 0.0,
            created_at_tick: tick,
            inflicted_by: Some(format!("zhenfa_shrine_ward:{}", instance.id)),
        });
        status_effects.send(ApplyStatusEffectIntent {
            target,
            kind: StatusEffectKind::Stunned,
            magnitude: 0.10,
            duration_ticks: 5,
            issued_at_tick: tick,
        });
        combat_events.send(CombatEvent {
            attacker: instance.owner,
            target,
            resolved_at_tick: tick,
            body_part: BodyPart::Chest,
            wound_kind: WoundKind::Concussion,
            source: crate::combat::events::AttackSource::Melee,
            debug_command: false,
            damage,
            contam_delta: 0.0,
            description: format!(
                "zhenfa_shrine_ward {} -> {:?} radius {}",
                instance.id, target, instance.ward_radius
            ),
            defense_kind: None,
            defense_effectiveness: Some(instance.reflect_ratio()),
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
            let cause_target = username
                .map(|username| canonical_player_id(username.0.as_str()))
                .unwrap_or_else(|| format!("entity:{:?}", target));
            death_events.send(DeathEvent {
                target,
                cause: format!("zhenfa_shrine_ward:{cause_target}"),
                attacker: Some(instance.owner),
                attacker_player_id: Some(instance.owner_player_id.clone()),
                at_tick: tick,
            });
            tracing::warn!(
                "[bong][zhenfa] shrine ward reduced {:?} ({:?}) to zero health",
                target,
                username.map(|u| u.0.as_str())
            );
        }
    }
}

fn shrine_ward_allows_target(
    instance: &ZhenfaInstance,
    target: Entity,
    lifecycle: Option<&Lifecycle>,
    relationships: Option<&Relationships>,
    renown: Option<&Renown>,
) -> bool {
    if target == instance.owner {
        return true;
    }

    let Some(character_id) = lifecycle.map(|lifecycle| lifecycle.character_id.as_str()) else {
        return false;
    };
    if character_id == instance.owner_player_id {
        return true;
    }

    let is_ally = relationships.is_some_and(|relationships| {
        relationships.edges.iter().any(|edge| {
            edge.peer == instance.owner_player_id
                && matches!(
                    edge.kind,
                    RelationshipKindV1::Companion | RelationshipKindV1::Pact
                )
        })
    });
    let has_trust = renown.is_some_and(|renown| renown.fame >= 80);
    is_ally && has_trust
}

#[allow(clippy::too_many_arguments)]
fn apply_trigger_snapshots(
    snapshots: Vec<TriggerSnapshot>,
    targets: &mut Query<ZhenfaDamageTarget<'_>>,
    practice_logs: &mut Query<&mut PracticeLog>,
    combat_events: &mut EventWriter<CombatEvent>,
    death_events: &mut EventWriter<DeathEvent>,
    status_effects: &mut EventWriter<ApplyStatusEffectIntent>,
    sense_pulses: &mut EventWriter<ZhenfaSensePulse>,
    mut vfx_events: Option<&mut Events<VfxEventRequest>>,
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
        emit_zhenfa_vfx(
            vfx_events.as_deref_mut(),
            gameplay_vfx::ZHENFA_TRAP,
            snapshot.pos,
            "#FF3344",
            snapshot.qi_invest_ratio.clamp(0.3, 1.0) as f32,
            16,
            24,
        );

        let damage_profile = damage_profile(snapshot.qi_invest_ratio);
        let mut hit_any = false;
        for (
            target,
            position,
            mut wounds,
            lifecycle,
            username,
            contamination,
            meridians,
            _relationships,
            _renown,
        ) in targets.iter_mut()
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
                        meridian_id: None,
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
                debug_command: false,
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

fn emit_zhenfa_vfx(
    events: Option<&mut Events<VfxEventRequest>>,
    event_id: &'static str,
    pos: [i32; 3],
    color: &'static str,
    strength: f32,
    count: u32,
    duration_ticks: u32,
) {
    let Some(events) = events else {
        return;
    };
    gameplay_vfx::send_spawn(
        events,
        gameplay_vfx::spawn_request(
            event_id,
            gameplay_vfx::block_center(pos),
            Some([0.0, 1.0, 0.0]),
            color,
            strength,
            count,
            duration_ticks,
        ),
    );
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

fn sanitize_invest_ratio(requested: f64, min: f64, cap: f64) -> f64 {
    let min = min.clamp(0.0, 1.0);
    let cap = cap.clamp(min, 1.0);
    if !requested.is_finite() {
        return min;
    }
    requested.clamp(min, cap)
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

fn ward_radius(
    kind: ZhenfaKind,
    ratio: f64,
    profile_radius: u8,
    specialist: ZhenfaSpecialistLevel,
) -> u8 {
    if matches!(
        kind,
        ZhenfaKind::ShrineWard
            | ZhenfaKind::Lingju
            | ZhenfaKind::DeceiveHeaven
            | ZhenfaKind::Illusion
    ) {
        return profile_radius.max(1);
    }
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

fn shrine_ward_damage_per_tick(realm: Realm, mastery: f64) -> f32 {
    let realm_factor = match realm {
        Realm::Awaken | Realm::Induce => 1.0,
        Realm::Condense => 1.25,
        Realm::Solidify => 1.5,
        Realm::Spirit => 2.0,
        Realm::Void => 3.0,
    };
    (5.0 * realm_factor * (1.0 + mastery_ratio(mastery))) as f32
}

pub fn deceive_heaven_reveal_chance(realm: Realm) -> f64 {
    if realm == Realm::Void {
        0.002
    } else {
        0.005
    }
}

fn deceive_heaven_detected(array_id: u64, tick: u64, realm: Realm) -> bool {
    deterministic_tick_roll(array_id, tick) <= deceive_heaven_reveal_chance(realm)
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
        ZhenfaKind::ShrineWard => 0.35,
        ZhenfaKind::Lingju => 0.25,
        ZhenfaKind::DeceiveHeaven => 1.5,
        ZhenfaKind::Illusion => 0.2,
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
            meridian_id: None,
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

fn deterministic_tick_roll(instance_id: u64, tick: u64) -> f64 {
    let mut x = instance_id.rotate_left(17) ^ tick.wrapping_mul(0x9E37_79B9_7F4A_7C15);
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

fn block_pos_from_array(pos: [i32; 3]) -> BlockPos {
    BlockPos::new(pos[0], pos[1], pos[2])
}

fn zhenfa_anchor_block_state(kind: ZhenfaKind) -> BlockState {
    let state = BlockState::BONG_ZHENFA_EYE;
    if matches!(kind, ZhenfaKind::ShrineWard | ZhenfaKind::DeceiveHeaven) {
        state.set(PropName::Charged, PropValue::True)
    } else {
        state.set(PropName::Charged, PropValue::False)
    }
}

fn place_zhenfa_anchor_block(
    layers: &mut Query<&mut ChunkLayer, With<OverworldLayer>>,
    pos: [i32; 3],
    block: BlockState,
) -> Result<bool, String> {
    let Some(mut layer) = layers.iter_mut().next() else {
        tracing::warn!(
            "[bong][zhenfa] place_zhenfa_anchor_block skipped: OverworldLayer not found pos={:?}",
            pos
        );
        return Ok(false);
    };

    place_bong_block(&mut layer, block_pos_from_array(pos), block)
        .map(|_| true)
        .map_err(|error| error.to_string())
}

fn remove_zhenfa_anchor_block(
    layers: &mut Query<&mut ChunkLayer, With<OverworldLayer>>,
    pos: [i32; 3],
) {
    let Some(mut layer) = layers.iter_mut().next() else {
        tracing::warn!(
            "[bong][zhenfa] remove_zhenfa_anchor_block skipped: OverworldLayer not found pos={:?}",
            pos
        );
        return;
    };
    remove_bong_block(&mut layer, block_pos_from_array(pos));
}

fn remove_zhenfa_anchor_blocks(
    layers: &mut Query<&mut ChunkLayer, With<OverworldLayer>>,
    positions: impl IntoIterator<Item = [i32; 3]>,
) {
    let positions = positions.into_iter().collect::<Vec<_>>();
    if positions.is_empty() {
        return;
    }
    let Some(mut layer) = layers.iter_mut().next() else {
        tracing::warn!(
            "[bong][zhenfa] remove_zhenfa_anchor_blocks skipped: OverworldLayer not found count={}",
            positions.len()
        );
        return;
    };
    for pos in positions {
        remove_bong_block(&mut layer, block_pos_from_array(pos));
    }
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
    use valence::prelude::{App, ChunkLayer, DVec3, Entity, Events, UnloadedChunk};
    use valence::testing::ScenarioSingleClient;

    fn app_with_zhenfa() -> App {
        let mut app = App::new();
        install_zhenfa_test_systems(&mut app);
        app
    }

    fn app_with_zhenfa_layer() -> (App, Entity) {
        let scenario = ScenarioSingleClient::new();
        let mut app = scenario.app;
        crate::world::dimension::mark_test_layer_as_overworld(&mut app);
        app.world_mut()
            .get_mut::<ChunkLayer>(scenario.layer)
            .expect("test layer should carry ChunkLayer")
            .insert_chunk([0, 0], UnloadedChunk::new());
        install_zhenfa_test_systems(&mut app);
        (app, scenario.layer)
    }

    fn app_with_loaded_zhenfa() -> App {
        let (app, _) = app_with_zhenfa_layer();
        app
    }

    fn app_with_zhenfa_unloaded_layer() -> (App, Entity) {
        let scenario = ScenarioSingleClient::new();
        let mut app = scenario.app;
        crate::world::dimension::mark_test_layer_as_overworld(&mut app);
        install_zhenfa_test_systems(&mut app);
        (app, scenario.layer)
    }

    fn zhenfa_eye_state(charged: bool) -> BlockState {
        BlockState::BONG_ZHENFA_EYE.set(
            PropName::Charged,
            if charged {
                PropValue::True
            } else {
                PropValue::False
            },
        )
    }

    fn layer_block_state(app: &App, layer_entity: Entity, pos: [i32; 3]) -> Option<BlockState> {
        app.world()
            .get::<ChunkLayer>(layer_entity)
            .and_then(|layer| {
                layer
                    .block(block_pos_from_array(pos))
                    .map(|block| block.state)
            })
    }

    fn install_zhenfa_test_systems(app: &mut App) {
        app.insert_resource(CombatClock::default());
        app.insert_resource(PendingGameplayNarrations::default());
        app.add_event::<ZhenfaPlaceRequest>();
        app.add_event::<ZhenfaTriggerRequest>();
        app.add_event::<ZhenfaDisarmRequest>();
        app.add_event::<ZhenfaSensePulse>();
        app.add_event::<WardArrayDeployEvent>();
        app.add_event::<LingArrayDeployEvent>();
        app.add_event::<DeceiveHeavenEvent>();
        app.add_event::<DeceiveHeavenExposedEvent>();
        app.add_event::<IllusionArrayDeployEvent>();
        app.add_event::<ArrayDecayEvent>();
        app.add_event::<ArrayBreakthroughEvent>();
        app.add_event::<JueBiTriggerEvent>();
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

    #[test]
    fn activate_emits_vfx() {
        let mut app = app_with_zhenfa();
        app.add_event::<VfxEventRequest>();
        let owner = spawn_player(&mut app, "owner", [0.0, 64.0, 0.0]);
        let _target = spawn_player(&mut app, "intruder", [1.5, 64.0, 0.5]);
        let anchor_entity = app.world_mut().spawn_empty().id();
        let id = app
            .world_mut()
            .resource_mut::<ZhenfaRegistry>()
            .insert(ZhenfaInstance {
                id: 0,
                kind: ZhenfaKind::Trap,
                owner,
                owner_player_id: "player:owner".to_string(),
                pos: [1, 64, 0],
                carrier: ZhenfaCarrierKind::CommonStone,
                qi_invest_ratio: 0.2,
                qi_invest_amount: 20.0,
                realm_at_cast: Realm::Induce,
                mastery_at_cast: 0.0,
                effect_radius: 1,
                ward_radius: 1,
                placed_at_tick: 1,
                expires_at_tick: 100,
                triggered_at: None,
                trigger: None,
                color_main: ColorKind::Intricate,
                color_secondary: None,
                anchor_entity,
            })
            .expect("insert trap");

        app.world_mut().send_event(ZhenfaTriggerRequest {
            player: owner,
            instance_id: Some(id),
            requested_at_tick: 10,
        });
        app.update();

        let events = app.world().resource::<Events<VfxEventRequest>>();
        let emitted = events
            .iter_current_update_events()
            .next()
            .expect("zhenfa trigger should emit vfx");
        match &emitted.payload {
            crate::schema::vfx_event::VfxEventPayloadV1::SpawnParticle { event_id, .. } => {
                assert_eq!(event_id, gameplay_vfx::ZHENFA_TRAP);
            }
            other => panic!("expected SpawnParticle, got {other:?}"),
        }
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
        let mut app = app_with_loaded_zhenfa();
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
    fn placement_writes_and_disarm_removes_custom_block() {
        let (mut app, layer_entity) = app_with_zhenfa_layer();
        let owner = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);
        let pos = [1, 64, 1];

        app.world_mut().send_event(ZhenfaPlaceRequest {
            player: owner,
            pos,
            kind: ZhenfaKind::Trap,
            carrier: ZhenfaCarrierKind::LingqiBlock,
            qi_invest_ratio: 0.10,
            trigger: None,
            requested_at_tick: 10,
        });
        app.update();

        assert_eq!(
            layer_block_state(&app, layer_entity, pos),
            Some(zhenfa_eye_state(false))
        );

        app.world_mut().send_event(ZhenfaDisarmRequest {
            player: owner,
            pos,
            mode: ZhenfaDisarmMode::ForceBreak,
            requested_at_tick: 11,
        });
        app.update();

        assert_eq!(
            layer_block_state(&app, layer_entity, pos),
            Some(BlockState::AIR)
        );
        assert!(app
            .world()
            .resource::<ZhenfaRegistry>()
            .find_at(pos)
            .is_none());
    }

    #[test]
    fn shrine_ward_writes_charged_custom_block() {
        let (mut app, layer_entity) = app_with_zhenfa_layer();
        let owner = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);
        let pos = [0, 64, 0];

        app.world_mut().send_event(ZhenfaPlaceRequest {
            player: owner,
            pos,
            kind: ZhenfaKind::ShrineWard,
            carrier: ZhenfaCarrierKind::LingqiBlock,
            qi_invest_ratio: 0.20,
            trigger: None,
            requested_at_tick: 10,
        });
        app.update();

        assert_eq!(
            layer_block_state(&app, layer_entity, pos),
            Some(zhenfa_eye_state(true))
        );
        assert!(app
            .world()
            .resource::<ZhenfaRegistry>()
            .find_at(pos)
            .is_some());
    }

    #[test]
    fn placement_rejects_unloaded_chunk_without_qi_debit_or_registry_entry() {
        let (mut app, layer_entity) = app_with_zhenfa_unloaded_layer();
        let owner = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);
        let pos = [1, 64, 1];

        app.world_mut().send_event(ZhenfaPlaceRequest {
            player: owner,
            pos,
            kind: ZhenfaKind::Trap,
            carrier: ZhenfaCarrierKind::LingqiBlock,
            qi_invest_ratio: 0.20,
            trigger: None,
            requested_at_tick: 10,
        });
        app.update();

        assert_eq!(
            app.world().get::<Cultivation>(owner).unwrap().qi_current,
            100.0
        );
        assert_eq!(app.world().resource::<ZhenfaRegistry>().len(), 0);
        assert_eq!(layer_block_state(&app, layer_entity, pos), None);
    }

    #[test]
    fn placement_registry_failure_cleans_world_block_and_anchor_entity() {
        let (mut app, layer_entity) = app_with_zhenfa_layer();
        let owner = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);
        let pos = [1, 64, 1];
        app.world_mut()
            .resource_mut::<ZhenfaRegistry>()
            .by_pos
            .insert(pos, 999);

        app.world_mut().send_event(ZhenfaPlaceRequest {
            player: owner,
            pos,
            kind: ZhenfaKind::Trap,
            carrier: ZhenfaCarrierKind::LingqiBlock,
            qi_invest_ratio: 0.20,
            trigger: None,
            requested_at_tick: 10,
        });
        app.update();

        assert_eq!(
            app.world().get::<Cultivation>(owner).unwrap().qi_current,
            100.0
        );
        assert_eq!(app.world().resource::<ZhenfaRegistry>().len(), 0);
        assert_eq!(
            layer_block_state(&app, layer_entity, pos),
            Some(BlockState::AIR)
        );
        let anchor_count = {
            let world = app.world_mut();
            let mut query = world.query::<&ZhenfaAnchor>();
            query.iter(world).count()
        };
        assert_eq!(anchor_count, 0);
    }

    #[test]
    fn placement_rejects_missing_overworld_layer_without_qi_debit_or_registry_entry() {
        let mut app = app_with_zhenfa();
        let owner = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);

        app.world_mut().send_event(ZhenfaPlaceRequest {
            player: owner,
            pos: [1, 64, 1],
            kind: ZhenfaKind::Trap,
            carrier: ZhenfaCarrierKind::LingqiBlock,
            qi_invest_ratio: 0.20,
            trigger: None,
            requested_at_tick: 10,
        });
        app.update();

        assert_eq!(
            app.world().get::<Cultivation>(owner).unwrap().qi_current,
            100.0
        );
        assert_eq!(app.world().resource::<ZhenfaRegistry>().len(), 0);
    }

    #[test]
    fn duplicate_same_block_is_rejected_without_second_qi_debit() {
        let mut app = app_with_loaded_zhenfa();
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
        let (mut app, layer_entity) = app_with_zhenfa_layer();
        let owner = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);
        let pos = [3, 64, 3];
        app.world_mut().send_event(ZhenfaPlaceRequest {
            player: owner,
            pos,
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
            .find_at(pos)
            .unwrap()
            .anchor_entity;
        assert_eq!(
            layer_block_state(&app, layer_entity, pos),
            Some(zhenfa_eye_state(false))
        );
        app.world_mut().resource_mut::<CombatClock>().tick =
            carrier_spec(ZhenfaCarrierKind::CommonStone).duration_ticks + 1;
        app.update();

        assert!(app
            .world()
            .resource::<ZhenfaRegistry>()
            .find_at(pos)
            .is_none());
        assert!(app.world().get_entity(anchor_entity).is_none());
        assert_eq!(
            layer_block_state(&app, layer_entity, pos),
            Some(BlockState::AIR)
        );
    }

    #[test]
    fn passive_trap_trigger_damages_legs_and_frees_array_eye() {
        let (mut app, layer_entity) = app_with_zhenfa_layer();
        let owner = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);
        let intruder = spawn_player(&mut app, "Bob", [5.5, 64.0, 5.5]);
        let pos = [5, 64, 5];
        app.world_mut().send_event(ZhenfaPlaceRequest {
            player: owner,
            pos,
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
            .find_at(pos)
            .unwrap()
            .id;
        let anchor_entity = app
            .world()
            .resource::<ZhenfaRegistry>()
            .find_at(pos)
            .unwrap()
            .anchor_entity;
        assert_eq!(
            layer_block_state(&app, layer_entity, pos),
            Some(zhenfa_eye_state(false))
        );
        app.world_mut().resource_mut::<CombatClock>().tick = 11;
        app.update();

        let registry = app.world().resource::<ZhenfaRegistry>();
        assert!(registry.get(id).is_none());
        assert!(registry.find_at(pos).is_none());
        assert!(app.world().get_entity(anchor_entity).is_none());
        assert_eq!(
            layer_block_state(&app, layer_entity, pos),
            Some(BlockState::AIR)
        );
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
        let mut app = app_with_loaded_zhenfa();
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
        let (mut app, layer_entity) = app_with_zhenfa_layer();
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
        assert_eq!(
            layer_block_state(&app, layer_entity, [3, 64, 0]),
            Some(BlockState::AIR)
        );
        assert_eq!(
            layer_block_state(&app, layer_entity, [10, 64, 0]),
            Some(zhenfa_eye_state(false))
        );
    }

    #[test]
    fn ward_alert_fires_on_entry_for_position_only_entities() {
        let mut app = app_with_loaded_zhenfa();
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
        let mut app = app_with_loaded_zhenfa();
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
        let mut app = app_with_loaded_zhenfa();
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
        assert_eq!(
            ward_radius(ZhenfaKind::Ward, 0.20, 8, ZhenfaSpecialistLevel::None),
            4
        );
        assert_eq!(
            ward_radius(ZhenfaKind::Ward, 0.20, 8, ZhenfaSpecialistLevel::Expert),
            8
        );
        assert!(in_horizontal_radius(
            DVec3::new(1.5, 64.0, 1.5),
            [1, 64, 1],
            0
        ));
    }

    #[test]
    fn shrine_ward_deploy_emits_event_and_burns_intruder() {
        let mut app = app_with_loaded_zhenfa();
        let owner = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);
        let intruder = spawn_player(&mut app, "Bob", [4.5, 64.0, 0.5]);
        app.world_mut().send_event(ZhenfaPlaceRequest {
            player: owner,
            pos: [0, 64, 0],
            kind: ZhenfaKind::ShrineWard,
            carrier: ZhenfaCarrierKind::LingqiBlock,
            qi_invest_ratio: 0.20,
            trigger: None,
            requested_at_tick: 1,
        });
        app.update();

        assert!(!app
            .world()
            .resource::<Events<WardArrayDeployEvent>>()
            .is_empty());
        app.world_mut().resource_mut::<CombatClock>().tick = 2;
        app.update();

        let wounds = app.world().get::<Wounds>(intruder).unwrap();
        assert!(wounds.health_current < wounds.health_max);
        assert!(wounds
            .entries
            .iter()
            .any(|w| w.inflicted_by.as_deref() == Some("zhenfa_shrine_ward:1")));
    }

    #[test]
    fn shrine_ward_lethal_pressure_emits_death_event() {
        let mut app = app_with_loaded_zhenfa();
        let owner = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);
        let intruder = spawn_player(&mut app, "Bob", [4.5, 64.0, 0.5]);
        app.world_mut()
            .get_mut::<Wounds>(intruder)
            .unwrap()
            .health_current = 4.0;
        app.world_mut().send_event(ZhenfaPlaceRequest {
            player: owner,
            pos: [0, 64, 0],
            kind: ZhenfaKind::ShrineWard,
            carrier: ZhenfaCarrierKind::LingqiBlock,
            qi_invest_ratio: 0.20,
            trigger: None,
            requested_at_tick: 1,
        });
        app.update();

        app.world_mut().resource_mut::<CombatClock>().tick = 2;
        app.update();

        let deaths: Vec<_> = app
            .world()
            .resource::<Events<DeathEvent>>()
            .get_reader()
            .read(app.world().resource::<Events<DeathEvent>>())
            .cloned()
            .collect();
        assert_eq!(deaths.len(), 1);
        assert_eq!(deaths[0].target, intruder);
        assert_eq!(deaths[0].attacker, Some(owner));
        assert_eq!(
            deaths[0].attacker_player_id.as_deref(),
            Some("offline:Alice")
        );
        assert_eq!(deaths[0].cause, "zhenfa_shrine_ward:offline:Bob");
    }

    #[test]
    fn shrine_ward_allows_trusted_allies() {
        let mut app = app_with_loaded_zhenfa();
        let owner = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);
        let ally = spawn_player(&mut app, "Bob", [4.5, 64.0, 0.5]);
        app.world_mut().entity_mut(ally).insert((
            Lifecycle {
                character_id: "offline:Bob".to_string(),
                ..Default::default()
            },
            Relationships {
                edges: vec![crate::social::components::Relationship {
                    kind: RelationshipKindV1::Pact,
                    peer: canonical_player_id("Alice"),
                    since_tick: 0,
                    metadata: serde_json::Value::Null,
                }],
            },
            Renown {
                fame: 80,
                ..Default::default()
            },
        ));

        app.world_mut().send_event(ZhenfaPlaceRequest {
            player: owner,
            pos: [0, 64, 0],
            kind: ZhenfaKind::ShrineWard,
            carrier: ZhenfaCarrierKind::LingqiBlock,
            qi_invest_ratio: 0.20,
            trigger: None,
            requested_at_tick: 1,
        });
        app.update();
        app.world_mut().resource_mut::<CombatClock>().tick = 2;
        app.update();

        let wounds = app.world().get::<Wounds>(ally).unwrap();
        assert_eq!(wounds.health_current, wounds.health_max);
        assert!(wounds.entries.is_empty());
    }

    #[test]
    fn deceive_heaven_requires_solidify_or_higher() {
        let mut app = app_with_zhenfa();
        let owner = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);
        app.world_mut().send_event(ZhenfaPlaceRequest {
            player: owner,
            pos: [0, 64, 0],
            kind: ZhenfaKind::DeceiveHeaven,
            carrier: ZhenfaCarrierKind::BeastCoreInlaid,
            qi_invest_ratio: 0.90,
            trigger: None,
            requested_at_tick: 1,
        });
        app.update();

        assert_eq!(app.world().resource::<ZhenfaRegistry>().len(), 0);
    }

    #[test]
    fn deceive_heaven_exposure_emits_dedicated_event() {
        let mut app = app_with_loaded_zhenfa();
        let owner = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);
        app.world_mut().entity_mut(owner).insert(Cultivation {
            realm: Realm::Solidify,
            qi_current: 100.0,
            qi_max: 100.0,
            ..Default::default()
        });
        app.world_mut().send_event(ZhenfaPlaceRequest {
            player: owner,
            pos: [0, 64, 0],
            kind: ZhenfaKind::DeceiveHeaven,
            carrier: ZhenfaCarrierKind::BeastCoreInlaid,
            qi_invest_ratio: 0.90,
            trigger: None,
            requested_at_tick: 1,
        });
        app.update();

        let instance_id = app
            .world()
            .resource::<ZhenfaRegistry>()
            .find_at([0, 64, 0])
            .unwrap()
            .id;
        let exposure_tick = (2..10_000)
            .find(|tick| deceive_heaven_detected(instance_id, *tick, Realm::Solidify))
            .expect("deterministic exposure tick should exist in test window");
        app.world_mut().resource_mut::<CombatClock>().tick = exposure_tick;
        app.update();

        assert!(app
            .world()
            .resource::<ZhenfaRegistry>()
            .find_at([0, 64, 0])
            .is_none());
        assert!(!app
            .world()
            .resource::<Events<DeceiveHeavenExposedEvent>>()
            .is_empty());
        assert!(app
            .world()
            .resource::<Events<JueBiTriggerEvent>>()
            .iter_current_update_events()
            .any(|event| event.source == JueBiTriggerSource::ZhenfaDeceptionExposed));
    }

    #[test]
    fn severed_kidney_blocks_lingju_array() {
        let mut app = app_with_zhenfa();
        let owner = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);
        let mut severed = MeridianSeveredPermanent::default();
        severed.insert(
            MeridianId::Kidney,
            crate::cultivation::meridian::severed::SeveredSource::CombatWound,
            1,
        );
        app.world_mut().entity_mut(owner).insert(severed);

        app.world_mut().send_event(ZhenfaPlaceRequest {
            player: owner,
            pos: [0, 64, 0],
            kind: ZhenfaKind::Lingju,
            carrier: ZhenfaCarrierKind::BeastCoreInlaid,
            qi_invest_ratio: 0.30,
            trigger: None,
            requested_at_tick: 1,
        });
        app.update();

        assert_eq!(app.world().resource::<ZhenfaRegistry>().len(), 0);
    }

    #[test]
    fn array_mastery_grows_on_cast_and_trigger() {
        let mut app = app_with_loaded_zhenfa();
        let owner = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);
        app.world_mut()
            .entity_mut(owner)
            .insert(ArrayMastery::default());
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
        assert_eq!(app.world().get::<ArrayMastery>(owner).unwrap().trap, 0.3);

        app.world_mut().send_event(ZhenfaTriggerRequest {
            player: owner,
            instance_id: None,
            requested_at_tick: 2,
        });
        app.update();
        assert_eq!(app.world().get::<ArrayMastery>(owner).unwrap().trap, 1.3);
    }

    #[test]
    fn zhenfa_v2_profiles_encode_plan_thresholds() {
        let lingju = zhenfa_kind_profile(
            ZhenfaKind::Lingju,
            Realm::Void,
            100.0,
            ZhenfaCarrierKind::BeastCoreInlaid,
        );
        assert_eq!(lingju.radius, 60);
        assert_eq!(lingju.density_multiplier, 3.5);
        assert!(lingju.duration_ticks > 6 * 60 * 60 * TICKS_PER_SECOND);

        let deceive = zhenfa_kind_profile(
            ZhenfaKind::DeceiveHeaven,
            Realm::Void,
            0.0,
            ZhenfaCarrierKind::BeastCoreInlaid,
        );
        assert_eq!(deceive.min_invest_ratio, 0.80);
        assert_eq!(deceive.reveal_chance_per_tick, 0.002);
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
            realm_at_cast: Realm::Induce,
            mastery_at_cast: 0.0,
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
        assert_eq!(instance.rejection_rate(), 0.35);
        assert_eq!(instance.medium().carrier, CarrierGrade::SpiritWeapon);
        assert_eq!(instance.defense_color(), ColorKind::Solid);
        assert_eq!(instance.resistance(), 0.5);
    }

    #[test]
    fn zhenfa_anchor_visual_state_reflects_registry_lifecycle() {
        let mut registry = ZhenfaRegistry::default();
        assert_eq!(
            registry.anchor_visual_state(&ZhenfaAnchor { id: 0 }),
            ZHENFA_VISUAL_STATE_INACTIVE
        );

        let id = registry
            .insert(ZhenfaInstance {
                id: 0,
                kind: ZhenfaKind::Trap,
                owner: Entity::from_raw(1),
                owner_player_id: "offline:Azure".to_string(),
                pos: [1, 64, 1],
                carrier: ZhenfaCarrierKind::LingqiBlock,
                qi_invest_ratio: 0.5,
                qi_invest_amount: 25.0,
                realm_at_cast: Realm::Induce,
                mastery_at_cast: 0.0,
                effect_radius: 2,
                ward_radius: 8,
                placed_at_tick: 1,
                expires_at_tick: 100,
                triggered_at: None,
                trigger: None,
                color_main: ColorKind::Intricate,
                color_secondary: Some(ColorKind::Solid),
                anchor_entity: Entity::from_raw(2),
            })
            .unwrap();
        assert_eq!(
            registry.anchor_visual_state(&ZhenfaAnchor { id }),
            ZHENFA_VISUAL_STATE_ACTIVE
        );

        registry
            .pending_chain
            .push_back(PendingChainTrigger { id, due_tick: 8 });
        assert_eq!(
            registry.anchor_visual_state(&ZhenfaAnchor { id }),
            ZHENFA_VISUAL_STATE_EXHAUSTED
        );
        assert_eq!(
            registry.anchor_visual_state(&ZhenfaAnchor { id: 999 }),
            ZHENFA_VISUAL_STATE_EXHAUSTED
        );
    }
}
