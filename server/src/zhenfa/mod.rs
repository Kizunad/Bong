use std::collections::{HashMap, HashSet, VecDeque};

use serde::{Deserialize, Serialize};
use valence::prelude::{
    bevy_ecs, bevy_ecs::system::SystemParam, App, BlockPos, BlockState, ChunkLayer, Client,
    Commands, Component, DVec3, Entity, Event, EventReader, EventWriter, Events, IntoSystemConfigs,
    Mut, Position, PropName, PropValue, Query, Res, ResMut, Resource, SystemSet, UniqueId, Update,
    Username, With, Without,
};

use crate::combat::components::{BodyPart, Lifecycle, LifecycleState, Wound, WoundKind, Wounds};
use crate::combat::events::{
    ApplyStatusEffectIntent, AttackIntent, CombatEvent, DeathEvent, StatusEffectKind,
};
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
use crate::fauna::components::{BeastKind, FaunaTag};
use crate::inventory::{
    add_item_to_player_inventory, consume_item_instance_once, inventory_item_by_instance_borrow,
    InventoryInstanceIdAllocator, ItemRegistry, PlayerInventory,
};
use crate::lingtian::{LingtianPlot, PLOT_QI_CAP_MAX, QI_LINGJU_ARRAY_CAP_BONUS};
use crate::network::{gameplay_vfx, vfx_event_emit::VfxEventRequest};
use crate::npc::spawn::DecoyTarget;
use crate::player::gameplay::PendingGameplayNarrations;
use crate::player::state::canonical_player_id;
use crate::qi_physics::constants::{
    QI_EPSILON, QI_NETWORK_ARRAY_LINGJU_CAP_BONUS, QI_SCATTER_BEAD_CAPACITY, QI_ZONE_UNIT_CAPACITY,
};
use crate::qi_physics::{
    qi_excretion, qi_release_to_zone, CarrierGrade, ContainerKind, EnvField, MediumKind,
    QiAccountId, QiTransfer, QiTransferReason, StyleAttack, StyleDefense, WorldQiAccount,
};
use crate::schema::common::NarrationStyle;
use crate::schema::realm_vision::{SenseEntryV1, SenseKindV1, SpiritualSenseTargetsV1};
use crate::schema::social::RelationshipKindV1;
use crate::social::components::{Relationships, Renown};
use crate::world::{
    bong_blocks::{place_bong_block, remove_bong_block},
    dimension::{DimensionKind, OverworldLayer},
    zone::{ZoneRegistry, DEFAULT_SPAWN_ZONE_NAME},
};

mod network_array;
pub mod trap_content;

const TICKS_PER_SECOND: u64 = 20;
const MIN_QI_INVEST_RATIO: f64 = 0.05;
// 旧阵旗道具：用于已实装拆阵/布阵检测；凡阶组网阵改走 array_flag_basic/array_eye_basic（P3）。
const ZHENFA_FLAG_ITEM_ID: &str = "array_flag";
// 破阵被动掉落物；主动投掷/埋设散逸道具是 qi_scatter_bead（plan-zhenfa-content-v2 P2）。
const ZHENFA_PEARL_ITEM_ID: &str = "scattered_qi_pearl";
const CHAIN_DELAY_TICKS: u64 = 6;
const WARD_ALERT_THROTTLE_TICKS: u64 = 60 * TICKS_PER_SECOND;
const BEAST_TRAP_IMMOBILIZED_TICKS: u64 = 8 * TICKS_PER_SECOND;
const BEAST_TRAP_SNAP_DAMAGE: f32 = 2.0;
const BAIT_STAKE_DURABILITY_HITS: u8 = 4;
const DISARM_RANGE: f64 = 4.5;
const QI_SCATTER_BEAD_ITEM_ID: &str = "qi_scatter_bead";
const NETWORK_ARRAY_FLAG_ITEM_ID: &str = "array_flag_basic";
const NETWORK_ARRAY_EYE_ITEM_ID: &str = "array_eye_basic";
const SCATTER_DISTURBANCE_EVENT: &str = "scatter_disturbance";
const SCATTER_DISTURBANCE_DURATION_TICKS: u64 = 30 * TICKS_PER_SECOND;
pub const DECEIVE_HEAVEN_DURATION_TICKS: u64 = 30 * 60 * TICKS_PER_SECOND;
pub const DECEIVE_HEAVEN_REVEAL_CHANCE: f64 = 0.10;
const DECEIVE_HEAVEN_SPIRITWOOD_ITEM_ID: &str = "ling_mu_ban";
const DECEIVE_HEAVEN_SPIRITWOOD_COST: u32 = 2;
const DECEIVE_HEAVEN_BEAST_BONE_ITEM_ID: &str = "yi_shou_gu";
const DECEIVE_HEAVEN_BEAST_BONE_COST: u32 = 4;
const DECEIVE_HEAVEN_BONE_COIN_COST: u64 = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ZhenfaKind {
    Trap,
    Ward,
    WarningTrap,
    BlastTrap,
    SlowTrap,
    BeastTrap,
    TripWire,
    DecoyStake,
    ShrineWard,
    Lingju,
    DeceiveHeaven,
    Illusion,
    NetworkArray,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemSet)]
pub enum ZhenfaSystemSet {
    Runtime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum ZhenfaCarrierKind {
    #[default]
    CommonStone,
    LingqiBlock,
    NightWitheredVine,
    BeastCoreInlaid,
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
    pub item_instance_id: Option<u64>,
    pub target_face: Option<trap_content::TrapTargetFace>,
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
pub struct ScatterBeadUseRequest {
    pub player: Entity,
    pub item_instance_id: u64,
    pub bury_pos: Option<[i32; 3]>,
    pub requested_at_tick: u64,
}

#[derive(Debug, Clone, Event)]
pub struct ScatterBeadTriggerRequest {
    pub player: Entity,
    pub bead_id: u64,
    pub requested_at_tick: u64,
}

#[derive(Debug, Clone, PartialEq)]
struct ScatterBeadBurial {
    id: u64,
    owner: Entity,
    owner_player_id: String,
    pos: [i32; 3],
    remaining_qi: f64,
    last_tick: u64,
}

#[derive(Debug, Default, Resource)]
struct ScatterBeadBurials {
    next_id: u64,
    beads: HashMap<u64, ScatterBeadBurial>,
}

impl ScatterBeadBurials {
    fn insert(
        &mut self,
        owner: Entity,
        owner_player_id: impl Into<String>,
        pos: [i32; 3],
        remaining_qi: f64,
        placed_at_tick: u64,
    ) -> u64 {
        self.next_id = self.next_id.saturating_add(1).max(1);
        let id = self.next_id;
        self.beads.insert(
            id,
            ScatterBeadBurial {
                id,
                owner,
                owner_player_id: owner_player_id.into(),
                pos,
                remaining_qi: remaining_qi.clamp(0.0, QI_SCATTER_BEAD_CAPACITY),
                last_tick: placed_at_tick,
            },
        );
        id
    }

    fn trigger_buried(
        &mut self,
        id: u64,
        requester: Entity,
        triggered_at_tick: u64,
    ) -> Option<ScatterBeadBurial> {
        let bead = self.beads.get(&id)?;
        if bead.owner != requester {
            return None;
        }
        let mut bead = self.beads.remove(&id)?;
        bead.last_tick = triggered_at_tick;
        Some(bead)
    }
}

#[derive(Debug, Default, Resource)]
struct ScatterDisturbanceZones {
    expires_at: HashMap<String, u64>,
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
    pub reveal_chance: f64,
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
    pub reveal_chance: f64,
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
pub struct NetworkArrayDeployEvent {
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

#[derive(Debug, Clone, Copy, Component, PartialEq, Eq)]
pub struct BaitDurability {
    pub remaining_hits: u8,
}

impl BaitDurability {
    pub const fn new(remaining_hits: u8) -> Self {
        Self { remaining_hits }
    }
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
    pub network_array: f64,
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
            network_array: 0.0,
        }
    }
}

impl ArrayMastery {
    pub fn value(&self, kind: ZhenfaKind) -> f64 {
        match kind {
            ZhenfaKind::Trap
            | ZhenfaKind::WarningTrap
            | ZhenfaKind::BlastTrap
            | ZhenfaKind::SlowTrap
            | ZhenfaKind::BeastTrap
            | ZhenfaKind::TripWire
            | ZhenfaKind::DecoyStake => self.trap,
            ZhenfaKind::Ward => self.ward,
            ZhenfaKind::ShrineWard => self.shrine_ward,
            ZhenfaKind::Lingju => self.lingju,
            ZhenfaKind::DeceiveHeaven => self.deceive_heaven,
            ZhenfaKind::Illusion => self.illusion,
            ZhenfaKind::NetworkArray => self.network_array,
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
            ZhenfaKind::Trap
            | ZhenfaKind::WarningTrap
            | ZhenfaKind::BlastTrap
            | ZhenfaKind::SlowTrap
            | ZhenfaKind::BeastTrap
            | ZhenfaKind::TripWire
            | ZhenfaKind::DecoyStake => &mut self.trap,
            ZhenfaKind::Ward => &mut self.ward,
            ZhenfaKind::ShrineWard => &mut self.shrine_ward,
            ZhenfaKind::Lingju => &mut self.lingju,
            ZhenfaKind::DeceiveHeaven => &mut self.deceive_heaven,
            ZhenfaKind::Illusion => &mut self.illusion,
            ZhenfaKind::NetworkArray => &mut self.network_array,
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
    kind: ZhenfaKind,
    owner: Entity,
    owner_player_id: String,
    pos: [i32; 3],
    triggered_at_tick: u64,
    qi_invest_ratio: f64,
    qi_invest_amount: f64,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NetworkArrayPlaceItem {
    Flag,
    Eye,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum PlotCapSource {
    Lingju(u64),
    NetworkArray(u64),
}

#[derive(Debug, Clone, PartialEq)]
struct ActiveNetworkArray {
    id: u64,
    owner: Entity,
    owner_player_id: String,
    eye_instance_id: u64,
    eye_pos: [i32; 3],
    flag_instance_ids: Vec<u64>,
    flag_positions: Vec<[i32; 3]>,
    hull: Vec<[i32; 3]>,
    area: f64,
    formed_at_tick: u64,
}

#[derive(Debug, Default)]
struct NetworkArrayRegistry {
    flags: HashSet<u64>,
    eyes: HashSet<u64>,
    flag_to_network: HashMap<u64, u64>,
    eye_to_network: HashMap<u64, u64>,
    active: HashMap<u64, ActiveNetworkArray>,
    dissolved: Vec<ActiveNetworkArray>,
}

#[derive(Debug, Default, Resource)]
pub struct ZhenfaRegistry {
    next_id: u64,
    instances: HashMap<u64, ZhenfaInstance>,
    by_pos: HashMap<[i32; 3], u64>,
    pending_chain: VecDeque<PendingChainTrigger>,
    ward_alert_seen: HashMap<(u64, Entity), u64>,
    ward_inside: HashSet<(u64, Entity)>,
    slow_charges_remaining: HashMap<u64, u8>,
    slow_inside: HashSet<(u64, Entity)>,
    network_inside: HashSet<(u64, Entity)>,
    network_arrays: NetworkArrayRegistry,
    plot_cap_sources: HashMap<BlockPos, HashMap<PlotCapSource, f32>>,
    plot_cap_base_caps: HashMap<BlockPos, f32>,
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
    app.init_resource::<WorldQiAccount>();
    app.insert_resource(ScatterBeadBurials::default());
    app.insert_resource(ScatterDisturbanceZones::default());
    app.add_event::<ZhenfaPlaceRequest>();
    app.add_event::<ZhenfaTriggerRequest>();
    app.add_event::<ZhenfaDisarmRequest>();
    app.add_event::<ScatterBeadUseRequest>();
    app.add_event::<ScatterBeadTriggerRequest>();
    app.add_event::<ZhenfaSensePulse>();
    app.add_event::<WardArrayDeployEvent>();
    app.add_event::<LingArrayDeployEvent>();
    app.add_event::<DeceiveHeavenEvent>();
    app.add_event::<DeceiveHeavenExposedEvent>();
    app.add_event::<IllusionArrayDeployEvent>();
    app.add_event::<NetworkArrayDeployEvent>();
    app.add_event::<ArrayDecayEvent>();
    app.add_event::<ArrayBreakthroughEvent>();
    app.add_event::<AttackIntent>();
    app.add_event::<QiTransfer>();
    app.add_systems(
        Update,
        (
            handle_zhenfa_place_requests,
            handle_scatter_bead_use,
            handle_scatter_bead_trigger_requests,
            handle_zhenfa_trigger_requests,
            handle_zhenfa_disarm_requests,
            tick_scatter_bead_excretion,
            tick_scatter_disturbance_zones,
            handle_decoy_stake_attack_intents,
            tick_zhenfa_registry,
            emit_zhenfa_sense_pulses,
        )
            .chain()
            .in_set(ZhenfaSystemSet::Runtime),
    );
}

impl NetworkArrayRegistry {
    fn mark_flag(&mut self, instance_id: u64) {
        self.flags.insert(instance_id);
    }

    fn mark_eye(&mut self, instance_id: u64) {
        self.eyes.insert(instance_id);
    }

    fn try_form_network(
        &mut self,
        eye: &ZhenfaInstance,
        instances: &HashMap<u64, ZhenfaInstance>,
        formed_at_tick: u64,
    ) -> Option<ActiveNetworkArray> {
        if !self.eyes.contains(&eye.id) || self.eye_to_network.contains_key(&eye.id) {
            return None;
        }
        let flags = self
            .flags
            .iter()
            .filter(|id| !self.flag_to_network.contains_key(id))
            .filter_map(|id| instances.get(id))
            .map(|instance| network_array::NetworkFlag {
                instance_id: instance.id,
                owner: instance.owner,
                pos: instance.pos,
            })
            .collect::<Vec<_>>();
        let geometry = network_array::try_form_network(
            eye.pos,
            eye.owner,
            &flags,
            network_array::NETWORK_ARRAY_MAX_AREA,
            network_array::NETWORK_ARRAY_EYE_FLAG_MAX_DISTANCE,
        )?;
        let network = ActiveNetworkArray {
            id: eye.id,
            owner: eye.owner,
            owner_player_id: eye.owner_player_id.clone(),
            eye_instance_id: eye.id,
            eye_pos: eye.pos,
            flag_instance_ids: geometry.flag_instance_ids,
            flag_positions: geometry.flag_positions,
            hull: geometry.hull,
            area: geometry.area,
            formed_at_tick,
        };
        for flag_id in &network.flag_instance_ids {
            self.flag_to_network.insert(*flag_id, network.id);
        }
        self.eye_to_network.insert(eye.id, network.id);
        self.active.insert(network.id, network.clone());
        Some(network)
    }

    fn active_networks(&self) -> impl Iterator<Item = &ActiveNetworkArray> {
        self.active.values()
    }

    fn remove_instance(&mut self, instance_id: u64) {
        self.flags.remove(&instance_id);
        self.eyes.remove(&instance_id);
        if let Some(network_id) = self
            .flag_to_network
            .remove(&instance_id)
            .or_else(|| self.eye_to_network.remove(&instance_id))
        {
            self.dissolve(network_id);
        }
    }

    fn dissolve(&mut self, network_id: u64) {
        let Some(network) = self.active.remove(&network_id) else {
            return;
        };
        self.eye_to_network.remove(&network.eye_instance_id);
        for flag_id in &network.flag_instance_ids {
            self.flag_to_network.remove(flag_id);
        }
        self.dissolved.push(network);
    }

    fn drain_dissolved(&mut self) -> Vec<ActiveNetworkArray> {
        std::mem::take(&mut self.dissolved)
    }
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
        if instance.kind == ZhenfaKind::SlowTrap {
            self.slow_charges_remaining
                .insert(id, trap_content::SLOW_TRAP_MAX_CHARGES);
        }
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
        self.slow_charges_remaining.remove(&id);
        self.slow_inside.retain(|(array_id, _)| *array_id != id);
        self.network_arrays.remove_instance(id);
        self.network_inside.retain(|(array_id, _)| *array_id != id);
        Some(removed)
    }

    fn sealed_qi_in_chunk(&self, pos: [i32; 3]) -> f64 {
        let chunk = trap_content::chunk_coord(pos);
        self.instances
            .values()
            .filter(|instance| trap_content::chunk_coord(instance.pos) == chunk)
            .map(|instance| instance.qi_invest_amount.max(0.0))
            .sum()
    }

    fn drain_slow_charge(&mut self, id: u64) -> bool {
        let remaining = self
            .slow_charges_remaining
            .entry(id)
            .or_insert(trap_content::SLOW_TRAP_MAX_CHARGES);
        *remaining = remaining.saturating_sub(1);
        *remaining == 0
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
                kind: instance.kind,
                owner: instance.owner,
                owner_player_id: instance.owner_player_id.clone(),
                pos: instance.pos,
                triggered_at_tick: tick,
                qi_invest_ratio: instance.qi_invest_ratio,
                qi_invest_amount: instance.qi_invest_amount,
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

    fn mark_network_node(&mut self, id: u64, role: NetworkArrayPlaceItem) {
        match role {
            NetworkArrayPlaceItem::Flag => self.network_arrays.mark_flag(id),
            NetworkArrayPlaceItem::Eye => self.network_arrays.mark_eye(id),
        }
    }

    fn try_form_network_array(
        &mut self,
        eye_id: u64,
        formed_at_tick: u64,
    ) -> Option<ActiveNetworkArray> {
        let eye = self.instances.get(&eye_id)?.clone();
        self.network_arrays
            .try_form_network(&eye, &self.instances, formed_at_tick)
    }

    fn active_network_arrays(&self) -> impl Iterator<Item = &ActiveNetworkArray> {
        self.network_arrays.active_networks()
    }

    fn drain_network_dissolutions(&mut self) -> Vec<ActiveNetworkArray> {
        let dissolved = self.network_arrays.drain_dissolved();
        for network in &dissolved {
            self.network_inside
                .retain(|(array_id, _)| *array_id != network.id);
            self.ward_alert_seen
                .retain(|(array_id, _), _| *array_id != network.id);
        }
        dissolved
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

fn apply_lingju_effect(
    instance: &ZhenfaInstance,
    registry: &mut ZhenfaRegistry,
    plot_env_writer: &mut Query<&mut LingtianPlot>,
) {
    apply_plot_cap_source(
        PlotCapSource::Lingju(instance.id),
        QI_LINGJU_ARRAY_CAP_BONUS,
        |pos| lingju_covers_plot(instance, pos),
        registry,
        plot_env_writer.iter_mut(),
    );
}

fn clear_lingju_effect(
    instance: &ZhenfaInstance,
    registry: &mut ZhenfaRegistry,
    plot_env_writer: &mut Query<&mut LingtianPlot>,
) {
    clear_lingju_effect_for_plots(instance, registry, plot_env_writer.iter_mut());
}

fn clear_lingju_effect_for_plots<'a>(
    instance: &ZhenfaInstance,
    registry: &mut ZhenfaRegistry,
    plots: impl Iterator<Item = Mut<'a, LingtianPlot>>,
) {
    clear_plot_cap_source(PlotCapSource::Lingju(instance.id), registry, plots);
}

fn lingju_covers_plot(instance: &ZhenfaInstance, pos: BlockPos) -> bool {
    in_horizontal_radius(
        DVec3::new(
            f64::from(pos.x) + 0.5,
            f64::from(pos.y),
            f64::from(pos.z) + 0.5,
        ),
        instance.pos,
        instance.effect_radius,
    )
}

fn apply_network_array_effect(
    network: &ActiveNetworkArray,
    registry: &mut ZhenfaRegistry,
    plot_env_writer: &mut Query<&mut LingtianPlot>,
) {
    apply_plot_cap_source(
        PlotCapSource::NetworkArray(network.id),
        QI_NETWORK_ARRAY_LINGJU_CAP_BONUS,
        |pos| network_array_covers_plot(network, pos),
        registry,
        plot_env_writer.iter_mut(),
    );
}

fn clear_network_array_effect(
    network: &ActiveNetworkArray,
    registry: &mut ZhenfaRegistry,
    plot_env_writer: &mut Query<&mut LingtianPlot>,
) {
    clear_plot_cap_source(
        PlotCapSource::NetworkArray(network.id),
        registry,
        plot_env_writer.iter_mut(),
    );
}

fn apply_plot_cap_source<'a>(
    source: PlotCapSource,
    bonus: f32,
    covers: impl Fn(BlockPos) -> bool,
    registry: &mut ZhenfaRegistry,
    plots: impl Iterator<Item = Mut<'a, LingtianPlot>>,
) {
    for mut plot in plots {
        if !covers(plot.pos) {
            continue;
        }
        {
            let sources = registry.plot_cap_sources.entry(plot.pos).or_default();
            if sources.is_empty() {
                registry
                    .plot_cap_base_caps
                    .entry(plot.pos)
                    .or_insert(plot.plot_qi_cap);
            }
            sources.insert(source, bonus);
        }
        recompute_plot_cap(&mut plot, registry);
    }
}

fn clear_plot_cap_source<'a>(
    source: PlotCapSource,
    registry: &mut ZhenfaRegistry,
    plots: impl Iterator<Item = Mut<'a, LingtianPlot>>,
) {
    let mut touched = Vec::new();
    for (pos, sources) in &mut registry.plot_cap_sources {
        if sources.remove(&source).is_some() {
            touched.push(*pos);
        }
    }
    for pos in touched {
        if registry
            .plot_cap_sources
            .get(&pos)
            .is_some_and(|sources| sources.is_empty())
        {
            registry.plot_cap_sources.remove(&pos);
        }
    }

    for mut plot in plots {
        if !registry.plot_cap_sources.contains_key(&plot.pos)
            && !registry.plot_cap_base_caps.contains_key(&plot.pos)
        {
            continue;
        }
        recompute_plot_cap(&mut plot, registry);
    }
}

fn recompute_plot_cap(plot: &mut LingtianPlot, registry: &mut ZhenfaRegistry) {
    let Some(base_cap) = registry.plot_cap_base_caps.get(&plot.pos).copied() else {
        return;
    };
    let Some(sources) = registry.plot_cap_sources.get(&plot.pos) else {
        plot.plot_qi_cap = base_cap;
        plot.plot_qi = plot.plot_qi.min(plot.plot_qi_cap);
        registry.plot_cap_base_caps.remove(&plot.pos);
        return;
    };
    if sources.is_empty() {
        plot.plot_qi_cap = base_cap;
        plot.plot_qi = plot.plot_qi.min(plot.plot_qi_cap);
        registry.plot_cap_base_caps.remove(&plot.pos);
        return;
    }
    let bonus = sources
        .values()
        .copied()
        .fold(0.0_f32, |max_bonus, source_bonus| {
            max_bonus.max(source_bonus)
        });
    plot.plot_qi_cap = (base_cap + bonus).min(PLOT_QI_CAP_MAX);
    plot.plot_qi = plot.plot_qi.min(plot.plot_qi_cap);
}

fn network_array_covers_plot(network: &ActiveNetworkArray, pos: BlockPos) -> bool {
    network_array::point_inside_hull_xz([pos.x, pos.y, pos.z], &network.hull)
        && (pos.y - network.eye_pos[1]).abs() <= 3
}

fn network_array_covers_position(network: &ActiveNetworkArray, position: DVec3) -> bool {
    network_array::point_inside_hull_xz_f64(position.x, position.z, &network.hull)
        && (position.y - f64::from(network.eye_pos[1])).abs() <= 3.0
}

fn network_warning_tick(
    network: &ActiveNetworkArray,
    now: u64,
    registry: &mut ZhenfaRegistry,
    ward_positions: &Query<(Entity, &Position), Without<ZhenfaAnchor>>,
    current_network_inside: &mut HashSet<(u64, Entity)>,
    network_alerts: &mut Vec<(u64, Entity, Entity, String, [i32; 3])>,
) {
    for (target, position) in ward_positions.iter() {
        if target == network.owner {
            continue;
        }
        if !network_array_covers_position(network, position.get()) {
            continue;
        }
        let key = (network.id, target);
        current_network_inside.insert(key);
        if registry.network_inside.contains(&key) {
            continue;
        }
        let last = registry.ward_alert_seen.get(&key).copied();
        if last.is_none_or(|tick| now.saturating_sub(tick) >= WARD_ALERT_THROTTLE_TICKS) {
            network_alerts.push((
                network.id,
                target,
                network.owner,
                network.owner_player_id.clone(),
                network.eye_pos,
            ));
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
    pub reveal_chance: f64,
    pub reflect_ratio: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ZhenfaMaterialCost {
    spiritwood_template_id: &'static str,
    spiritwood_count: u32,
    beast_bone_template_id: &'static str,
    beast_bone_count: u32,
    bone_coin_count: u64,
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
            reveal_chance: 0.0,
            reflect_ratio: 0.0,
        },
        ZhenfaKind::WarningTrap => ZhenfaKindProfile {
            min_invest_ratio: 0.0,
            cap_invest_ratio: 0.02,
            cast_time_ticks: cast_time_between(1, 1, mastery_ratio),
            duration_ticks: trap_content::survival_ticks(trap_content::OrdinaryTrapKind::Warning),
            radius: 2,
            density_multiplier: 1.0,
            tiandao_gaze_weight: 0.0,
            reveal_threshold: trap_content::discovery_profile(
                trap_content::OrdinaryTrapKind::Warning,
            )
            .reveal_threshold(),
            reveal_chance: 0.0,
            reflect_ratio: 0.0,
        },
        ZhenfaKind::BlastTrap => ZhenfaKindProfile {
            min_invest_ratio: 0.15,
            cap_invest_ratio: 0.30,
            cast_time_ticks: cast_time_between(1, 1, mastery_ratio),
            duration_ticks: trap_content::survival_ticks(trap_content::OrdinaryTrapKind::Blast),
            radius: 2,
            density_multiplier: 1.0,
            tiandao_gaze_weight: 0.0,
            reveal_threshold: trap_content::discovery_profile(
                trap_content::OrdinaryTrapKind::Blast,
            )
            .reveal_threshold(),
            reveal_chance: 0.0,
            reflect_ratio: 0.0,
        },
        ZhenfaKind::SlowTrap => ZhenfaKindProfile {
            min_invest_ratio: 0.0,
            cap_invest_ratio: 0.08,
            cast_time_ticks: cast_time_between(1, 1, mastery_ratio),
            duration_ticks: trap_content::survival_ticks(trap_content::OrdinaryTrapKind::Slow),
            radius: 2,
            density_multiplier: 1.0,
            tiandao_gaze_weight: 0.0,
            reveal_threshold: trap_content::discovery_profile(trap_content::OrdinaryTrapKind::Slow)
                .reveal_threshold(),
            reveal_chance: 0.0,
            reflect_ratio: 0.0,
        },
        ZhenfaKind::BeastTrap => ZhenfaKindProfile {
            min_invest_ratio: 0.0,
            cap_invest_ratio: 0.0,
            cast_time_ticks: cast_time_between(1, 1, mastery_ratio),
            duration_ticks: trap_content::survival_ticks(trap_content::OrdinaryTrapKind::Beast),
            radius: trap_content::OrdinaryTrapKind::Beast
                .detection_radius()
                .ceil() as u8,
            density_multiplier: 1.0,
            tiandao_gaze_weight: 0.0,
            reveal_threshold: trap_content::discovery_profile(
                trap_content::OrdinaryTrapKind::Beast,
            )
            .reveal_threshold(),
            reveal_chance: 0.0,
            reflect_ratio: 0.0,
        },
        ZhenfaKind::TripWire => ZhenfaKindProfile {
            min_invest_ratio: 0.0,
            cap_invest_ratio: 0.0,
            cast_time_ticks: cast_time_between(1, 1, mastery_ratio),
            duration_ticks: trap_content::survival_ticks(trap_content::OrdinaryTrapKind::TripWire),
            radius: trap_content::OrdinaryTrapKind::TripWire
                .detection_radius()
                .ceil() as u8,
            density_multiplier: 1.0,
            tiandao_gaze_weight: 0.0,
            reveal_threshold: trap_content::discovery_profile(
                trap_content::OrdinaryTrapKind::TripWire,
            )
            .reveal_threshold(),
            reveal_chance: 0.0,
            reflect_ratio: 0.0,
        },
        ZhenfaKind::DecoyStake => ZhenfaKindProfile {
            min_invest_ratio: 0.0,
            cap_invest_ratio: 0.0,
            cast_time_ticks: cast_time_between(1, 1, mastery_ratio),
            duration_ticks: trap_content::survival_ticks(trap_content::OrdinaryTrapKind::Decoy),
            radius: trap_content::OrdinaryTrapKind::Decoy
                .detection_radius()
                .ceil() as u8,
            density_multiplier: 1.0,
            tiandao_gaze_weight: 0.0,
            reveal_threshold: trap_content::discovery_profile(
                trap_content::OrdinaryTrapKind::Decoy,
            )
            .reveal_threshold(),
            reveal_chance: 0.0,
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
            reveal_chance: 0.0,
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
                reveal_chance: 0.0,
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
                reveal_chance: 0.0,
                reflect_ratio: 0.0,
            }
        }
        ZhenfaKind::DeceiveHeaven => ZhenfaKindProfile {
            min_invest_ratio: 0.80,
            cap_invest_ratio: 1.0,
            cast_time_ticks: cast_time_between(300, 120, mastery_ratio),
            duration_ticks: DECEIVE_HEAVEN_DURATION_TICKS,
            radius: if realm == Realm::Void { 24 } else { 16 },
            density_multiplier: 0.25,
            tiandao_gaze_weight: 1.5,
            reveal_threshold: 50.0,
            reveal_chance: deceive_heaven_reveal_chance(realm),
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
            reveal_chance: 0.0,
            reflect_ratio: 0.0,
        },
        ZhenfaKind::NetworkArray => ZhenfaKindProfile {
            min_invest_ratio: 0.0,
            cap_invest_ratio: 0.0,
            cast_time_ticks: cast_time_between(1, 1, mastery_ratio),
            duration_ticks: duration_with_mastery(6 * 60 * 60 * TICKS_PER_SECOND, mastery_ratio),
            radius: network_array::NETWORK_ARRAY_EYE_FLAG_MAX_DISTANCE as u8,
            density_multiplier: 1.0,
            tiandao_gaze_weight: 0.5,
            reveal_threshold: 30.0,
            reveal_chance: 0.0,
            reflect_ratio: 0.0,
        },
    }
}

fn zhenfa_material_cost(kind: ZhenfaKind) -> Option<ZhenfaMaterialCost> {
    (kind == ZhenfaKind::DeceiveHeaven).then_some(ZhenfaMaterialCost {
        spiritwood_template_id: DECEIVE_HEAVEN_SPIRITWOOD_ITEM_ID,
        spiritwood_count: DECEIVE_HEAVEN_SPIRITWOOD_COST,
        beast_bone_template_id: DECEIVE_HEAVEN_BEAST_BONE_ITEM_ID,
        beast_bone_count: DECEIVE_HEAVEN_BEAST_BONE_COST,
        bone_coin_count: DECEIVE_HEAVEN_BONE_COIN_COST,
    })
}

fn validate_zhenfa_material_cost(
    inventory: Option<&PlayerInventory>,
    cost: ZhenfaMaterialCost,
) -> Result<(), String> {
    let Some(inventory) = inventory else {
        return Err("inventory missing".to_string());
    };
    if inventory.bone_coins < cost.bone_coin_count {
        return Err(format!(
            "bone_coins {} < required {}",
            inventory.bone_coins, cost.bone_coin_count
        ));
    }
    let spiritwood = inventory_template_count(inventory, cost.spiritwood_template_id);
    if spiritwood < cost.spiritwood_count {
        return Err(format!(
            "{} count {} < required {}",
            cost.spiritwood_template_id, spiritwood, cost.spiritwood_count
        ));
    }
    let beast_bone = inventory_template_count(inventory, cost.beast_bone_template_id);
    if beast_bone < cost.beast_bone_count {
        return Err(format!(
            "{} count {} < required {}",
            cost.beast_bone_template_id, beast_bone, cost.beast_bone_count
        ));
    }
    Ok(())
}

fn consume_zhenfa_material_cost(
    inventory: Option<&mut PlayerInventory>,
    cost: ZhenfaMaterialCost,
) -> Result<(), String> {
    let Some(inventory) = inventory else {
        return Err("inventory missing".to_string());
    };
    validate_zhenfa_material_cost(Some(inventory), cost)?;
    for _ in 0..cost.spiritwood_count {
        let instance_id =
            find_inventory_instance_by_template(inventory, cost.spiritwood_template_id)
                .ok_or_else(|| format!("{} missing", cost.spiritwood_template_id))?;
        consume_item_instance_once(inventory, instance_id)?;
    }
    for _ in 0..cost.beast_bone_count {
        let instance_id =
            find_inventory_instance_by_template(inventory, cost.beast_bone_template_id)
                .ok_or_else(|| format!("{} missing", cost.beast_bone_template_id))?;
        consume_item_instance_once(inventory, instance_id)?;
    }
    inventory.bone_coins = inventory.bone_coins.saturating_sub(cost.bone_coin_count);
    Ok(())
}

fn inventory_template_count(inventory: &PlayerInventory, template_id: &str) -> u32 {
    inventory
        .containers
        .iter()
        .flat_map(|container| container.items.iter())
        .filter(|placed| placed.instance.template_id == template_id)
        .map(|placed| placed.instance.stack_count)
        .chain(
            inventory
                .hotbar
                .iter()
                .flatten()
                .filter(|item| item.template_id == template_id)
                .map(|item| item.stack_count),
        )
        .sum()
}

fn find_inventory_instance_by_template(
    inventory: &PlayerInventory,
    template_id: &str,
) -> Option<u64> {
    inventory
        .containers
        .iter()
        .flat_map(|container| container.items.iter())
        .find(|placed| placed.instance.template_id == template_id)
        .map(|placed| placed.instance.instance_id)
        .or_else(|| {
            inventory
                .hotbar
                .iter()
                .flatten()
                .find(|item| item.template_id == template_id)
                .map(|item| item.instance_id)
        })
}

pub fn zhenfa_meridian_dependencies(kind: ZhenfaKind) -> &'static [MeridianId] {
    match kind {
        ZhenfaKind::Trap
        | ZhenfaKind::Ward
        | ZhenfaKind::WarningTrap
        | ZhenfaKind::BlastTrap
        | ZhenfaKind::SlowTrap
        | ZhenfaKind::BeastTrap
        | ZhenfaKind::TripWire
        | ZhenfaKind::DecoyStake => &[MeridianId::Ren],
        ZhenfaKind::ShrineWard => &[MeridianId::Ren, MeridianId::Du],
        ZhenfaKind::Lingju => &[MeridianId::Ren, MeridianId::Du, MeridianId::Kidney],
        ZhenfaKind::DeceiveHeaven => &[
            MeridianId::Ren,
            MeridianId::Du,
            MeridianId::Kidney,
            MeridianId::Heart,
        ],
        ZhenfaKind::Illusion => &[MeridianId::Kidney],
        ZhenfaKind::NetworkArray => &[MeridianId::Ren, MeridianId::Du],
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
    zones: Option<Res<ZoneRegistry>>,
    mut ward_events: EventWriter<WardArrayDeployEvent>,
    mut ling_events: EventWriter<LingArrayDeployEvent>,
    mut deceive_events: EventWriter<DeceiveHeavenEvent>,
    mut illusion_events: EventWriter<IllusionArrayDeployEvent>,
    mut network_events: EventWriter<NetworkArrayDeployEvent>,
    mut qi_transfers: EventWriter<QiTransfer>,
    mut pending_narrations: Option<ResMut<PendingGameplayNarrations>>,
    mut vfx_events: Option<ResMut<Events<VfxEventRequest>>>,
) {
    for req in requests.read() {
        if registry.find_at(req.pos).is_some() {
            tracing::warn!(
                "[bong][zhenfa] place rejected: pos={:?} already has an array eye",
                req.pos
            );
            continue;
        }

        let Ok((
            username,
            unique_id,
            mut cultivation,
            qi_color,
            modifiers,
            mut inventory,
            severed,
            mastery,
        )) = players.get_mut(req.player)
        else {
            tracing::warn!(
                "[bong][zhenfa] place rejected: player {:?} missing cultivation bundle",
                req.player
            );
            continue;
        };
        let ordinary_trap = trap_content::OrdinaryTrapKind::from_zhenfa_kind(req.kind);
        let network_item = match validate_network_array_place_item(
            req.kind,
            inventory.as_deref(),
            req.item_instance_id,
        ) {
            Ok(item) => item,
            Err(error) => {
                tracing::warn!(
                    "[bong][zhenfa] network array place rejected: player {:?} {error}",
                    req.player
                );
                continue;
            }
        };
        if ordinary_trap.is_none()
            && req.kind != ZhenfaKind::NetworkArray
            && !has_zhenfa_flag(inventory.as_deref())
        {
            tracing::warn!(
                "[bong][zhenfa] place rejected: player {:?} has no array flag",
                req.player
            );
            continue;
        }
        if let Some(trap_kind) = ordinary_trap {
            let Some(face) = req.target_face else {
                tracing::warn!(
                    "[bong][zhenfa] ordinary trap place rejected: missing target_face for {:?}",
                    req.kind
                );
                continue;
            };
            if !trap_content::placement_allowed(trap_kind, face) {
                tracing::warn!(
                    "[bong][zhenfa] ordinary trap place rejected: {:?} cannot attach to {:?}",
                    req.kind,
                    face
                );
                continue;
            }
            let Some(item_instance_id) = req.item_instance_id else {
                tracing::warn!(
                    "[bong][zhenfa] ordinary trap place rejected: missing item_instance_id for {:?}",
                    req.kind
                );
                continue;
            };
            let Some(inventory_ref) = inventory.as_deref() else {
                tracing::warn!(
                    "[bong][zhenfa] ordinary trap place rejected: player {:?} has no inventory",
                    req.player
                );
                continue;
            };
            let Some(item) = inventory_item_by_instance_borrow(inventory_ref, item_instance_id)
            else {
                tracing::warn!(
                    "[bong][zhenfa] ordinary trap place rejected: missing item instance {}",
                    item_instance_id
                );
                continue;
            };
            if item.template_id != trap_kind.expected_item_id() {
                tracing::warn!(
                    "[bong][zhenfa] ordinary trap place rejected: item {} does not match {:?}",
                    item.template_id,
                    req.kind
                );
                continue;
            }
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
        if let Some(cost) = zhenfa_material_cost(req.kind) {
            if let Err(error) = validate_zhenfa_material_cost(inventory.as_deref(), cost) {
                tracing::warn!(
                    "[bong][zhenfa] place rejected: {:?} material cost unmet: {error}",
                    req.kind
                );
                continue;
            }
        }

        let mastery_at_cast = mastery
            .as_deref()
            .map(|m| m.value(req.kind))
            .unwrap_or_default();
        let profile =
            zhenfa_kind_profile(req.kind, cultivation.realm, mastery_at_cast, req.carrier);
        let (invest_ratio, qi_cost, effect_radius) = if let Some(trap_kind) = ordinary_trap {
            let cost =
                trap_content::resolve_qi_cost(trap_kind, cultivation.qi_max, req.qi_invest_ratio);
            (
                cost.ratio_of_max,
                cost.sealed_qi,
                trap_kind.detection_radius().ceil() as u8,
            )
        } else {
            let invest_ratio = sanitize_invest_ratio(
                req.qi_invest_ratio,
                profile.min_invest_ratio,
                profile.cap_invest_ratio,
            );
            let effect_radius = if matches!(req.kind, ZhenfaKind::Lingju | ZhenfaKind::NetworkArray)
            {
                profile.radius
            } else {
                trap_effect_radius(invest_ratio)
            };
            (
                invest_ratio,
                cultivation.qi_max.max(1.0) * invest_ratio,
                effect_radius,
            )
        };
        if cultivation.qi_current + f64::EPSILON < qi_cost {
            tracing::warn!(
                "[bong][zhenfa] place rejected: player {:?} qi_current {:.3} < cost {:.3}",
                req.player,
                cultivation.qi_current,
                qi_cost
            );
            continue;
        }
        if ordinary_trap.is_some()
            && trap_content::chunk_density_exceeded(registry.sealed_qi_in_chunk(req.pos) + qi_cost)
        {
            tracing::warn!(
                "[bong][zhenfa] ordinary trap place rejected: chunk density would exceed threshold pos={:?} cost={:.3}",
                req.pos,
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
        let base_duration_ticks = if let Some(trap_kind) = ordinary_trap {
            let zone_qi = zone_qi_at_pos(zones.as_deref(), req.pos).unwrap_or(0.2);
            trap_content::survival_ticks_with_environment(trap_kind, zone_qi)
        } else {
            profile.duration_ticks
        };
        let duration_ticks =
            zhenfa_instance_duration_ticks(req.kind, base_duration_ticks, qi_color, specialist);
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
            effect_radius,
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
                if let Some(cost) = zhenfa_material_cost(req.kind) {
                    if let Err(error) = consume_zhenfa_material_cost(inventory.as_deref_mut(), cost)
                    {
                        registry.remove(id);
                        remove_zhenfa_anchor_block(&mut layers, req.pos);
                        commands.entity(anchor_entity).despawn();
                        tracing::warn!(
                            "[bong][zhenfa] place rolled back: {:?} material consume failed: {error}",
                            req.kind
                        );
                        continue;
                    }
                }
                if let Some(item_instance_id) = req.item_instance_id {
                    if ordinary_trap.is_some() || network_item.is_some() {
                        let consume_result = inventory
                            .as_deref_mut()
                            .ok_or_else(|| "inventory missing".to_string())
                            .and_then(|inventory| {
                                consume_item_instance_once(inventory, item_instance_id)
                            });
                        if let Err(error) = consume_result {
                            registry.remove(id);
                            remove_zhenfa_anchor_block(&mut layers, req.pos);
                            commands.entity(anchor_entity).despawn();
                            tracing::warn!(
                                "[bong][zhenfa] ordinary trap place rolled back: item consume failed: {error}"
                            );
                            continue;
                        }
                    }
                }
                cultivation.qi_current = (cultivation.qi_current - qi_cost).max(0.0);
                if should_release_sealed_qi_to_zone(req.kind) {
                    emit_zhenfa_seal_qi_transfer(
                        &mut qi_transfers,
                        owner_player_id.as_str(),
                        id,
                        qi_cost,
                    );
                }
                if req.kind == ZhenfaKind::DecoyStake {
                    commands.entity(anchor_entity).insert((
                        ZhenfaAnchor { id },
                        DecoyTarget(req.player),
                        BaitDurability::new(BAIT_STAKE_DURABILITY_HITS),
                    ));
                } else {
                    commands.entity(anchor_entity).insert(ZhenfaAnchor { id });
                }
                if let Some(mut mastery) = mastery {
                    mastery.add_cast(req.kind);
                }
                // plan-skill-av-relink-v1 P1 — 落阵成功 → rune_draw 画符动画。内联于
                // 成功分支（覆盖全部 ZhenfaKind——deploy 事件仅覆盖 4 kind + 组网成型，
                // 普通陷阱无事件，走 adapter 会漏）；失败/回滚路径不发。
                if let Some(events) = vfx_events.as_deref_mut() {
                    events.send(VfxEventRequest::new(
                        gameplay_vfx::block_center(req.pos),
                        crate::schema::vfx_event::VfxEventPayloadV1::PlayAnim {
                            target_player: unique_id.0.to_string(),
                            anim_id: crate::network::vfx_animation_trigger::ANIM_RUNE_DRAW
                                .to_string(),
                            priority: crate::network::vfx_animation_trigger::COMBAT_PRIORITY,
                            fade_in_ticks: Some(2),
                        },
                    ));
                }
                emit_deploy_event(
                    req.kind,
                    req.player,
                    owner_player_id.clone(),
                    id,
                    req.pos,
                    &profile,
                    req.requested_at_tick,
                    &mut ward_events,
                    &mut ling_events,
                    &mut deceive_events,
                    &mut illusion_events,
                    &mut network_events,
                );
                emit_lingju_activate_feedback(
                    req.kind,
                    req.pos,
                    zones.as_deref(),
                    pending_narrations.as_deref_mut(),
                    vfx_events.as_deref_mut(),
                );
                if let Some(network_item) = network_item {
                    match network_item {
                        NetworkArrayPlaceItem::Flag => {
                            registry.mark_network_node(id, NetworkArrayPlaceItem::Flag);
                        }
                        NetworkArrayPlaceItem::Eye => {
                            registry.mark_network_node(id, NetworkArrayPlaceItem::Eye);
                            if let Some(network) =
                                registry.try_form_network_array(id, req.requested_at_tick)
                            {
                                emit_network_array_deploy_event(
                                    &network,
                                    &profile,
                                    &mut network_events,
                                );
                                emit_network_array_form_feedback(
                                    &network,
                                    zones.as_deref(),
                                    pending_narrations.as_deref_mut(),
                                    vfx_events.as_deref_mut(),
                                );
                            }
                        }
                    }
                }
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
    mut practice_logs: Query<(&mut PracticeLog, Option<&QiColor>)>,
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
            &mut layers,
            &mut practice_logs,
            &mut combat_events,
            &mut death_events,
            &mut status_effects,
            &mut sense_pulses,
            vfx_events.as_deref_mut(),
        );
    }
}

fn handle_decoy_stake_attack_intents(
    mut intents: EventReader<AttackIntent>,
    mut registry: ResMut<ZhenfaRegistry>,
    mut commands: Commands,
    mut layers: Query<&mut ChunkLayer, With<OverworldLayer>>,
    mut decoys: Query<(Entity, &ZhenfaAnchor, &Position, &mut BaitDurability), With<DecoyTarget>>,
    mut vfx_events: Option<ResMut<Events<VfxEventRequest>>>,
) {
    let mut destroyed = HashSet::new();
    for intent in intents.read() {
        let Some(target) = intent.target else {
            continue;
        };
        if destroyed.contains(&target) {
            continue;
        }
        let Ok((entity, anchor, position, mut durability)) = decoys.get_mut(target) else {
            continue;
        };
        if anchor.id == 0 {
            continue;
        }

        durability.remaining_hits = durability.remaining_hits.saturating_sub(1);
        if durability.remaining_hits > 0 {
            continue;
        }

        let fallback_pos = player_block_pos(position.get());
        let pos = registry
            .remove(anchor.id)
            .map(|instance| instance.pos)
            .unwrap_or(fallback_pos);
        remove_zhenfa_anchor_block(&mut layers, pos);
        commands.entity(entity).despawn();
        destroyed.insert(target);
        emit_zhenfa_vfx(
            vfx_events.as_deref_mut(),
            gameplay_vfx::DECOY_BREAK,
            pos,
            "#C8B878",
            0.80,
            10,
            18,
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
    _network_events: &mut EventWriter<NetworkArrayDeployEvent>,
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
                reveal_chance: profile.reveal_chance,
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
        ZhenfaKind::Trap
        | ZhenfaKind::Ward
        | ZhenfaKind::WarningTrap
        | ZhenfaKind::BlastTrap
        | ZhenfaKind::SlowTrap
        | ZhenfaKind::BeastTrap
        | ZhenfaKind::TripWire
        | ZhenfaKind::DecoyStake
        | ZhenfaKind::NetworkArray => {}
    }
}

fn emit_network_array_deploy_event(
    network: &ActiveNetworkArray,
    profile: &ZhenfaKindProfile,
    network_events: &mut EventWriter<NetworkArrayDeployEvent>,
) {
    network_events.send(NetworkArrayDeployEvent {
        owner: network.owner,
        owner_player_id: network.owner_player_id.clone(),
        array_id: network.id,
        pos: network.eye_pos,
        radius: profile.radius,
        density_multiplier: profile.density_multiplier,
        tiandao_gaze_weight: profile.tiandao_gaze_weight,
        placed_at_tick: network.formed_at_tick,
    });
}

fn emit_lingju_activate_feedback(
    kind: ZhenfaKind,
    pos: [i32; 3],
    zones: Option<&ZoneRegistry>,
    pending_narrations: Option<&mut PendingGameplayNarrations>,
    vfx_events: Option<&mut Events<VfxEventRequest>>,
) {
    if kind != ZhenfaKind::Lingju {
        return;
    }
    let zone_name = zone_name_at_pos(zones, pos);
    if let Some(pending_narrations) = pending_narrations {
        pending_narrations.push_zone(
            zone_name.as_str(),
            "此地灵气似有汇聚之势，呼吸间多了几分清润。",
            NarrationStyle::Perception,
        );
        pending_narrations.push_zone(
            zone_name.as_str(),
            "脚下方块隐隐泛起微光——聚灵阵已成。",
            NarrationStyle::Perception,
        );
        pending_narrations.push_zone(
            zone_name.as_str(),
            "又一个把家当往一处堆的。天道的眼睛，最爱这种亮堂的地方。",
            NarrationStyle::Narration,
        );
    }
    emit_zhenfa_vfx(
        vfx_events,
        gameplay_vfx::LINGJU_ACTIVATE,
        pos,
        "#7FD8A8",
        0.65,
        8,
        20,
    );
}

fn emit_network_array_form_feedback(
    network: &ActiveNetworkArray,
    zones: Option<&ZoneRegistry>,
    pending_narrations: Option<&mut PendingGameplayNarrations>,
    vfx_events: Option<&mut Events<VfxEventRequest>>,
) {
    if let Some(pending_narrations) = pending_narrations {
        pending_narrations.push_player(
            network.owner_player_id.as_str(),
            format!(
                "组网阵已成，边界 {} 旗已连通。",
                network.flag_instance_ids.len()
            ),
            NarrationStyle::Perception,
        );
        pending_narrations.push_zone(
            zone_name_at_pos(zones, network.eye_pos).as_str(),
            "旗影相连，一道无形的网在脚下铺开。",
            NarrationStyle::Narration,
        );
    }
    emit_zhenfa_vfx(
        vfx_events,
        gameplay_vfx::NETWORK_ARRAY_FORM,
        network.eye_pos,
        "#96D6EC",
        0.70,
        network.flag_instance_ids.len().max(3) as u32,
        30,
    );
}

fn emit_network_array_break_feedback(
    network: &ActiveNetworkArray,
    pending_narrations: Option<&mut PendingGameplayNarrations>,
    vfx_events: Option<&mut Events<VfxEventRequest>>,
) {
    if let Some(pending_narrations) = pending_narrations {
        pending_narrations.push_player(
            network.owner_player_id.as_str(),
            "阵破：旗眼失衡，组网阵溃散。",
            NarrationStyle::Perception,
        );
    }
    emit_zhenfa_vfx(
        vfx_events,
        gameplay_vfx::NETWORK_ARRAY_BREAK,
        network.eye_pos,
        "#D96666",
        0.65,
        network.flag_instance_ids.len().max(3) as u32,
        20,
    );
}

fn zone_name_at_pos(zones: Option<&ZoneRegistry>, pos: [i32; 3]) -> String {
    zones
        .and_then(|zones| {
            zones.find_zone(DimensionKind::Overworld, gameplay_vfx::block_center(pos))
        })
        .map(|zone| zone.name.clone())
        .unwrap_or_else(|| DEFAULT_SPAWN_ZONE_NAME.to_string())
}

fn zone_name_for_block(zones: &ZoneRegistry, pos: [i32; 3]) -> Option<String> {
    zones
        .find_zone(DimensionKind::Overworld, gameplay_vfx::block_center(pos))
        .map(|zone| zone.name.clone())
}

fn player_block_pos(position: DVec3) -> [i32; 3] {
    [
        position.x.floor() as i32,
        position.y.floor() as i32,
        position.z.floor() as i32,
    ]
}

fn mark_scatter_disturbance(
    zones: &mut ZoneRegistry,
    disturbances: &mut ScatterDisturbanceZones,
    zone_name: &str,
    now: u64,
) {
    if let Some(zone) = zones.find_zone_mut(zone_name) {
        if !zone
            .active_events
            .iter()
            .any(|event| event == SCATTER_DISTURBANCE_EVENT)
        {
            zone.active_events
                .push(SCATTER_DISTURBANCE_EVENT.to_string());
        }
        disturbances.expires_at.insert(
            zone_name.to_string(),
            now.saturating_add(SCATTER_DISTURBANCE_DURATION_TICKS),
        );
    }
}

fn tick_scatter_disturbance_zones(
    clock: Res<CombatClock>,
    mut zones: Option<ResMut<ZoneRegistry>>,
    mut disturbances: ResMut<ScatterDisturbanceZones>,
) {
    let Some(zones) = zones.as_deref_mut() else {
        return;
    };
    let now = clock.tick;
    let expired = disturbances
        .expires_at
        .iter()
        .filter_map(|(zone_name, expires_at)| (*expires_at <= now).then_some(zone_name.clone()))
        .collect::<Vec<_>>();
    for zone_name in expired {
        disturbances.expires_at.remove(zone_name.as_str());
        if let Some(zone) = zones.find_zone_mut(zone_name.as_str()) {
            zone.active_events
                .retain(|event| event != SCATTER_DISTURBANCE_EVENT);
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct ScatterReleaseOutcome {
    zone_name: String,
    accepted: f64,
    overflow: f64,
    transfer: Option<QiTransfer>,
}

#[allow(clippy::too_many_arguments)]
fn release_scatter_qi_to_zone(
    zones: &mut ZoneRegistry,
    ledger: &mut WorldQiAccount,
    mut qi_transfers: Option<&mut Events<QiTransfer>>,
    source: QiAccountId,
    source_balance_before: f64,
    pos: [i32; 3],
    amount: f64,
    overflow_key: &str,
) -> Option<ScatterReleaseOutcome> {
    if amount <= QI_EPSILON {
        return None;
    }
    let zone_name = zone_name_for_block(zones, pos)?;
    let zone = zones.find_zone_mut(zone_name.as_str())?;
    let to = QiAccountId::zone(zone.name.clone());
    let zone_current = zone.spirit_qi.max(0.0) * QI_ZONE_UNIT_CAPACITY;
    let outcome = match qi_release_to_zone(
        amount,
        source.clone(),
        to,
        zone_current,
        QI_ZONE_UNIT_CAPACITY,
    ) {
        Ok(outcome) => outcome,
        Err(error) => {
            tracing::warn!(
                ?error,
                "[bong][zhenfa] scatter bead release rejected at pos={pos:?}"
            );
            return None;
        }
    };

    if let Err(error) = ledger.set_balance(source.clone(), source_balance_before) {
        tracing::warn!(
            ?error,
            "[bong][zhenfa] scatter bead ledger source init failed at pos={pos:?}"
        );
        return None;
    }

    zone.spirit_qi = (outcome.zone_after / QI_ZONE_UNIT_CAPACITY).clamp(-1.0, 1.0);
    let mut applied_transfer = None;
    if let Some(transfer) = outcome.transfer.clone() {
        if let Err(error) = ledger.transfer(transfer.clone()) {
            tracing::warn!(
                ?error,
                "[bong][zhenfa] scatter bead ledger transfer failed at pos={pos:?}"
            );
            return None;
        }
        if let Some(events) = &mut qi_transfers {
            events.send(transfer.clone());
        }
        applied_transfer = Some(transfer);
    }
    if outcome.overflow > QI_EPSILON {
        let overflow = QiTransfer::new(
            source,
            QiAccountId::overflow(format!("qi_scatter_overflow:{overflow_key}")),
            outcome.overflow,
            QiTransferReason::ReleaseToZone,
        )
        .ok()?;
        if let Err(error) = ledger.transfer(overflow.clone()) {
            tracing::warn!(
                ?error,
                "[bong][zhenfa] scatter bead overflow ledger transfer failed at pos={pos:?}"
            );
            return None;
        }
        if let Some(events) = &mut qi_transfers {
            events.send(overflow);
        }
    }

    Some(ScatterReleaseOutcome {
        zone_name,
        accepted: outcome.accepted,
        overflow: outcome.overflow,
        transfer: applied_transfer,
    })
}

#[allow(clippy::too_many_arguments)]
fn handle_scatter_bead_use(
    mut requests: EventReader<ScatterBeadUseRequest>,
    mut players: Query<ScatterBeadUsePlayer<'_>>,
    mut zones: Option<ResMut<ZoneRegistry>>,
    mut ledger: ResMut<WorldQiAccount>,
    mut qi_transfers: Option<ResMut<Events<QiTransfer>>>,
    mut pending_narrations: Option<ResMut<PendingGameplayNarrations>>,
    mut vfx_events: Option<ResMut<Events<VfxEventRequest>>>,
    mut burials: ResMut<ScatterBeadBurials>,
    mut disturbances: ResMut<ScatterDisturbanceZones>,
) {
    for req in requests.read() {
        let Ok((username, position, mut inventory)) = players.get_mut(req.player) else {
            tracing::warn!(
                "[bong][zhenfa] scatter bead rejected: player {:?} missing bundle",
                req.player
            );
            continue;
        };
        let Some(inventory_ref) = inventory.as_deref() else {
            tracing::warn!(
                "[bong][zhenfa] scatter bead rejected: player {:?} has no inventory",
                req.player
            );
            continue;
        };
        let Some(item) = inventory_item_by_instance_borrow(inventory_ref, req.item_instance_id)
        else {
            tracing::warn!(
                "[bong][zhenfa] scatter bead rejected: missing item instance {}",
                req.item_instance_id
            );
            continue;
        };
        if item.template_id != QI_SCATTER_BEAD_ITEM_ID {
            tracing::warn!(
                "[bong][zhenfa] scatter bead rejected: item {} is not {}",
                item.template_id,
                QI_SCATTER_BEAD_ITEM_ID
            );
            continue;
        }
        let pos = req
            .bury_pos
            .unwrap_or_else(|| player_block_pos(position.get()));
        let Some(zones_ref) = zones.as_deref_mut() else {
            tracing::warn!(
                "[bong][zhenfa] scatter bead rejected: ZoneRegistry missing player={:?}",
                req.player
            );
            continue;
        };
        if zone_name_for_block(zones_ref, pos).is_none() {
            tracing::warn!("[bong][zhenfa] scatter bead rejected: no zone for pos={pos:?}");
            continue;
        }
        if let Err(error) = inventory
            .as_deref_mut()
            .ok_or_else(|| "inventory missing".to_string())
            .and_then(|inventory| consume_item_instance_once(inventory, req.item_instance_id))
        {
            tracing::warn!(
                "[bong][zhenfa] scatter bead rejected: consume failed instance={} error={error}",
                req.item_instance_id
            );
            continue;
        }
        let owner_player_id = canonical_player_id(username.0.as_str());
        if req.bury_pos.is_some() {
            let bead_id = burials.insert(
                req.player,
                owner_player_id.clone(),
                pos,
                QI_SCATTER_BEAD_CAPACITY,
                req.requested_at_tick,
            );
            let source =
                QiAccountId::container(format!("qi_scatter_buried:{owner_player_id}:{bead_id}"));
            if let Err(error) = ledger.set_balance(source, QI_SCATTER_BEAD_CAPACITY) {
                burials.beads.remove(&bead_id);
                tracing::warn!(
                    ?error,
                    "[bong][zhenfa] scatter bead burial source init failed instance={} pos={pos:?}",
                    req.item_instance_id
                );
                continue;
            }
            tracing::info!(
                "[bong][zhenfa] scatter bead buried player={:?} instance={} bead_id={} pos={pos:?}",
                req.player,
                req.item_instance_id,
                bead_id
            );
            continue;
        }
        let source = QiAccountId::container(format!(
            "qi_scatter:{owner_player_id}:{}",
            req.item_instance_id
        ));
        let Some(outcome) = release_scatter_qi_to_zone(
            zones_ref,
            &mut ledger,
            qi_transfers.as_deref_mut(),
            source,
            QI_SCATTER_BEAD_CAPACITY,
            pos,
            QI_SCATTER_BEAD_CAPACITY,
            &format!("{owner_player_id}:{}", req.item_instance_id),
        ) else {
            tracing::warn!(
                "[bong][zhenfa] scatter bead consumed but release failed instance={} pos={pos:?}",
                req.item_instance_id
            );
            continue;
        };
        mark_scatter_disturbance(
            zones_ref,
            &mut disturbances,
            outcome.zone_name.as_str(),
            req.requested_at_tick,
        );
        emit_scatter_bead_feedback(
            pos,
            outcome.zone_name.as_str(),
            pending_narrations.as_deref_mut(),
            vfx_events.as_deref_mut(),
        );
        tracing::info!(
            "[bong][zhenfa] scatter bead used player={:?} instance={} accepted={:.3} overflow={:.3}",
            req.player,
            req.item_instance_id,
            outcome.accepted,
            outcome.overflow
        );
    }
}

fn handle_scatter_bead_trigger_requests(
    mut requests: EventReader<ScatterBeadTriggerRequest>,
    mut scatter: ScatterBeadRuntime,
) {
    for req in requests.read() {
        let Some(bead) =
            scatter
                .burials
                .trigger_buried(req.bead_id, req.player, req.requested_at_tick)
        else {
            tracing::warn!(
                "[bong][zhenfa] buried scatter bead trigger rejected: player {:?} is not owner of bead {}",
                req.player,
                req.bead_id
            );
            continue;
        };
        let Some(zones_ref) = scatter.zones.as_deref_mut() else {
            scatter.burials.beads.insert(req.bead_id, bead);
            tracing::warn!(
                "[bong][zhenfa] buried scatter bead trigger rejected: ZoneRegistry missing bead={}",
                req.bead_id
            );
            continue;
        };
        let source = QiAccountId::container(format!(
            "qi_scatter_buried:{}:{}",
            bead.owner_player_id, bead.id
        ));
        let Some(outcome) = release_scatter_qi_to_zone(
            zones_ref,
            &mut scatter.ledger,
            scatter.qi_transfers.as_deref_mut(),
            source,
            bead.remaining_qi,
            bead.pos,
            bead.remaining_qi,
            &format!("buried:{}:{}", bead.owner_player_id, bead.id),
        ) else {
            scatter.burials.beads.insert(req.bead_id, bead);
            tracing::warn!(
                "[bong][zhenfa] buried scatter bead trigger release failed bead={}",
                req.bead_id
            );
            continue;
        };
        mark_scatter_disturbance(
            zones_ref,
            &mut scatter.disturbances,
            outcome.zone_name.as_str(),
            req.requested_at_tick,
        );
        emit_scatter_bead_feedback(
            bead.pos,
            outcome.zone_name.as_str(),
            scatter.pending_narrations.as_deref_mut(),
            scatter.vfx_events.as_deref_mut(),
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn tick_scatter_bead_excretion(
    clock: Res<CombatClock>,
    mut burials: ResMut<ScatterBeadBurials>,
    mut zones: Option<ResMut<ZoneRegistry>>,
    mut ledger: ResMut<WorldQiAccount>,
    mut qi_transfers: Option<ResMut<Events<QiTransfer>>>,
    mut disturbances: ResMut<ScatterDisturbanceZones>,
) {
    let Some(zones_ref) = zones.as_deref_mut() else {
        return;
    };
    let now = clock.tick;
    let ids = burials.beads.keys().copied().collect::<Vec<_>>();
    let mut depleted = Vec::new();

    for id in ids {
        let Some(bead) = burials.beads.get_mut(&id) else {
            continue;
        };
        if bead.remaining_qi <= QI_EPSILON {
            depleted.push(id);
            continue;
        }
        let elapsed_ticks = now.saturating_sub(bead.last_tick);
        if elapsed_ticks == 0 {
            continue;
        }

        let Some(zone_name) = zone_name_for_block(zones_ref, bead.pos) else {
            continue;
        };
        let Some(zone_qi) = zones_ref
            .find_zone_mut(zone_name.as_str())
            .map(|zone| zone.spirit_qi)
        else {
            continue;
        };
        let elapsed_secs = elapsed_ticks as f64 / TICKS_PER_SECOND as f64;
        let remaining_after = qi_excretion(
            bead.remaining_qi,
            ContainerKind::EmbeddedTrap,
            elapsed_secs,
            EnvField::new(zone_qi),
        );
        let leaked = (bead.remaining_qi - remaining_after).max(0.0);
        bead.last_tick = now;

        if leaked <= QI_EPSILON {
            bead.remaining_qi = remaining_after;
            continue;
        }

        let source = QiAccountId::container(format!(
            "qi_scatter_buried:{}:{}",
            bead.owner_player_id, bead.id
        ));
        let Some(outcome) = release_scatter_qi_to_zone(
            zones_ref,
            &mut ledger,
            qi_transfers.as_deref_mut(),
            source,
            bead.remaining_qi,
            bead.pos,
            leaked,
            &format!("buried:{}:{}", bead.owner_player_id, bead.id),
        ) else {
            continue;
        };
        bead.remaining_qi = remaining_after;
        mark_scatter_disturbance(
            zones_ref,
            &mut disturbances,
            outcome.zone_name.as_str(),
            now,
        );
        if bead.remaining_qi <= QI_EPSILON {
            depleted.push(id);
        }
    }

    for id in depleted {
        burials.beads.remove(&id);
    }
}

fn emit_scatter_bead_feedback(
    pos: [i32; 3],
    zone_name: &str,
    pending_narrations: Option<&mut PendingGameplayNarrations>,
    vfx_events: Option<&mut Events<VfxEventRequest>>,
) {
    if let Some(pending_narrations) = pending_narrations {
        pending_narrations.push_zone(
            zone_name,
            "珠子应声碎裂,一缕灰白真元散入空气,周遭气息变得浑浊难辨。",
            NarrationStyle::Perception,
        );
        pending_narrations.push_zone(
            zone_name,
            "你能感觉到这片地界的气机被搅了一下,像一碗清水里落了灰。",
            NarrationStyle::Perception,
        );
    }
    emit_zhenfa_vfx(
        vfx_events,
        gameplay_vfx::SCATTER_BURST,
        pos,
        "#E8F0EE",
        0.75,
        14,
        16,
    );
}

type ZhenfaDamageTarget<'a> = (
    Entity,
    &'a Position,
    &'a mut Wounds,
    Option<&'a Lifecycle>,
    Option<&'a Username>,
    Option<&'a FaunaTag>,
    Option<&'a mut Contamination>,
    Option<&'a mut MeridianSystem>,
    Option<&'a Relationships>,
    Option<&'a Renown>,
);

type ZhenfaPlacePlayer<'a> = (
    &'a Username,
    // plan-skill-av-relink-v1 P1 — 落阵成功 rune_draw 动画的 target_player uuid。
    &'a UniqueId,
    &'a mut Cultivation,
    &'a QiColor,
    Option<&'a InsightModifiers>,
    Option<&'a mut PlayerInventory>,
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

type ScatterBeadUsePlayer<'a> = (&'a Username, &'a Position, Option<&'a mut PlayerInventory>);

#[derive(SystemParam)]
struct ScatterBeadRuntime<'w> {
    zones: Option<ResMut<'w, ZoneRegistry>>,
    ledger: ResMut<'w, WorldQiAccount>,
    qi_transfers: Option<ResMut<'w, Events<QiTransfer>>>,
    burials: ResMut<'w, ScatterBeadBurials>,
    disturbances: ResMut<'w, ScatterDisturbanceZones>,
    pending_narrations: Option<ResMut<'w, PendingGameplayNarrations>>,
    vfx_events: Option<ResMut<'w, Events<VfxEventRequest>>>,
}

#[derive(SystemParam)]
struct ZhenfaTickEventWriters<'w> {
    combat_events: EventWriter<'w, CombatEvent>,
    death_events: EventWriter<'w, DeathEvent>,
    status_effects: EventWriter<'w, ApplyStatusEffectIntent>,
    sense_pulses: EventWriter<'w, ZhenfaSensePulse>,
    decay_events: EventWriter<'w, ArrayDecayEvent>,
    deceive_exposed_events: EventWriter<'w, DeceiveHeavenExposedEvent>,
    juebi_events: EventWriter<'w, JueBiTriggerEvent>,
    qi_transfers: EventWriter<'w, QiTransfer>,
}

#[allow(clippy::too_many_arguments)]
fn tick_zhenfa_registry(
    clock: Res<CombatClock>,
    mut registry: ResMut<ZhenfaRegistry>,
    mut commands: Commands,
    mut layers: Query<&mut ChunkLayer, With<OverworldLayer>>,
    mut plots: Query<&mut LingtianPlot>,
    mut targets: Query<ZhenfaDamageTarget<'_>>,
    mut practice_logs: Query<(&mut PracticeLog, Option<&QiColor>)>,
    ward_positions: Query<(Entity, &Position), Without<ZhenfaAnchor>>,
    mut events: ZhenfaTickEventWriters,
    mut zones: Option<ResMut<ZoneRegistry>>,
    mut pending_narrations: Option<ResMut<PendingGameplayNarrations>>,
    mut vfx_events: Option<ResMut<Events<VfxEventRequest>>>,
) {
    let now = clock.tick;
    let expired = registry.expire_at_or_before(now);
    if !expired.is_empty() {
        tracing::debug!("[bong][zhenfa] expired {} array eye(s)", expired.len());
    }
    for instance in &expired {
        if instance.kind == ZhenfaKind::Lingju {
            clear_lingju_effect(instance, &mut registry, &mut plots);
        }
        if should_release_sealed_qi_to_zone(instance.kind) {
            release_zhenfa_qi_to_zone(zones.as_deref_mut(), &mut events.qi_transfers, instance);
        }
        remove_zhenfa_anchor_block(&mut layers, instance.pos);
        events.decay_events.send(ArrayDecayEvent {
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
    let dissolved_networks = registry.drain_network_dissolutions();
    for network in &dissolved_networks {
        clear_network_array_effect(network, &mut registry, &mut plots);
        emit_network_array_break_feedback(
            network,
            pending_narrations.as_deref_mut(),
            vfx_events.as_deref_mut(),
        );
    }

    let mut passive_triggers = Vec::new();
    let mut blast_triggers = Vec::new();
    let mut warning_alerts = Vec::new();
    let mut beast_triggers = Vec::new();
    let mut trip_wire_alerts = Vec::new();
    let mut slow_triggers = Vec::new();
    let mut ward_alerts = Vec::new();
    let mut network_alerts = Vec::new();
    let mut deceived_exposed = Vec::new();
    let mut lingju_instances = Vec::new();
    let mut current_ward_inside = HashSet::new();
    let mut current_slow_inside = HashSet::new();
    let mut current_network_inside = HashSet::new();
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
            ZhenfaKind::WarningTrap => {
                for (target, position, ..) in &mut targets {
                    if target == instance.owner {
                        continue;
                    }
                    let pos = position.get();
                    if trap_content::vertical_column_contains(
                        pos,
                        instance.pos,
                        trap_content::OrdinaryTrapKind::Warning.detection_radius(),
                        trap_content::OrdinaryTrapKind::Warning.vertical_height(),
                    ) {
                        let key = (instance.id, target);
                        let last = registry.ward_alert_seen.get(&key).copied();
                        if last.is_none_or(|tick| {
                            now.saturating_sub(tick) >= trap_content::WARNING_TRIGGER_THROTTLE_TICKS
                        }) {
                            warning_alerts.push((
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
            ZhenfaKind::BlastTrap => {
                for (target, position, ..) in &mut targets {
                    if target == instance.owner {
                        continue;
                    }
                    let pos = position.get();
                    if !trap_content::horizontal_same_layer_contains(
                        pos,
                        instance.pos,
                        trap_content::OrdinaryTrapKind::Blast.detection_radius(),
                    ) {
                        continue;
                    }
                    if !blast_has_clear_los(&mut layers, instance.pos, pos) {
                        continue;
                    }
                    blast_triggers.push(instance.id);
                    break;
                }
            }
            ZhenfaKind::SlowTrap => {
                for (target, position, ..) in &mut targets {
                    if target == instance.owner {
                        continue;
                    }
                    let pos = position.get();
                    if trap_content::vertical_column_contains(
                        pos,
                        instance.pos,
                        trap_content::OrdinaryTrapKind::Slow.detection_radius(),
                        trap_content::OrdinaryTrapKind::Slow.vertical_height(),
                    ) {
                        let key = (instance.id, target);
                        current_slow_inside.insert(key);
                        if registry.slow_inside.contains(&key) {
                            continue;
                        }
                        slow_triggers.push((
                            instance.id,
                            target,
                            instance.owner,
                            instance.owner_player_id.clone(),
                            instance.pos,
                        ));
                    }
                }
            }
            ZhenfaKind::BeastTrap => {
                for (target, position, _wounds, _lifecycle, _username, fauna_tag, ..) in
                    &mut targets
                {
                    if !is_non_owner_beast_target(target, instance.owner, fauna_tag) {
                        continue;
                    }
                    if trap_content::vertical_column_contains(
                        position.get(),
                        instance.pos,
                        trap_content::OrdinaryTrapKind::Beast.detection_radius(),
                        trap_content::OrdinaryTrapKind::Beast.vertical_height(),
                    ) {
                        beast_triggers.push((
                            instance.id,
                            target,
                            instance.owner,
                            instance.owner_player_id.clone(),
                            instance.pos,
                        ));
                        break;
                    }
                }
            }
            ZhenfaKind::TripWire => {
                for (target, position, ..) in &mut targets {
                    if target == instance.owner {
                        continue;
                    }
                    if trap_content::vertical_column_contains(
                        position.get(),
                        instance.pos,
                        trap_content::OrdinaryTrapKind::TripWire.detection_radius(),
                        trap_content::OrdinaryTrapKind::TripWire.vertical_height(),
                    ) {
                        let key = (instance.id, target);
                        let last = registry.ward_alert_seen.get(&key).copied();
                        if last.is_none_or(|tick| {
                            now.saturating_sub(tick) >= WARD_ALERT_THROTTLE_TICKS
                        }) {
                            trip_wire_alerts.push((
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
            ZhenfaKind::DecoyStake => {
                if now.is_multiple_of(10) {
                    emit_zhenfa_vfx(
                        vfx_events.as_deref_mut(),
                        gameplay_vfx::DECOY_TAUNT,
                        instance.pos,
                        "#D86060",
                        0.45,
                        1,
                        12,
                    );
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
                    &mut events.combat_events,
                    &mut events.death_events,
                    &mut events.status_effects,
                );
            }
            ZhenfaKind::Lingju => {
                lingju_instances.push(instance.clone());
            }
            ZhenfaKind::DeceiveHeaven => {
                if deceive_heaven_detected(instance, now) {
                    deceived_exposed.push((
                        instance.id,
                        instance.owner,
                        instance.pos,
                        instance.anchor_entity,
                    ));
                }
            }
            ZhenfaKind::Illusion => {}
            ZhenfaKind::NetworkArray => {}
        }
    }
    for instance in &lingju_instances {
        apply_lingju_effect(instance, &mut registry, &mut plots);
    }
    let active_networks = registry
        .active_network_arrays()
        .cloned()
        .collect::<Vec<_>>();
    for network in &active_networks {
        apply_network_array_effect(network, &mut registry, &mut plots);
        network_warning_tick(
            network,
            now,
            &mut registry,
            &ward_positions,
            &mut current_network_inside,
            &mut network_alerts,
        );
    }
    registry
        .ward_inside
        .retain(|key| current_ward_inside.contains(key));
    registry.ward_inside.extend(current_ward_inside);

    registry
        .slow_inside
        .retain(|key| current_slow_inside.contains(key));
    registry.slow_inside.extend(current_slow_inside);

    registry
        .network_inside
        .retain(|key| current_network_inside.contains(key));
    registry.network_inside.extend(current_network_inside);

    for (id, intruder, owner, owner_player_id, pos) in ward_alerts {
        registry.ward_alert_seen.insert((id, intruder), now);
        if let Some(pending_narrations) = pending_narrations.as_deref_mut() {
            pending_narrations.push_player(
                owner_player_id.as_str(),
                "你心头一颤，布下的警戒场传回一缕陌生气机。",
                NarrationStyle::Perception,
            );
        }
        events.sense_pulses.send(ZhenfaSensePulse {
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

    for (id, intruder, owner, owner_player_id, pos) in network_alerts {
        registry.ward_alert_seen.insert((id, intruder), now);
        if let Some(pending_narrations) = pending_narrations.as_deref_mut() {
            pending_narrations.push_player(
                owner_player_id.as_str(),
                "阵内有动静——有目标闯入组网阵。",
                NarrationStyle::Perception,
            );
        }
        events.sense_pulses.send(ZhenfaSensePulse {
            owner,
            kind: SenseKindV1::ZhenfaWardAlert,
            pos,
            intensity: 0.75,
            generation: now,
        });
        emit_zhenfa_vfx(
            vfx_events.as_deref_mut(),
            gameplay_vfx::NETWORK_ARRAY_FORM,
            pos,
            "#96D6EC",
            0.45,
            6,
            20,
        );
    }

    for (id, intruder, owner, owner_player_id, pos) in warning_alerts {
        registry.ward_alert_seen.insert((id, intruder), now);
        if let Some(pending_narrations) = pending_narrations.as_deref_mut() {
            pending_narrations.push_player(
                owner_player_id.as_str(),
                "你埋下的警示阵震了一下，三格内有陌生真元靠近。",
                NarrationStyle::Perception,
            );
        }
        events.sense_pulses.send(ZhenfaSensePulse {
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
            "#66BBFF",
            0.55,
            14,
            50,
        );
    }

    for (id, intruder, owner, owner_player_id, pos) in trip_wire_alerts {
        registry.ward_alert_seen.insert((id, intruder), now);
        if let Some(pending_narrations) = pending_narrations.as_deref_mut() {
            pending_narrations.push_player(
                owner_player_id.as_str(),
                "绊线被扯动了，埋线处传回一声干涩轻响。",
                NarrationStyle::Perception,
            );
        }
        events.sense_pulses.send(ZhenfaSensePulse {
            owner,
            kind: SenseKindV1::ZhenfaWardAlert,
            pos,
            intensity: 1.0,
            generation: now,
        });
        emit_zhenfa_vfx(
            vfx_events.as_deref_mut(),
            gameplay_vfx::TRIP_WIRE_TRIGGER,
            pos,
            "#66BBFF",
            0.55,
            14,
            50,
        );
    }

    for snapshot in registry.trigger_now(beast_triggers.iter().map(|(id, ..)| *id), now) {
        remove_zhenfa_anchor_block(&mut layers, snapshot.pos);
        commands.entity(snapshot.anchor_entity).despawn();
        if let Some((_, target, owner, _owner_player_id, pos)) =
            beast_triggers.iter().find(|(id, ..)| *id == snapshot.id)
        {
            apply_beast_trap_snap(
                *target,
                *owner,
                *pos,
                now,
                &mut targets,
                &mut events.status_effects,
                &mut events.sense_pulses,
                vfx_events.as_deref_mut(),
            );
        }
    }

    for (id, intruder, owner, owner_player_id, pos) in slow_triggers {
        if registry.drain_slow_charge(id) {
            if let Some(instance) = registry.remove(id) {
                remove_zhenfa_anchor_block(&mut layers, instance.pos);
                commands.entity(instance.anchor_entity).despawn();
            }
        }
        events.status_effects.send(ApplyStatusEffectIntent {
            target: intruder,
            kind: StatusEffectKind::Slowed,
            magnitude: 0.50,
            duration_ticks: trap_content::SLOW_TRAP_EFFECT_TICKS,
            issued_at_tick: now,
        });
        events.status_effects.send(ApplyStatusEffectIntent {
            target: intruder,
            kind: StatusEffectKind::QiRegenPaused,
            magnitude: 1.0,
            duration_ticks: trap_content::SLOW_TRAP_EFFECT_TICKS,
            issued_at_tick: now,
        });
        if let Some(pending_narrations) = pending_narrations.as_deref_mut() {
            pending_narrations.push_player(
                owner_player_id.as_str(),
                "缓阵收紧了，来者的步子被拖慢了。",
                NarrationStyle::Perception,
            );
        }
        events.sense_pulses.send(ZhenfaSensePulse {
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
            "#55DDEE",
            0.60,
            18,
            60,
        );
    }

    for (id, owner, pos, anchor_entity) in deceived_exposed {
        if let Some(instance) = registry.remove(id) {
            release_zhenfa_qi_to_zone(zones.as_deref_mut(), &mut events.qi_transfers, &instance);
            events.juebi_events.send(JueBiTriggerEvent {
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
            events
                .deceive_exposed_events
                .send(DeceiveHeavenExposedEvent {
                    owner: instance.owner,
                    owner_player_id: instance.owner_player_id.clone(),
                    array_id: instance.id,
                    pos: instance.pos,
                    self_weight_multiplier: 0.5,
                    target_weight_multiplier: 1.5,
                    reveal_chance: deceive_heaven_reveal_chance(instance.realm_at_cast),
                    exposed_at_tick: now,
                });
        }
        commands.entity(anchor_entity).despawn();
        remove_zhenfa_anchor_block(&mut layers, pos);
    }

    passive_triggers.extend(blast_triggers);
    let mut snapshots = registry.trigger_now(passive_triggers, now);
    snapshots.extend(registry.drain_due_chain_triggers(now));
    for snapshot in &snapshots {
        if snapshot.kind == ZhenfaKind::BlastTrap {
            release_trap_snapshot_qi_to_zone(
                zones.as_deref_mut(),
                &mut events.qi_transfers,
                snapshot,
            );
        }
    }
    remove_zhenfa_anchor_blocks(&mut layers, snapshots.iter().map(|snapshot| snapshot.pos));
    despawn_triggered_anchors(&mut commands, &snapshots);
    apply_trigger_snapshots(
        snapshots,
        &mut targets,
        &mut layers,
        &mut practice_logs,
        &mut events.combat_events,
        &mut events.death_events,
        &mut events.status_effects,
        &mut events.sense_pulses,
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
    mut plots: Query<&mut LingtianPlot>,
    item_registry: Option<Res<ItemRegistry>>,
    mut allocator: Option<ResMut<InventoryInstanceIdAllocator>>,
    mut breakthrough_events: EventWriter<ArrayBreakthroughEvent>,
    mut pending_narrations: Option<ResMut<PendingGameplayNarrations>>,
    mut vfx_events: Option<ResMut<Events<VfxEventRequest>>>,
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
        if instance.kind == ZhenfaKind::Lingju {
            clear_lingju_effect(&instance, &mut registry, &mut plots);
        }
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
                if !is_mundane_mechanical_trap_kind(instance.kind) {
                    apply_backlash(
                        req.player,
                        &mut wounds,
                        contamination,
                        meridians,
                        req.requested_at_tick,
                        backlash_contam_delta(instance.kind),
                    );
                }
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
                            req.requested_at_tick,
                        ) {
                            tracing::warn!(
                                "[bong][zhenfa] disarm succeeded but pearl grant failed: {error}"
                            );
                        }
                    }
                }
            }
        }
        let dissolved_networks = registry.drain_network_dissolutions();
        for network in &dissolved_networks {
            clear_network_array_effect(network, &mut registry, &mut plots);
            emit_network_array_break_feedback(
                network,
                pending_narrations.as_deref_mut(),
                vfx_events.as_deref_mut(),
            );
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
        _fauna_tag,
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
            // humanoid-only boundary（P0 决议，本轮不迁移）：结界灼烧是无差别持续伤害，
            // 固定命中 Chest 代表部位；玩家恒为人形。
            location: crate::body_plan::legacy_body_part_to_id(BodyPart::Chest),
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
            physical_damage: 0.0,
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
                    LifecycleState::AwaitingRevival | LifecycleState::Terminated
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
    layers: &mut Query<&mut ChunkLayer, With<OverworldLayer>>,
    practice_logs: &mut Query<(&mut PracticeLog, Option<&QiColor>)>,
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
            if snapshot.kind == ZhenfaKind::BlastTrap {
                "#FF4422"
            } else {
                "#FF3344"
            },
            if snapshot.kind == ZhenfaKind::BlastTrap {
                (snapshot.qi_invest_amount / 30.0).clamp(0.5, 1.0) as f32
            } else {
                snapshot.qi_invest_ratio.clamp(0.3, 1.0) as f32
            },
            16,
            24,
        );

        let damage_profile = if snapshot.kind == ZhenfaKind::BlastTrap {
            DamageProfile {
                damage: trap_content::blast_damage(snapshot.qi_invest_amount),
                severity: (snapshot.qi_invest_amount / 30.0).clamp(0.35, 0.80) as f32,
                bleeding_per_sec: 0.12,
                meridian_integrity_loss: 0.06,
            }
        } else {
            damage_profile(snapshot.qi_invest_ratio)
        };
        let mut hit_any = false;
        for (
            target,
            position,
            mut wounds,
            lifecycle,
            username,
            _fauna_tag,
            contamination,
            meridians,
            _relationships,
            _renown,
        ) in targets.iter_mut()
        {
            if target == snapshot.owner {
                continue;
            }
            let target_pos = position.get();
            let in_trigger_area = if snapshot.kind == ZhenfaKind::BlastTrap {
                trap_content::horizontal_same_layer_contains(
                    target_pos,
                    snapshot.pos,
                    trap_content::OrdinaryTrapKind::Blast.detection_radius(),
                ) && blast_has_clear_los(layers, snapshot.pos, target_pos)
            } else {
                in_horizontal_radius(target_pos, snapshot.pos, snapshot.effect_radius)
            };
            if !in_trigger_area {
                continue;
            }
            hit_any = true;

            let was_alive = wounds.health_current > 0.0;
            wounds.health_current =
                (wounds.health_current - damage_profile.damage).clamp(0.0, wounds.health_max);
            let hit_parts: &[BodyPart] = if snapshot.kind == ZhenfaKind::BlastTrap {
                &[BodyPart::Chest]
            } else {
                &[BodyPart::LegL, BodyPart::LegR]
            };
            let wound_kind = if snapshot.kind == ZhenfaKind::BlastTrap {
                WoundKind::Cut
            } else {
                WoundKind::Pierce
            };
            for part in hit_parts {
                wounds.entries.push(Wound {
                    // humanoid-only boundary（P0 决议，本轮不迁移）：阵法陷阱固定命中表
                    // （BlastTrap→Chest / 其余→双腿）本就以 legacy BodyPart 枚举声明，
                    // 全双射转换，行为不变。
                    location: crate::body_plan::legacy_body_part_to_id(*part),
                    kind: wound_kind,
                    severity: damage_profile.severity,
                    bleeding_per_sec: damage_profile.bleeding_per_sec,
                    created_at_tick: tick,
                    inflicted_by: Some(format!("{:?}:{}", snapshot.kind, snapshot.id)),
                });
            }

            if let Some(mut meridians) = meridians {
                for id in [MeridianId::Bladder, MeridianId::Kidney] {
                    let meridian = meridians.get_mut(id);
                    meridian.integrity =
                        (meridian.integrity - damage_profile.meridian_integrity_loss).max(0.0);
                }
            }

            let contam_delta = if snapshot.kind == ZhenfaKind::BlastTrap {
                0.10
            } else {
                trap_contam_delta(snapshot.color_main, snapshot.color_secondary)
            };
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
                body_part: hit_parts[0],
                wound_kind,
                source: crate::combat::events::AttackSource::Melee,
                debug_command: false,
                physical_damage: 0.0,
                damage: damage_profile.damage,
                contam_delta,
                description: format!(
                    "{:?} {} -> {:?} qi {:.3}",
                    snapshot.kind, snapshot.id, target, snapshot.qi_invest_amount
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
                        LifecycleState::AwaitingRevival | LifecycleState::Terminated
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
            if let Ok((mut practice_log, qi_color)) = practice_logs.get_mut(snapshot.owner) {
                record_style_practice(&mut practice_log, ColorKind::Intricate, qi_color);
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn apply_beast_trap_snap(
    target: Entity,
    owner: Entity,
    pos: [i32; 3],
    tick: u64,
    targets: &mut Query<ZhenfaDamageTarget<'_>>,
    status_effects: &mut EventWriter<ApplyStatusEffectIntent>,
    sense_pulses: &mut EventWriter<ZhenfaSensePulse>,
    vfx_events: Option<&mut Events<VfxEventRequest>>,
) {
    let Ok((_target, _position, mut wounds, ..)) = targets.get_mut(target) else {
        return;
    };
    wounds.health_current =
        (wounds.health_current - BEAST_TRAP_SNAP_DAMAGE).clamp(0.0, wounds.health_max);
    wounds.entries.push(Wound {
        // humanoid-only boundary（P0 决议，本轮不迁移）：兽夹陷阱固定命中 LegL 代表部位。
        location: crate::body_plan::legacy_body_part_to_id(BodyPart::LegL),
        kind: WoundKind::Pierce,
        severity: BEAST_TRAP_SNAP_DAMAGE,
        bleeding_per_sec: 0.0,
        created_at_tick: tick,
        inflicted_by: Some("beast_trap".to_string()),
    });
    status_effects.send(ApplyStatusEffectIntent {
        target,
        kind: StatusEffectKind::Immobilized,
        magnitude: 1.0,
        duration_ticks: BEAST_TRAP_IMMOBILIZED_TICKS,
        issued_at_tick: tick,
    });
    sense_pulses.send(ZhenfaSensePulse {
        owner,
        kind: SenseKindV1::ZhenfaWardAlert,
        pos,
        intensity: 1.0,
        generation: tick,
    });
    emit_zhenfa_vfx(
        vfx_events,
        gameplay_vfx::BEAST_TRAP_SNAP,
        pos,
        "#B06338",
        0.85,
        18,
        24,
    );
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
            | ZhenfaKind::NetworkArray
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
    // 杂色玩家专项加成清零（worldview §六.2「只剩基础真元属性」）
    let color_factor = if !qi_color.is_chaotic
        && color_matches(qi_color.main, qi_color.secondary, ColorKind::Solid)
    {
        2.0
    } else {
        1.0
    };
    ((base_ticks as f64) * specialist_factor * color_factor).round() as u64
}

fn zhenfa_instance_duration_ticks(
    kind: ZhenfaKind,
    base_ticks: u64,
    qi_color: &QiColor,
    specialist: ZhenfaSpecialistLevel,
) -> u64 {
    if kind == ZhenfaKind::DeceiveHeaven {
        return DECEIVE_HEAVEN_DURATION_TICKS;
    }
    effective_duration_ticks(base_ticks, qi_color, specialist)
}

fn active_trigger_range(cultivation: &Cultivation, qi_color: &QiColor) -> f64 {
    let base =
        crate::cultivation::spiritual_sense::scanner::scan_radius_for_realm(cultivation.realm);
    let base = if base <= 0.0 { 16.0 } else { base };
    // 杂色玩家专项加成清零（worldview §六.2「只剩基础真元属性」）
    if !qi_color.is_chaotic
        && color_matches(qi_color.main, qi_color.secondary, ColorKind::Intricate)
    {
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

pub fn deceive_heaven_reveal_chance(_realm: Realm) -> f64 {
    DECEIVE_HEAVEN_REVEAL_CHANCE
}

fn deceive_heaven_detected(instance: &ZhenfaInstance, tick: u64) -> bool {
    deceive_heaven_reveal_tick(
        instance.id,
        instance.placed_at_tick,
        instance.expires_at_tick,
        instance.realm_at_cast,
    )
    .is_some_and(|reveal_tick| tick >= reveal_tick)
}

fn deceive_heaven_reveal_tick(
    array_id: u64,
    placed_at_tick: u64,
    expires_at_tick: u64,
    realm: Realm,
) -> Option<u64> {
    let duration_ticks = expires_at_tick.saturating_sub(placed_at_tick);
    if duration_ticks == 0
        || deterministic_lifetime_roll(array_id) > deceive_heaven_reveal_chance(realm)
    {
        return None;
    }

    let offset = 1 + deterministic_reveal_offset(array_id, duration_ticks.saturating_sub(1));
    Some(placed_at_tick.saturating_add(offset))
}

fn has_zhenfa_flag(inventory: Option<&PlayerInventory>) -> bool {
    let Some(inventory) = inventory else {
        return false;
    };
    inventory
        .equipped
        .values()
        .flat_map(|s| s.iter_all())
        .chain(inventory.hotbar.iter().flatten())
        .any(|item| item.template_id == ZHENFA_FLAG_ITEM_ID)
}

fn validate_network_array_place_item(
    kind: ZhenfaKind,
    inventory: Option<&PlayerInventory>,
    item_instance_id: Option<u64>,
) -> Result<Option<NetworkArrayPlaceItem>, String> {
    if kind != ZhenfaKind::NetworkArray {
        return Ok(None);
    }
    let item_instance_id =
        item_instance_id.ok_or_else(|| "missing item_instance_id".to_string())?;
    let inventory = inventory.ok_or_else(|| "inventory missing".to_string())?;
    let item = inventory_item_by_instance_borrow(inventory, item_instance_id)
        .ok_or_else(|| format!("missing item instance {item_instance_id}"))?;
    match item.template_id.as_str() {
        NETWORK_ARRAY_FLAG_ITEM_ID => Ok(Some(NetworkArrayPlaceItem::Flag)),
        NETWORK_ARRAY_EYE_ITEM_ID => Ok(Some(NetworkArrayPlaceItem::Eye)),
        other => Err(format!(
            "item {other} must be {NETWORK_ARRAY_FLAG_ITEM_ID} or {NETWORK_ARRAY_EYE_ITEM_ID}"
        )),
    }
}

fn backlash_contam_delta(kind: ZhenfaKind) -> f64 {
    match kind {
        ZhenfaKind::Trap | ZhenfaKind::BlastTrap => 0.5,
        ZhenfaKind::WarningTrap => 0.2,
        ZhenfaKind::SlowTrap => 0.35,
        ZhenfaKind::BeastTrap | ZhenfaKind::TripWire | ZhenfaKind::DecoyStake => 0.0,
        ZhenfaKind::Ward => 0.3,
        ZhenfaKind::ShrineWard => 0.35,
        ZhenfaKind::Lingju => 0.25,
        ZhenfaKind::DeceiveHeaven => 1.5,
        ZhenfaKind::Illusion => 0.2,
        ZhenfaKind::NetworkArray => 0.2,
    }
}

fn is_mundane_mechanical_trap_kind(kind: ZhenfaKind) -> bool {
    matches!(
        kind,
        ZhenfaKind::BeastTrap | ZhenfaKind::TripWire | ZhenfaKind::DecoyStake
    )
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
        // humanoid-only boundary（P0 决议，本轮不迁移）：布阵反噬固定命中 ArmR（持阵手）
        // 代表部位。
        location: crate::body_plan::legacy_body_part_to_id(BodyPart::ArmR),
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

fn deterministic_lifetime_roll(instance_id: u64) -> f64 {
    deterministic_seeded_roll(instance_id, 0xD3CE_1FEA_5EED_u64)
}

fn deterministic_reveal_offset(instance_id: u64, max_offset: u64) -> u64 {
    if max_offset == 0 {
        return 0;
    }
    (deterministic_seeded_roll(instance_id, 0xA11E_5EED_u64) * (max_offset as f64)).floor() as u64
}

fn deterministic_seeded_roll(instance_id: u64, salt: u64) -> f64 {
    let mut x = instance_id.rotate_left(17) ^ salt.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    x ^= x >> 30;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^= x >> 31;
    (x as f64) / ((u64::MAX as f64) + 1.0)
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

fn blast_has_clear_los(
    layers: &mut Query<&mut ChunkLayer, With<OverworldLayer>>,
    trap_pos: [i32; 3],
    target_pos: valence::math::DVec3,
) -> bool {
    let Some(layer) = layers.iter_mut().next() else {
        return true;
    };
    let origin = valence::math::DVec3::new(
        f64::from(trap_pos[0]) + 0.5,
        f64::from(trap_pos[1]) + 0.5,
        f64::from(trap_pos[2]) + 0.5,
    );
    let delta = target_pos - origin;
    let steps = ((delta.x.abs().max(delta.z.abs()).max(delta.y.abs())) * 4.0)
        .ceil()
        .max(1.0) as i32;

    for step in 1..steps {
        let t = f64::from(step) / f64::from(steps);
        let sample = origin + delta * t;
        let block_pos = BlockPos::new(
            sample.x.floor() as i32,
            sample.y.floor() as i32,
            sample.z.floor() as i32,
        );
        if block_pos == block_pos_from_array(trap_pos) {
            continue;
        }
        let Some(block) = layer.block(block_pos) else {
            continue;
        };
        if is_solid_for_blast_los(block.state) {
            return false;
        }
    }
    true
}

fn is_solid_for_blast_los(block: BlockState) -> bool {
    if block == BlockState::AIR || block == BlockState::CAVE_AIR {
        return false;
    }
    block.collision_shapes().next().is_some()
}

fn zone_qi_at_pos(zones: Option<&ZoneRegistry>, pos: [i32; 3]) -> Option<f64> {
    zones
        .and_then(|zones| {
            zones.find_zone(
                DimensionKind::Overworld,
                valence::math::DVec3::new(
                    f64::from(pos[0]) + 0.5,
                    f64::from(pos[1]) + 0.5,
                    f64::from(pos[2]) + 0.5,
                ),
            )
        })
        .map(|zone| zone.spirit_qi)
}

fn release_trap_snapshot_qi_to_zone(
    zones: Option<&mut ZoneRegistry>,
    qi_transfers: &mut EventWriter<QiTransfer>,
    snapshot: &TriggerSnapshot,
) {
    release_zhenfa_qi_amount_to_zone(
        zones,
        qi_transfers,
        snapshot.id,
        snapshot.owner_player_id.as_str(),
        snapshot.pos,
        snapshot.qi_invest_amount,
    );
}

fn release_zhenfa_qi_to_zone(
    zones: Option<&mut ZoneRegistry>,
    qi_transfers: &mut EventWriter<QiTransfer>,
    instance: &ZhenfaInstance,
) {
    release_zhenfa_qi_amount_to_zone(
        zones,
        qi_transfers,
        instance.id,
        instance.owner_player_id.as_str(),
        instance.pos,
        instance.qi_invest_amount,
    );
}

fn release_zhenfa_qi_amount_to_zone(
    zones: Option<&mut ZoneRegistry>,
    qi_transfers: &mut EventWriter<QiTransfer>,
    array_id: u64,
    owner_player_id: &str,
    pos: [i32; 3],
    amount: f64,
) {
    if amount <= f64::EPSILON {
        return;
    }
    let from = zhenfa_sealed_qi_account(owner_player_id, array_id);
    let Some(zones) = zones else {
        tracing::warn!(
            "[bong][zhenfa] zhenfa qi release routed to overflow: ZoneRegistry missing array_id={array_id}"
        );
        send_zhenfa_release_overflow(qi_transfers, from, owner_player_id, array_id, amount);
        return;
    };
    let zone_name = zones
        .find_zone(
            DimensionKind::Overworld,
            valence::math::DVec3::new(
                f64::from(pos[0]) + 0.5,
                f64::from(pos[1]) + 0.5,
                f64::from(pos[2]) + 0.5,
            ),
        )
        .map(|zone| zone.name.clone());
    let Some(zone_name) = zone_name else {
        tracing::warn!(
            "[bong][zhenfa] zhenfa qi release routed to overflow: no zone for array_id={array_id} pos={pos:?}"
        );
        send_zhenfa_release_overflow(qi_transfers, from, owner_player_id, array_id, amount);
        return;
    };
    let Some(zone) = zones.find_zone_mut(zone_name.as_str()) else {
        send_zhenfa_release_overflow(qi_transfers, from, owner_player_id, array_id, amount);
        return;
    };
    let to = QiAccountId::zone(zone.name.clone());
    let zone_current = zone.spirit_qi.max(0.0) * QI_ZONE_UNIT_CAPACITY;
    match qi_release_to_zone(
        amount,
        from.clone(),
        to,
        zone_current,
        QI_ZONE_UNIT_CAPACITY,
    ) {
        Ok(outcome) => {
            zone.spirit_qi = (outcome.zone_after / QI_ZONE_UNIT_CAPACITY).clamp(-1.0, 1.0);
            if let Some(transfer) = outcome.transfer {
                qi_transfers.send(transfer);
            }
            if outcome.overflow > f64::EPSILON {
                send_zhenfa_release_overflow(
                    qi_transfers,
                    from,
                    owner_player_id,
                    array_id,
                    outcome.overflow,
                );
            }
        }
        Err(error) => {
            tracing::warn!(
                ?error,
                "[bong][zhenfa] invalid trap qi release array_id={array_id}"
            );
            send_zhenfa_release_overflow(qi_transfers, from, owner_player_id, array_id, amount);
        }
    }
}

fn zhenfa_sealed_qi_account(owner_player_id: &str, array_id: u64) -> QiAccountId {
    QiAccountId::container(format!("zhenfa_trap:{owner_player_id}:{array_id}"))
}

fn emit_zhenfa_seal_qi_transfer(
    qi_transfers: &mut EventWriter<QiTransfer>,
    owner_player_id: &str,
    array_id: u64,
    amount: f64,
) {
    if amount <= f64::EPSILON {
        return;
    }
    if let Ok(transfer) = QiTransfer::new(
        QiAccountId::player(owner_player_id.to_string()),
        zhenfa_sealed_qi_account(owner_player_id, array_id),
        amount,
        QiTransferReason::Channeling,
    ) {
        qi_transfers.send(transfer);
    }
}

fn send_zhenfa_release_overflow(
    qi_transfers: &mut EventWriter<QiTransfer>,
    from: QiAccountId,
    owner_player_id: &str,
    array_id: u64,
    amount: f64,
) {
    if amount <= f64::EPSILON {
        return;
    }
    let overflow_to = QiAccountId::overflow(format!(
        "zhenfa_release_overflow:{owner_player_id}:{array_id}"
    ));
    if let Ok(transfer) =
        QiTransfer::new(from, overflow_to, amount, QiTransferReason::ReleaseToZone)
    {
        qi_transfers.send(transfer);
    }
}

fn should_release_sealed_qi_to_zone(kind: ZhenfaKind) -> bool {
    // NetworkArray has cap_invest_ratio=0.0 so qi_cost is always zero; emit_zhenfa_seal_qi_transfer
    // and release_zhenfa_qi_amount_to_zone both guard on `amount <= EPSILON`, so including it here
    // is harmless.  We return true for every kind that can carry a nonzero qi_invest_amount so the
    // conservation ledger stays balanced on both deploy and expiry.
    !matches!(kind, ZhenfaKind::NetworkArray)
}

pub fn is_beast_target(tag: &FaunaTag) -> bool {
    matches!(
        tag.beast_kind,
        BeastKind::Rat
            | BeastKind::Spider
            | BeastKind::GreenSpider
            | BeastKind::JungleScorpion
            | BeastKind::CockadeSnake
    )
}

fn is_non_owner_beast_target(target: Entity, owner: Entity, fauna_tag: Option<&FaunaTag>) -> bool {
    target != owner && fauna_tag.is_some_and(is_beast_target)
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
#[path = "mod_tests.rs"]
mod tests;
