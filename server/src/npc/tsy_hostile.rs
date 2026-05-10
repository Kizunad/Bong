//! plan-tsy-hostile-v1 — TSY 敌对 NPC 分层、spawn pool、Fuya 光环与 NPC 掉落。

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::Path;

use bevy_transform::components::{GlobalTransform, Transform};
use big_brain::prelude::{
    ActionBuilder, ActionState, Actor, BigBrainSet, FirstToScore, Score, ScorerBuilder, Thinker,
    ThinkerBuilder,
};
use serde::Deserialize;
use valence::entity::marker::MarkerEntityBundle;
use valence::prelude::{
    bevy_ecs, Added, App, Commands, Component, DVec3, Entity, EntityLayerId, Event, EventReader,
    EventWriter, IntoSystemConfigs, Position, PreUpdate, Query, Res, ResMut, Resource, Update,
    With,
};

use crate::combat::components::Wounds;
use crate::combat::events::{AttackIntent, AttackSource};
use crate::cultivation::components::Cultivation;
use crate::fauna::experience::play_audio;
use crate::fauna::visual::{
    FaunaVisualKind, DAOXIANG_ENTITY_KIND, FUYA_ENTITY_KIND, TSY_SENTINEL_ENTITY_KIND,
    ZHINIAN_ENTITY_KIND,
};
use crate::inventory::ancient_relics::{AncientRelicPool, AncientRelicSource};
use crate::inventory::{
    DroppedLootEntry, DroppedLootRegistry, InventoryInstanceIdAllocator, ItemInstance, ItemRegistry,
};
use crate::network::audio_event_emit::{PlaySoundRecipeRequest, StopSoundRecipeRequest};
use crate::npc::brain::{
    AgeingScorer, ChaseAction, ChaseTargetScorer, DashAction, MeleeAttackAction, MeleeRangeScorer,
    RetireAction, WanderAction, WanderScorer,
};
use crate::npc::lifecycle::{npc_runtime_bundle, NpcArchetype, NpcRegistry};
use crate::npc::movement::{GameTick, MovementCapabilities, MovementController, MovementCooldowns};
use crate::npc::navigator::Navigator;
use crate::npc::patrol::NpcPatrol;
use crate::npc::spawn::{
    NpcBlackboard, NpcCombatLoadout, NpcMarker, NpcMeleeArchetype, NpcMeleeProfile,
};
use crate::world::dimension::DimensionKind;
use crate::world::tsy_container::{ContainerKind, LootContainer};
use crate::world::tsy_origin::TsyOrigin;
use crate::world::zone::{TsyDepth, Zone, ZoneRegistry};

const DAOXIANG_INSTINCT_COOLDOWN_TICKS: u32 = 600;
const DAOXIANG_INSTINCT_LOW_QI_RATIO: f64 = 0.2;
const ZHINIAN_AMBUSH_RANGE: f32 = 8.0;
const ZHINIAN_CHASE_RANGE: f32 = 32.0;
const SENTINEL_AGGRO_RANGE: f32 = 16.0;
const FUYA_CHARGE_MIN_RANGE: f32 = 5.0;
const FUYA_CHARGE_MAX_RANGE: f32 = 12.0;
const FUYA_DEFAULT_AURA_RADIUS: f32 = 8.0;
const FUYA_DEFAULT_DRAIN_MULTIPLIER: f64 = 1.5;
const FUYA_PRESSURE_AUDIO_FLAG_PREFIX: &str = "fauna_fuya_pressure";

type AddedFuyaAuraQuery<'w, 's> =
    Query<'w, 's, (Entity, &'static Position), (With<FuyaAura>, Added<FuyaAura>)>;

pub const DEFAULT_TSY_SPAWN_POOLS_PATH: &str = "tsy_spawn_pools.json";
pub const DEFAULT_TSY_DROPS_PATH: &str = "tsy_drops.json";

#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct TsyHostileMarker {
    pub family_id: String,
}

#[derive(Component, Debug, Clone)]
pub struct TsySentinelMarker {
    pub family_id: String,
    pub guarding_container: Option<Entity>,
    pub phase: u8,
    pub max_phase: u8,
}

#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct FuyaAura {
    pub radius_blocks: f32,
    pub drain_boost_multiplier: f64,
}

impl Default for FuyaAura {
    fn default() -> Self {
        Self {
            radius_blocks: FUYA_DEFAULT_AURA_RADIUS,
            drain_boost_multiplier: FUYA_DEFAULT_DRAIN_MULTIPLIER,
        }
    }
}

#[derive(Component, Debug, Clone, Copy)]
pub struct FuyaEnragedMarker;

#[derive(Component, Debug, Clone, Copy)]
pub struct DaoxiangInstinctCooldown {
    pub ready_at_tick: u32,
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZhinianPhase {
    Masquerade,
    Aggressive,
}

#[derive(Component, Debug, Clone)]
pub struct ZhinianMind {
    pub phase: ZhinianPhase,
    pub phase_entered_at_tick: u64,
    pub combat_memory: CombatCombo,
}

impl Default for ZhinianMind {
    fn default() -> Self {
        Self {
            phase: ZhinianPhase::Masquerade,
            phase_entered_at_tick: 0,
            combat_memory: CombatCombo::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CombatCombo {
    pub steps: Vec<ComboStep>,
    pub current_step: usize,
}

impl Default for CombatCombo {
    fn default() -> Self {
        Self {
            steps: vec![
                ComboStep {
                    kind: ComboKind::Melee,
                    cooldown_ticks: 20,
                    damage_mul: 1.4,
                },
                ComboStep {
                    kind: ComboKind::Dash,
                    cooldown_ticks: 40,
                    damage_mul: 1.8,
                },
                ComboStep {
                    kind: ComboKind::Projectile,
                    cooldown_ticks: 50,
                    damage_mul: 1.2,
                },
            ],
            current_step: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ComboStep {
    pub kind: ComboKind,
    pub cooldown_ticks: u32,
    pub damage_mul: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComboKind {
    Melee,
    Dash,
    Projectile,
}

#[derive(Component, Clone, Copy, Debug)]
pub struct DaoxiangInstinctScorer;

#[derive(Component, Clone, Copy, Debug)]
pub struct DaoxiangInstinctAction;

#[derive(Component, Clone, Copy, Debug)]
pub struct ZhinianAmbushScorer;

#[derive(Component, Clone, Copy, Debug)]
pub struct ZhinianChaseScorer;

#[derive(Component, Clone, Copy, Debug)]
pub struct ZhinianComboStepAction;

#[derive(Component, Clone, Copy, Debug)]
pub struct SentinelAggroScorer;

#[derive(Component, Clone, Copy, Debug)]
pub struct SentinelPhaseAction;

#[derive(Component, Clone, Copy, Debug)]
pub struct FuyaEnrageScorer;

#[derive(Component, Clone, Copy, Debug)]
pub struct FuyaChargeScorer;

#[derive(Component, Clone, Copy, Debug)]
pub struct FuyaEnrageAction;

#[derive(Component, Debug, Clone, Copy)]
pub struct TsyNpcDropIssued;

macro_rules! impl_builder {
    ($ty:ty, $trait_name:ident, $label:literal) => {
        impl $trait_name for $ty {
            fn build(&self, cmd: &mut Commands, entity: Entity, _actor: Entity) {
                cmd.entity(entity).insert(*self);
            }

            fn label(&self) -> Option<&str> {
                Some($label)
            }
        }
    };
}

impl_builder!(
    DaoxiangInstinctScorer,
    ScorerBuilder,
    "DaoxiangInstinctScorer"
);
impl_builder!(ZhinianAmbushScorer, ScorerBuilder, "ZhinianAmbushScorer");
impl_builder!(ZhinianChaseScorer, ScorerBuilder, "ZhinianChaseScorer");
impl_builder!(SentinelAggroScorer, ScorerBuilder, "SentinelAggroScorer");
impl_builder!(FuyaEnrageScorer, ScorerBuilder, "FuyaEnrageScorer");
impl_builder!(FuyaChargeScorer, ScorerBuilder, "FuyaChargeScorer");
impl_builder!(
    DaoxiangInstinctAction,
    ActionBuilder,
    "DaoxiangInstinctAction"
);
impl_builder!(
    ZhinianComboStepAction,
    ActionBuilder,
    "ZhinianComboStepAction"
);
impl_builder!(SentinelPhaseAction, ActionBuilder, "SentinelPhaseAction");
impl_builder!(FuyaEnrageAction, ActionBuilder, "FuyaEnrageAction");

#[derive(Debug, Default, Resource)]
pub struct TsySpawnPoolRegistry {
    by_origin: HashMap<TsyOrigin, TsyOriginSpawnPool>,
    sentinel_count_by_origin: HashMap<TsyOrigin, u32>,
}

impl TsySpawnPoolRegistry {
    pub fn get_for_family(&self, family_id: &str) -> Option<&TsyOriginSpawnPool> {
        let origin = TsyOrigin::from_zone_name(family_id)?;
        self.by_origin.get(&origin)
    }

    pub fn sentinel_count_for_family(&self, family_id: &str) -> u32 {
        TsyOrigin::from_zone_name(family_id)
            .and_then(|origin| self.sentinel_count_by_origin.get(&origin).copied())
            .unwrap_or(0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TsyOriginSpawnPool {
    pub shallow: TsyLayerSpawnCounts,
    pub mid: TsyLayerSpawnCounts,
    pub deep: TsyLayerSpawnCounts,
}

impl TsyOriginSpawnPool {
    pub fn for_depth(&self, depth: TsyDepth) -> TsyLayerSpawnCounts {
        match depth {
            TsyDepth::Shallow => self.shallow,
            TsyDepth::Mid => self.mid,
            TsyDepth::Deep => self.deep,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TsyLayerSpawnCounts {
    pub daoxiang: u32,
    pub zhinian: u32,
    pub fuya: u32,
}

impl TsyLayerSpawnCounts {
    pub const fn total(self) -> u32 {
        self.daoxiang + self.zhinian + self.fuya
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct TsyHostileSpawnSummary {
    pub daoxiang: u32,
    pub zhinian: u32,
    pub fuya: u32,
    pub sentinel: u32,
}

#[derive(Event, Debug, Clone)]
pub struct TsyNpcSpawned {
    pub family_id: String,
    pub archetype: TsyHostileArchetype,
    pub count: u32,
    pub at_tick: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TsyHostileArchetype {
    Daoxiang,
    Zhinian,
    GuardianRelicSentinel,
    Fuya,
}

#[derive(Event, Debug, Clone)]
pub struct TsySentinelPhaseChanged {
    pub family_id: String,
    pub container_entity_id: u64,
    pub phase: u8,
    pub max_phase: u8,
    pub at_tick: u64,
}

impl TsyHostileSpawnSummary {
    pub const fn total(self) -> u32 {
        self.daoxiang + self.zhinian + self.fuya + self.sentinel
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TsyContainerSpawnRef {
    pub entity: Entity,
    pub pos: DVec3,
}

#[derive(Debug, Default, Resource)]
pub struct TsyNpcDropTableRegistry {
    entries: HashMap<String, TsyDropTableEntry>,
}

impl TsyNpcDropTableRegistry {
    pub fn get(&self, key: &str) -> Option<&TsyDropTableEntry> {
        self.entries.get(key)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TsyDropTableEntry {
    pub guaranteed: Vec<TsyDropRoll>,
    pub rolls: Vec<TsyDropRoll>,
    pub max_rolls: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TsyDropRoll {
    pub template_id: String,
    pub chance: f32,
    pub count: (u32, u32),
}

#[allow(clippy::too_many_arguments)]
pub fn register(app: &mut App) {
    let spawn_pools = load_tsy_spawn_pool_registry().unwrap_or_else(|error| {
        tracing::error!(
            "[bong][tsy-hostile] failed to load tsy_spawn_pools.json: {error} — using empty registry, tsy hostile spawning disabled"
        );
        TsySpawnPoolRegistry::default()
    });
    let drop_tables = load_tsy_drop_table_registry().unwrap_or_else(|error| {
        tracing::error!(
            "[bong][tsy-hostile] failed to load tsy_drops.json: {error} — using empty registry, tsy hostile drops disabled"
        );
        TsyNpcDropTableRegistry::default()
    });

    app.add_event::<TsyNpcSpawned>()
        .add_event::<TsySentinelPhaseChanged>()
        .add_event::<TsyHostileSpawnedSummary>()
        .insert_resource(spawn_pools)
        .insert_resource(drop_tables)
        .add_systems(
            PreUpdate,
            (
                update_sentinel_phase_system,
                daoxiang_instinct_scorer_system,
                zhinian_ambush_scorer_system,
                zhinian_chase_scorer_system,
                sentinel_aggro_scorer_system,
                fuya_enrage_scorer_system,
                fuya_charge_scorer_system,
            )
                .in_set(BigBrainSet::Scorers),
        )
        .add_systems(
            PreUpdate,
            (
                daoxiang_instinct_action_system,
                zhinian_combo_step_action_system,
                sentinel_phase_action_system,
                fuya_enrage_action_system,
            )
                .in_set(BigBrainSet::Actions),
        )
        .add_systems(
            Update,
            (
                emit_tsy_hostile_spawn_summary
                    .after(crate::world::tsy_dev_command::apply_tsy_spawn_requests),
                emit_fuya_pressure_hum_audio_system,
                stop_fuya_pressure_hum_audio_on_death_system,
                handle_npc_death_drop,
            ),
        );
}

pub fn load_tsy_spawn_pool_registry() -> Result<TsySpawnPoolRegistry, String> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(DEFAULT_TSY_SPAWN_POOLS_PATH);
    load_tsy_spawn_pool_registry_from_path(path)
}

pub fn load_tsy_spawn_pool_registry_from_path(
    path: impl AsRef<Path>,
) -> Result<TsySpawnPoolRegistry, String> {
    let path = path.as_ref();
    let content = std::fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    let raw: TsySpawnPoolsJson = serde_json::from_str(&content)
        .map_err(|error| format!("failed to parse {}: {error}", path.display()))?;

    let mut by_origin = HashMap::with_capacity(raw.by_origin.len());
    for (origin_key, pool) in raw.by_origin {
        let origin = TsyOrigin::from_str(&origin_key)
            .ok_or_else(|| format!("unknown TSY origin `{origin_key}`"))?;
        by_origin.insert(origin, pool.into_pool());
    }

    let mut sentinel_count_by_origin = HashMap::with_capacity(raw.sentinel_count_by_origin.len());
    for (origin_key, count) in raw.sentinel_count_by_origin {
        let origin = TsyOrigin::from_str(&origin_key)
            .ok_or_else(|| format!("unknown TSY origin `{origin_key}`"))?;
        sentinel_count_by_origin.insert(origin, count);
    }

    Ok(TsySpawnPoolRegistry {
        by_origin,
        sentinel_count_by_origin,
    })
}

pub fn load_tsy_drop_table_registry() -> Result<TsyNpcDropTableRegistry, String> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(DEFAULT_TSY_DROPS_PATH);
    load_tsy_drop_table_registry_from_path(path)
}

pub fn load_tsy_drop_table_registry_from_path(
    path: impl AsRef<Path>,
) -> Result<TsyNpcDropTableRegistry, String> {
    let path = path.as_ref();
    let content = std::fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    let raw: TsyDropsJson = serde_json::from_str(&content)
        .map_err(|error| format!("failed to parse {}: {error}", path.display()))?;
    let mut entries = HashMap::with_capacity(raw.tables.len());
    for (key, entry) in raw.tables {
        entries.insert(key, entry.into_entry());
    }
    Ok(TsyNpcDropTableRegistry { entries })
}

#[allow(clippy::too_many_arguments)]
pub fn spawn_tsy_hostiles_for_family(
    commands: &mut Commands,
    layer: Entity,
    family_id: &str,
    registry: &TsySpawnPoolRegistry,
    zones: &ZoneRegistry,
    relic_cores: &[TsyContainerSpawnRef],
    tick: u64,
    mut npc_registry: Option<&mut NpcRegistry>,
) -> TsyHostileSpawnSummary {
    let Some(pool) = registry.get_for_family(family_id) else {
        tracing::debug!(family = %family_id, "[bong][tsy-hostile] no spawn pool for family");
        return TsyHostileSpawnSummary::default();
    };

    let sentinel_desired = registry
        .sentinel_count_for_family(family_id)
        .min(relic_cores.len() as u32);
    let desired = [TsyDepth::Shallow, TsyDepth::Mid, TsyDepth::Deep]
        .into_iter()
        .map(|depth| pool.for_depth(depth).total())
        .sum::<u32>()
        .saturating_add(sentinel_desired);
    if desired == 0 {
        return TsyHostileSpawnSummary::default();
    }

    let budget = npc_registry
        .as_deref_mut()
        .map(|registry| reserve_tsy_family_budget(registry, family_id, pool, sentinel_desired))
        .unwrap_or(desired);
    if budget == 0 {
        return TsyHostileSpawnSummary::default();
    }

    let mut remaining = budget;
    let mut summary = TsyHostileSpawnSummary::default();

    for depth in [TsyDepth::Shallow, TsyDepth::Mid, TsyDepth::Deep] {
        let counts = pool.for_depth(depth);
        let zone_name = format!("{family_id}_{}", depth_suffix(depth));
        let Some(zone) = zones.find_zone_by_name(&zone_name) else {
            continue;
        };
        spawn_layer_hostiles(
            commands,
            layer,
            family_id,
            depth,
            zone,
            counts,
            tick,
            &mut remaining,
            &mut summary,
        );
        if remaining == 0 {
            break;
        }
    }

    let sentinel_count = sentinel_desired.min(remaining);
    for guard in relic_cores.iter().take(sentinel_count as usize) {
        let entity = spawn_tsy_sentinel_at(
            commands,
            layer,
            family_id,
            &format!("{family_id}_deep"),
            guard.pos + DVec3::new(2.0, 0.0, 0.0),
            guard.entity,
        );
        tracing::debug!(?entity, family = %family_id, "[bong][tsy-hostile] spawned TSY sentinel");
        summary.sentinel = summary.sentinel.saturating_add(1);
        remaining = remaining.saturating_sub(1);
    }

    if let Some(registry) = npc_registry {
        registry.release_zone_batch(&format!("{family_id}_deep"), remaining as usize);
    }

    summary
}

fn reserve_tsy_family_budget(
    registry: &mut NpcRegistry,
    family_id: &str,
    pool: &TsyOriginSpawnPool,
    sentinel_desired: u32,
) -> u32 {
    [TsyDepth::Shallow, TsyDepth::Mid, TsyDepth::Deep]
        .into_iter()
        .map(|depth| {
            registry.reserve_zone_batch(
                &format!("{family_id}_{}", depth_suffix(depth)),
                pool.for_depth(depth).total() as usize,
            ) as u32
        })
        .sum::<u32>()
        .saturating_add(
            registry.reserve_zone_batch(&format!("{family_id}_deep"), sentinel_desired as usize)
                as u32,
        )
}

#[allow(clippy::too_many_arguments)]
fn spawn_layer_hostiles(
    commands: &mut Commands,
    layer: Entity,
    family_id: &str,
    depth: TsyDepth,
    zone: &Zone,
    counts: TsyLayerSpawnCounts,
    tick: u64,
    remaining: &mut u32,
    summary: &mut TsyHostileSpawnSummary,
) {
    for i in 0..counts.daoxiang {
        if *remaining == 0 {
            return;
        }
        if let Some(pos) = sample_hostile_position(zone, family_id, depth, "daoxiang", i, tick) {
            spawn_tsy_daoxiang_at(
                commands,
                layer,
                family_id,
                &zone.name,
                pos,
                zone.patrol_target(0),
            );
            summary.daoxiang = summary.daoxiang.saturating_add(1);
            *remaining = remaining.saturating_sub(1);
        }
    }
    for i in 0..counts.zhinian {
        if *remaining == 0 {
            return;
        }
        if let Some(pos) = sample_hostile_position(zone, family_id, depth, "zhinian", i, tick) {
            spawn_tsy_zhinian_at(
                commands,
                layer,
                family_id,
                &zone.name,
                pos,
                zone.patrol_target(0),
            );
            summary.zhinian = summary.zhinian.saturating_add(1);
            *remaining = remaining.saturating_sub(1);
        }
    }
    for i in 0..counts.fuya {
        if *remaining == 0 {
            return;
        }
        if let Some(pos) = sample_hostile_position(zone, family_id, depth, "fuya", i, tick) {
            spawn_tsy_fuya_at(
                commands,
                layer,
                family_id,
                &zone.name,
                pos,
                zone.patrol_target(0),
            );
            summary.fuya = summary.fuya.saturating_add(1);
            *remaining = remaining.saturating_sub(1);
        }
    }
}

pub fn spawn_tsy_daoxiang_at(
    commands: &mut Commands,
    layer: Entity,
    family_id: &str,
    home_zone: &str,
    spawn_position: DVec3,
    patrol_target: DVec3,
) -> Entity {
    let loadout = NpcCombatLoadout::new(
        NpcMeleeArchetype::Brawler,
        MovementCapabilities {
            can_sprint: false,
            can_dash: false,
        },
    );
    let entity = spawn_zombie_shell(
        commands,
        layer,
        NpcArchetype::Daoxiang,
        loadout.clone(),
        home_zone,
        spawn_position,
        patrol_target,
    );
    commands.entity(entity).insert((
        TsyHostileMarker {
            family_id: family_id.to_string(),
        },
        crate::npc::brain::WanderState::default(),
        daoxiang_thinker(),
    ));
    commands
        .entity(entity)
        .insert(npc_runtime_bundle(entity, NpcArchetype::Daoxiang));
    entity
}

pub fn emit_tsy_hostile_spawn_summary(
    mut events: EventWriter<TsyNpcSpawned>,
    mut summaries: EventReader<TsyHostileSpawnedSummary>,
) {
    for summary in summaries.read() {
        send_spawn_event(
            &mut events,
            &summary.family_id,
            TsyHostileArchetype::Daoxiang,
            summary.daoxiang,
            summary.at_tick,
        );
        send_spawn_event(
            &mut events,
            &summary.family_id,
            TsyHostileArchetype::Zhinian,
            summary.zhinian,
            summary.at_tick,
        );
        send_spawn_event(
            &mut events,
            &summary.family_id,
            TsyHostileArchetype::Fuya,
            summary.fuya,
            summary.at_tick,
        );
        send_spawn_event(
            &mut events,
            &summary.family_id,
            TsyHostileArchetype::GuardianRelicSentinel,
            summary.sentinel,
            summary.at_tick,
        );
    }
}

fn send_spawn_event(
    events: &mut EventWriter<TsyNpcSpawned>,
    family_id: &str,
    archetype: TsyHostileArchetype,
    count: u32,
    at_tick: u64,
) {
    if count == 0 {
        return;
    }
    events.send(TsyNpcSpawned {
        family_id: family_id.to_string(),
        archetype,
        count,
        at_tick,
    });
}

#[derive(Event, Debug, Clone)]
pub struct TsyHostileSpawnedSummary {
    pub family_id: String,
    pub daoxiang: u32,
    pub zhinian: u32,
    pub fuya: u32,
    pub sentinel: u32,
    pub at_tick: u64,
}

impl TsyHostileSpawnedSummary {
    pub fn from_summary(
        family_id: impl Into<String>,
        summary: TsyHostileSpawnSummary,
        at_tick: u64,
    ) -> Self {
        Self {
            family_id: family_id.into(),
            daoxiang: summary.daoxiang,
            zhinian: summary.zhinian,
            fuya: summary.fuya,
            sentinel: summary.sentinel,
            at_tick,
        }
    }
}

pub fn spawn_tsy_zhinian_at(
    commands: &mut Commands,
    layer: Entity,
    family_id: &str,
    home_zone: &str,
    spawn_position: DVec3,
    patrol_target: DVec3,
) -> Entity {
    let loadout = NpcCombatLoadout::new(
        NpcMeleeArchetype::Sword,
        MovementCapabilities {
            can_sprint: true,
            can_dash: true,
        },
    );
    let entity = spawn_zombie_shell(
        commands,
        layer,
        NpcArchetype::Zhinian,
        loadout.clone(),
        home_zone,
        spawn_position,
        patrol_target,
    );
    commands.entity(entity).insert((
        TsyHostileMarker {
            family_id: family_id.to_string(),
        },
        ZhinianMind::default(),
        crate::npc::brain::WanderState::default(),
        zhinian_thinker(),
    ));
    commands
        .entity(entity)
        .insert(npc_runtime_bundle(entity, NpcArchetype::Zhinian));
    entity
}

pub fn spawn_tsy_fuya_at(
    commands: &mut Commands,
    layer: Entity,
    family_id: &str,
    home_zone: &str,
    spawn_position: DVec3,
    patrol_target: DVec3,
) -> Entity {
    let loadout = NpcCombatLoadout::new(
        NpcMeleeArchetype::Brawler,
        MovementCapabilities {
            can_sprint: false,
            can_dash: true,
        },
    );
    let entity = spawn_zombie_shell(
        commands,
        layer,
        NpcArchetype::Fuya,
        loadout.clone(),
        home_zone,
        spawn_position,
        patrol_target,
    );
    commands.entity(entity).insert((
        TsyHostileMarker {
            family_id: family_id.to_string(),
        },
        FuyaAura::default(),
        crate::npc::brain::WanderState::default(),
        fuya_thinker(),
    ));
    commands
        .entity(entity)
        .insert(npc_runtime_bundle(entity, NpcArchetype::Fuya));
    entity
}

pub fn spawn_tsy_sentinel_at(
    commands: &mut Commands,
    layer: Entity,
    family_id: &str,
    _home_zone: &str,
    spawn_position: DVec3,
    guarding_container: Entity,
) -> Entity {
    let loadout = NpcCombatLoadout::new(
        NpcMeleeArchetype::Sword,
        MovementCapabilities {
            can_sprint: false,
            can_dash: false,
        },
    );
    let entity = commands
        .spawn((
            MarkerEntityBundle {
                kind: TSY_SENTINEL_ENTITY_KIND,
                layer: EntityLayerId(layer),
                position: Position::new([spawn_position.x, spawn_position.y, spawn_position.z]),
                ..Default::default()
            },
            Transform::from_xyz(
                spawn_position.x as f32,
                spawn_position.y as f32,
                spawn_position.z as f32,
            ),
            GlobalTransform::default(),
            NpcMarker,
            NpcArchetype::GuardianRelic,
            FaunaVisualKind::TsySentinel,
        ))
        .id();
    commands.entity(entity).insert((
        NpcBlackboard::default(),
        loadout.clone(),
        loadout.melee_archetype,
        loadout.melee_profile(),
        Navigator::new(),
        MovementController::new(),
        loadout.movement_capabilities,
        MovementCooldowns::default(),
        TsyHostileMarker {
            family_id: family_id.to_string(),
        },
        TsySentinelMarker {
            family_id: family_id.to_string(),
            guarding_container: Some(guarding_container),
            phase: 0,
            max_phase: 3,
        },
        sentinel_thinker(),
    ));
    commands
        .entity(entity)
        .insert(npc_runtime_bundle(entity, NpcArchetype::GuardianRelic));
    entity
}

pub fn emit_fuya_pressure_hum_audio_system(
    fuyas: AddedFuyaAuraQuery<'_, '_>,
    mut audio_events: EventWriter<PlaySoundRecipeRequest>,
) {
    for (fuya, position) in &fuyas {
        let pos = position.get();
        audio_events.send(PlaySoundRecipeRequest {
            recipe_id: "fauna_fuya_pressure_hum".to_string(),
            instance_id: fuya_pressure_audio_instance_id(fuya),
            pos: Some([
                pos.x.floor() as i32,
                pos.y.floor() as i32,
                pos.z.floor() as i32,
            ]),
            flag: Some(fuya_pressure_audio_flag(fuya)),
            volume_mul: 1.0,
            pitch_shift: 0.0,
            recipient: crate::network::audio_event_emit::AudioRecipient::Radius {
                origin: pos,
                radius: 64.0,
            },
        });
    }
}

pub fn stop_fuya_pressure_hum_audio_on_death_system(
    mut deaths: EventReader<crate::combat::events::DeathEvent>,
    fuyas: Query<(), With<FuyaAura>>,
    mut audio_events: EventWriter<StopSoundRecipeRequest>,
) {
    for death in deaths.read() {
        if fuyas.get(death.target).is_err() {
            continue;
        }
        audio_events.send(StopSoundRecipeRequest {
            instance_id: fuya_pressure_audio_instance_id(death.target),
            fade_out_ticks: 20,
            recipient: crate::network::audio_event_emit::AudioRecipient::All,
        });
    }
}

fn fuya_pressure_audio_instance_id(fuya: Entity) -> u64 {
    fuya.to_bits().max(1)
}

fn fuya_pressure_audio_flag(fuya: Entity) -> String {
    format!(
        "{}:{}",
        FUYA_PRESSURE_AUDIO_FLAG_PREFIX,
        fuya_pressure_audio_instance_id(fuya)
    )
}

fn spawn_zombie_shell(
    commands: &mut Commands,
    layer: Entity,
    archetype: NpcArchetype,
    loadout: NpcCombatLoadout,
    home_zone: &str,
    spawn_position: DVec3,
    patrol_target: DVec3,
) -> Entity {
    commands
        .spawn((
            MarkerEntityBundle {
                kind: entity_kind_for_tsy_archetype(archetype),
                layer: EntityLayerId(layer),
                position: Position::new([spawn_position.x, spawn_position.y, spawn_position.z]),
                ..Default::default()
            },
            Transform::from_xyz(
                spawn_position.x as f32,
                spawn_position.y as f32,
                spawn_position.z as f32,
            ),
            GlobalTransform::default(),
            NpcMarker,
            NpcBlackboard::default(),
            loadout.clone(),
            loadout.melee_archetype,
            loadout.melee_profile(),
            archetype,
            Navigator::new(),
            MovementController::new(),
            loadout.movement_capabilities,
            MovementCooldowns::default(),
            NpcPatrol::new(home_zone, patrol_target),
            visual_kind_for_tsy_archetype(archetype),
        ))
        .id()
}

fn entity_kind_for_tsy_archetype(archetype: NpcArchetype) -> valence::prelude::EntityKind {
    match archetype {
        NpcArchetype::Daoxiang => DAOXIANG_ENTITY_KIND,
        NpcArchetype::Zhinian => ZHINIAN_ENTITY_KIND,
        NpcArchetype::Fuya => FUYA_ENTITY_KIND,
        NpcArchetype::Zombie
        | NpcArchetype::Commoner
        | NpcArchetype::Rogue
        | NpcArchetype::Beast
        | NpcArchetype::Disciple
        | NpcArchetype::GuardianRelic => {
            unreachable!("entity_kind_for_tsy_archetype only supports TSY hostile archetypes")
        }
    }
}

fn visual_kind_for_tsy_archetype(archetype: NpcArchetype) -> FaunaVisualKind {
    match archetype {
        NpcArchetype::Daoxiang => FaunaVisualKind::Daoxiang,
        NpcArchetype::Zhinian => FaunaVisualKind::Zhinian,
        NpcArchetype::Fuya => FaunaVisualKind::Fuya,
        NpcArchetype::Zombie
        | NpcArchetype::Commoner
        | NpcArchetype::Rogue
        | NpcArchetype::Beast
        | NpcArchetype::Disciple
        | NpcArchetype::GuardianRelic => {
            unreachable!("visual_kind_for_tsy_archetype only supports TSY hostile archetypes")
        }
    }
}

fn daoxiang_thinker() -> ThinkerBuilder {
    Thinker::build()
        .picker(FirstToScore { threshold: 0.05 })
        .when(AgeingScorer, RetireAction)
        .when(DaoxiangInstinctScorer, DaoxiangInstinctAction)
        .when(MeleeRangeScorer, MeleeAttackAction)
        .when(ChaseTargetScorer, ChaseAction)
        .when(WanderScorer, WanderAction)
}

fn zhinian_thinker() -> ThinkerBuilder {
    Thinker::build()
        .picker(FirstToScore { threshold: 0.05 })
        .when(AgeingScorer, RetireAction)
        .when(ZhinianAmbushScorer, ZhinianComboStepAction)
        .when(MeleeRangeScorer, ZhinianComboStepAction)
        .when(ZhinianChaseScorer, ChaseAction)
        .when(WanderScorer, WanderAction)
}

fn sentinel_thinker() -> ThinkerBuilder {
    Thinker::build()
        .picker(FirstToScore { threshold: 0.05 })
        .when(SentinelAggroScorer, SentinelPhaseAction)
}

fn fuya_thinker() -> ThinkerBuilder {
    Thinker::build()
        .picker(FirstToScore { threshold: 0.05 })
        .when(AgeingScorer, RetireAction)
        .when(FuyaEnrageScorer, FuyaEnrageAction)
        .when(FuyaChargeScorer, DashAction)
        .when(MeleeRangeScorer, MeleeAttackAction)
        .when(ChaseTargetScorer, ChaseAction)
        .when(WanderScorer, WanderAction)
}

pub fn compute_fuya_aura_drain_multiplier<'a>(
    player_pos: DVec3,
    auras: impl IntoIterator<Item = (&'a Position, &'a FuyaAura)>,
) -> f64 {
    auras.into_iter().fold(1.0, |acc, (pos, aura)| {
        let radius = f64::from(aura.radius_blocks.max(0.0));
        if radius > 0.0 && player_pos.distance(pos.get()) <= radius {
            acc * aura.drain_boost_multiplier.max(1.0)
        } else {
            acc
        }
    })
}

#[allow(clippy::type_complexity)]
fn daoxiang_instinct_scorer_system(
    npcs: Query<
        (
            &Position,
            &NpcBlackboard,
            &NpcMeleeProfile,
            Option<&DaoxiangInstinctCooldown>,
        ),
        (With<NpcMarker>, With<TsyHostileMarker>),
    >,
    players: Query<(&Position, &Cultivation, Option<&valence::entity::Look>)>,
    mut scorers: Query<(&Actor, &mut Score), With<DaoxiangInstinctScorer>>,
    game_tick: Option<Res<GameTick>>,
) {
    let tick = game_tick.as_deref().map(|tick| tick.0).unwrap_or(0);
    for (Actor(actor), mut score) in &mut scorers {
        let value = if let Ok((npc_pos, bb, profile, cooldown)) = npcs.get(*actor) {
            let cooldown_ready = cooldown
                .map(|cooldown| tick >= cooldown.ready_at_tick)
                .unwrap_or(true);
            let Some(player_entity) = bb.nearest_player else {
                score.set(0.0);
                continue;
            };
            if let Ok((player_pos, cultivation, look)) = players.get(player_entity) {
                daoxiang_instinct_score(
                    bb.player_distance,
                    profile,
                    qi_ratio(cultivation),
                    player_back_faces_npc(player_pos.get(), look, npc_pos.get()),
                    cooldown_ready,
                )
            } else {
                0.0
            }
        } else {
            0.0
        };
        score.set(value);
    }
}

fn daoxiang_instinct_score(
    distance: f32,
    profile: &NpcMeleeProfile,
    player_qi_ratio: f64,
    player_back_facing: bool,
    cooldown_ready: bool,
) -> f32 {
    if !cooldown_ready || distance > profile.reach.max {
        return 0.0;
    }
    if player_back_facing || player_qi_ratio < DAOXIANG_INSTINCT_LOW_QI_RATIO {
        1.0
    } else {
        0.0
    }
}

fn qi_ratio(cultivation: &Cultivation) -> f64 {
    cultivation.qi_current.max(0.0) / cultivation.qi_max.max(1.0)
}

fn player_back_faces_npc(
    player_pos: DVec3,
    look: Option<&valence::entity::Look>,
    npc_pos: DVec3,
) -> bool {
    let Some(look) = look else { return false };
    let to_npc = npc_pos - player_pos;
    let to_npc = DVec3::new(to_npc.x, 0.0, to_npc.z);
    if to_npc.length_squared() <= f64::EPSILON {
        return false;
    }
    let yaw = f64::from(look.yaw).to_radians();
    let facing = DVec3::new(-yaw.sin(), 0.0, yaw.cos());
    facing.dot(to_npc.normalize()) < 0.0
}

fn daoxiang_instinct_action_system(
    mut commands: Commands,
    mut actions: Query<(&Actor, &mut ActionState), With<DaoxiangInstinctAction>>,
    mut npcs: Query<(&mut NpcBlackboard, &NpcMeleeProfile, &mut Navigator), With<NpcMarker>>,
    mut attack_intents: bevy_ecs::event::EventWriter<AttackIntent>,
    game_tick: Option<Res<GameTick>>,
) {
    let tick = game_tick.as_deref().map(|tick| tick.0).unwrap_or(0);
    for (Actor(actor), mut state) in &mut actions {
        match *state {
            ActionState::Requested => {
                let Ok((mut bb, profile, mut navigator)) = npcs.get_mut(*actor) else {
                    *state = ActionState::Failure;
                    continue;
                };
                navigator.stop();
                if let Some(target) = bb.nearest_player {
                    bb.last_melee_tick = tick;
                    attack_intents.send(AttackIntent {
                        attacker: *actor,
                        target: Some(target),
                        issued_at_tick: u64::from(tick),
                        reach: profile.reach,
                        qi_invest: 25.0,
                        wound_kind: profile.wound_kind,
                        source: AttackSource::Melee,
                        debug_command: None,
                    });
                }
                commands.entity(*actor).insert(DaoxiangInstinctCooldown {
                    ready_at_tick: tick.saturating_add(DAOXIANG_INSTINCT_COOLDOWN_TICKS),
                });
                *state = ActionState::Success;
            }
            ActionState::Cancelled => *state = ActionState::Failure,
            ActionState::Init
            | ActionState::Executing
            | ActionState::Success
            | ActionState::Failure => {}
        }
    }
}

fn zhinian_ambush_scorer_system(
    npcs: Query<(&NpcBlackboard, &ZhinianMind), With<NpcMarker>>,
    mut scorers: Query<(&Actor, &mut Score), With<ZhinianAmbushScorer>>,
) {
    for (Actor(actor), mut score) in &mut scorers {
        let value = if let Ok((bb, mind)) = npcs.get(*actor) {
            if mind.phase == ZhinianPhase::Masquerade && bb.player_distance <= ZHINIAN_AMBUSH_RANGE
            {
                1.0
            } else {
                0.0
            }
        } else {
            0.0
        };
        score.set(value);
    }
}

fn zhinian_chase_scorer_system(
    npcs: Query<(&NpcBlackboard, &NpcMeleeProfile, &ZhinianMind), With<NpcMarker>>,
    mut scorers: Query<(&Actor, &mut Score), With<ZhinianChaseScorer>>,
) {
    for (Actor(actor), mut score) in &mut scorers {
        let value = if let Ok((bb, profile, mind)) = npcs.get(*actor) {
            if mind.phase != ZhinianPhase::Aggressive
                || !bb.player_distance.is_finite()
                || bb.player_distance > ZHINIAN_CHASE_RANGE
                || bb.player_distance <= profile.preferred_distance
            {
                0.0
            } else {
                ((ZHINIAN_CHASE_RANGE - bb.player_distance) / ZHINIAN_CHASE_RANGE).clamp(0.0, 1.0)
            }
        } else {
            0.0
        };
        score.set(value);
    }
}

fn zhinian_combo_step_action_system(
    mut actions: Query<(&Actor, &mut ActionState), With<ZhinianComboStepAction>>,
    mut npcs: Query<
        (
            &mut NpcBlackboard,
            &NpcMeleeProfile,
            &mut ZhinianMind,
            &mut Navigator,
        ),
        With<NpcMarker>,
    >,
    mut attack_intents: bevy_ecs::event::EventWriter<AttackIntent>,
    game_tick: Option<Res<GameTick>>,
) {
    let tick = game_tick.as_deref().map(|tick| tick.0).unwrap_or(0);
    for (Actor(actor), mut state) in &mut actions {
        match *state {
            ActionState::Requested => {
                let Ok((mut bb, profile, mut mind, mut navigator)) = npcs.get_mut(*actor) else {
                    *state = ActionState::Failure;
                    continue;
                };
                navigator.stop();
                if mind.phase == ZhinianPhase::Masquerade {
                    mind.phase = ZhinianPhase::Aggressive;
                    mind.phase_entered_at_tick = u64::from(tick);
                }
                let step = mind
                    .combat_memory
                    .steps
                    .get(mind.combat_memory.current_step)
                    .copied()
                    .unwrap_or(ComboStep {
                        kind: ComboKind::Melee,
                        cooldown_ticks: 30,
                        damage_mul: 1.0,
                    });
                if tick.wrapping_sub(bb.last_melee_tick) < step.cooldown_ticks {
                    *state = ActionState::Success;
                    continue;
                }
                if let Some(target) = bb.nearest_player {
                    bb.last_melee_tick = tick;
                    let qi_invest = match step.kind {
                        ComboKind::Melee => 10.0,
                        ComboKind::Dash => 14.0,
                        ComboKind::Projectile => 12.0,
                    } * step.damage_mul;
                    attack_intents.send(AttackIntent {
                        attacker: *actor,
                        target: Some(target),
                        issued_at_tick: u64::from(tick),
                        reach: profile.reach,
                        qi_invest,
                        wound_kind: profile.wound_kind,
                        source: AttackSource::Melee,
                        debug_command: None,
                    });
                }
                if !mind.combat_memory.steps.is_empty() {
                    mind.combat_memory.current_step =
                        (mind.combat_memory.current_step + 1) % mind.combat_memory.steps.len();
                }
                *state = ActionState::Success;
            }
            ActionState::Cancelled => *state = ActionState::Failure,
            ActionState::Init
            | ActionState::Executing
            | ActionState::Success
            | ActionState::Failure => {}
        }
    }
}

fn sentinel_aggro_scorer_system(
    npcs: Query<(&NpcBlackboard, &TsySentinelMarker), With<NpcMarker>>,
    mut scorers: Query<(&Actor, &mut Score), With<SentinelAggroScorer>>,
) {
    for (Actor(actor), mut score) in &mut scorers {
        let value = if let Ok((bb, _marker)) = npcs.get(*actor) {
            if bb.player_distance <= SENTINEL_AGGRO_RANGE {
                1.0
            } else {
                0.0
            }
        } else {
            0.0
        };
        score.set(value);
    }
}

fn update_sentinel_phase_system(
    mut sentinels: Query<(&mut TsySentinelMarker, &Wounds)>,
    mut phase_events: EventWriter<TsySentinelPhaseChanged>,
    game_tick: Option<Res<GameTick>>,
) {
    let at_tick = game_tick
        .as_deref()
        .map(|tick| u64::from(tick.0))
        .unwrap_or(0);
    for (mut marker, wounds) in &mut sentinels {
        let next_phase = sentinel_phase_for_wounds(wounds, marker.max_phase);
        if next_phase == marker.phase {
            continue;
        }
        marker.phase = next_phase;
        if let Some(container) = marker.guarding_container {
            phase_events.send(TsySentinelPhaseChanged {
                family_id: marker.family_id.clone(),
                container_entity_id: container.to_bits(),
                phase: marker.phase,
                max_phase: marker.max_phase,
                at_tick,
            });
        }
    }
}

fn sentinel_phase_for_wounds(wounds: &Wounds, max_phase: u8) -> u8 {
    let health_ratio = if wounds.health_max <= f32::EPSILON {
        0.0
    } else {
        (wounds.health_current / wounds.health_max).clamp(0.0, 1.0)
    };
    let phase = if health_ratio <= 1.0 / 3.0 {
        2
    } else if health_ratio <= 2.0 / 3.0 {
        1
    } else {
        0
    };
    phase.min(max_phase.saturating_sub(1))
}

fn sentinel_phase_action_system(
    mut actions: Query<(&Actor, &mut ActionState), With<SentinelPhaseAction>>,
    mut npcs: Query<
        (
            &mut NpcBlackboard,
            &NpcMeleeProfile,
            &mut Navigator,
            &TsySentinelMarker,
        ),
        With<NpcMarker>,
    >,
    mut attack_intents: bevy_ecs::event::EventWriter<AttackIntent>,
    game_tick: Option<Res<GameTick>>,
) {
    let tick = game_tick.as_deref().map(|tick| tick.0).unwrap_or(0);
    for (Actor(actor), mut state) in &mut actions {
        match *state {
            ActionState::Requested => {
                let Ok((mut bb, profile, mut navigator, marker)) = npcs.get_mut(*actor) else {
                    *state = ActionState::Failure;
                    continue;
                };
                navigator.stop();
                let cooldown = match marker.phase {
                    0 => 40,
                    1 => 60,
                    _ => 80,
                };
                if tick.wrapping_sub(bb.last_melee_tick) >= cooldown {
                    bb.last_melee_tick = tick;
                    if let Some(target) = bb.nearest_player {
                        attack_intents.send(AttackIntent {
                            attacker: *actor,
                            target: Some(target),
                            issued_at_tick: u64::from(tick),
                            reach: profile.reach,
                            qi_invest: match marker.phase {
                                0 => 12.0,
                                1 => 18.0,
                                _ => 30.0,
                            },
                            wound_kind: profile.wound_kind,
                            source: AttackSource::Melee,
                            debug_command: None,
                        });
                    }
                }
                *state = ActionState::Success;
            }
            ActionState::Cancelled => *state = ActionState::Failure,
            ActionState::Init
            | ActionState::Executing
            | ActionState::Success
            | ActionState::Failure => {}
        }
    }
}

fn fuya_enrage_scorer_system(
    npcs: Query<(&Wounds, Option<&FuyaEnragedMarker>), With<NpcMarker>>,
    mut scorers: Query<(&Actor, &mut Score), With<FuyaEnrageScorer>>,
) {
    for (Actor(actor), mut score) in &mut scorers {
        let value = if let Ok((wounds, enraged)) = npcs.get(*actor) {
            if enraged.is_some() || wounds.health_max <= f32::EPSILON {
                0.0
            } else if wounds.health_current / wounds.health_max < 0.3 {
                1.0
            } else {
                0.0
            }
        } else {
            0.0
        };
        score.set(value);
    }
}

#[allow(clippy::type_complexity)]
fn fuya_charge_scorer_system(
    npcs: Query<
        (
            &NpcBlackboard,
            &MovementCapabilities,
            &MovementCooldowns,
            &MovementController,
            Option<&FuyaEnragedMarker>,
        ),
        With<NpcMarker>,
    >,
    mut scorers: Query<(&Actor, &mut Score), With<FuyaChargeScorer>>,
    game_tick: Option<Res<GameTick>>,
) {
    let tick = game_tick.as_deref().map(|tick| tick.0).unwrap_or(0);
    for (Actor(actor), mut score) in &mut scorers {
        let value = if let Ok((bb, caps, cooldowns, ctrl, enraged)) = npcs.get(*actor) {
            if enraged.is_none()
                || !caps.can_dash
                || tick < cooldowns.dash_ready_at
                || ctrl.navigator_should_yield()
                || bb.player_distance < FUYA_CHARGE_MIN_RANGE
                || bb.player_distance > FUYA_CHARGE_MAX_RANGE
            {
                0.0
            } else {
                0.95
            }
        } else {
            0.0
        };
        score.set(value);
    }
}

fn fuya_enrage_action_system(
    mut commands: Commands,
    npcs: Query<&Position, With<NpcMarker>>,
    mut actions: Query<(&Actor, &mut ActionState), With<FuyaEnrageAction>>,
    mut audio_events: EventWriter<PlaySoundRecipeRequest>,
) {
    for (Actor(actor), mut state) in &mut actions {
        match *state {
            ActionState::Requested => {
                commands.entity(*actor).insert(FuyaEnragedMarker);
                if let Ok(position) = npcs.get(*actor) {
                    audio_events.send(play_audio("fauna_fuya_charge", position.get(), 1.0, 0.0));
                }
                *state = ActionState::Success;
            }
            ActionState::Cancelled => *state = ActionState::Failure,
            ActionState::Init
            | ActionState::Executing
            | ActionState::Success
            | ActionState::Failure => {}
        }
    }
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn handle_npc_death_drop(
    mut commands: Commands,
    mut events: EventReader<crate::combat::events::DeathEvent>,
    npcs: Query<
        (
            &NpcArchetype,
            &Position,
            Option<&TsyHostileMarker>,
            Option<&TsySentinelMarker>,
            Option<&crate::world::tsy_lifecycle::DaoxiangOrigin>,
            Option<&TsyNpcDropIssued>,
        ),
        With<NpcMarker>,
    >,
    containers: Query<&LootContainer>,
    drop_tables: Option<Res<TsyNpcDropTableRegistry>>,
    item_registry: Option<Res<ItemRegistry>>,
    relic_pool: Option<Res<AncientRelicPool>>,
    mut loot_registry: Option<ResMut<DroppedLootRegistry>>,
    mut allocator: Option<ResMut<InventoryInstanceIdAllocator>>,
) {
    let (
        Some(drop_tables),
        Some(item_registry),
        Some(relic_pool),
        Some(loot_registry),
        Some(allocator),
    ) = (
        drop_tables.as_deref(),
        item_registry.as_deref(),
        relic_pool.as_deref(),
        loot_registry.as_deref_mut(),
        allocator.as_deref_mut(),
    )
    else {
        return;
    };

    for event in events.read() {
        let Ok((archetype, pos, hostile, sentinel, daoxiang_origin, issued)) =
            npcs.get(event.target)
        else {
            continue;
        };
        if issued.is_some() {
            continue;
        }

        let Some(drop_key) = drop_key_for_npc(*archetype, sentinel) else {
            continue;
        };
        let Some(entry) = drop_tables.get(drop_key) else {
            continue;
        };

        let family_id = hostile
            .map(|marker| marker.family_id.as_str())
            .or_else(|| sentinel.map(|marker| marker.family_id.as_str()))
            .or_else(|| daoxiang_origin.map(|origin| origin.from_family.as_str()))
            .unwrap_or("tsy_unknown");
        let guarding_kind = sentinel
            .and_then(|marker| marker.guarding_container)
            .and_then(|entity| containers.get(entity).ok())
            .map(|container| container.kind);
        let ctx = DropContext {
            source_class: source_class_for_family(family_id),
            guarding_container_kind: guarding_kind,
        };
        let seed = stable_seed_u64(family_id, drop_key, event.at_tick, event.target.index());
        let items = roll_drop_entry(entry, &ctx, item_registry, relic_pool, allocator, seed);
        for (idx, item) in items.into_iter().enumerate() {
            let world_pos = jittered_drop_pos(pos.get(), seed, idx as u64);
            let instance_id = item.instance_id;
            loot_registry.entries.insert(
                instance_id,
                DroppedLootEntry {
                    instance_id,
                    source_container_id: format!("tsy_npc_drop:{family_id}:{drop_key}"),
                    source_row: 0,
                    source_col: 0,
                    world_pos,
                    dimension: DimensionKind::Tsy,
                    item,
                },
            );
        }
        commands.entity(event.target).insert(TsyNpcDropIssued);
    }
}

fn drop_key_for_npc(
    archetype: NpcArchetype,
    sentinel: Option<&TsySentinelMarker>,
) -> Option<&'static str> {
    match archetype {
        NpcArchetype::Daoxiang => Some("daoxiang"),
        NpcArchetype::Zhinian => Some("zhinian"),
        NpcArchetype::GuardianRelic if sentinel.is_some() => Some("tsy_sentinel"),
        NpcArchetype::Fuya => Some("fuya"),
        _ => None,
    }
}

struct DropContext {
    source_class: AncientRelicSource,
    guarding_container_kind: Option<ContainerKind>,
}

fn roll_drop_entry(
    entry: &TsyDropTableEntry,
    ctx: &DropContext,
    item_registry: &ItemRegistry,
    relic_pool: &AncientRelicPool,
    allocator: &mut InventoryInstanceIdAllocator,
    seed: u64,
) -> Vec<ItemInstance> {
    let mut rng = SeedRng::new(seed);
    let mut out = Vec::new();
    for roll in &entry.guaranteed {
        out.extend(resolve_drop_roll(
            roll,
            ctx,
            item_registry,
            relic_pool,
            allocator,
            &mut rng,
        ));
    }

    let mut hits = 0u32;
    for roll in &entry.rolls {
        if entry.max_rolls > 0 && hits >= entry.max_rolls {
            break;
        }
        if rng.unit_f32() <= roll.chance.clamp(0.0, 1.0) {
            out.extend(resolve_drop_roll(
                roll,
                ctx,
                item_registry,
                relic_pool,
                allocator,
                &mut rng,
            ));
            hits = hits.saturating_add(1);
        }
    }
    out
}

fn resolve_drop_roll(
    roll: &TsyDropRoll,
    ctx: &DropContext,
    item_registry: &ItemRegistry,
    relic_pool: &AncientRelicPool,
    allocator: &mut InventoryInstanceIdAllocator,
    rng: &mut SeedRng,
) -> Vec<ItemInstance> {
    let count = rng.range_u32(roll.count.0, roll.count.1).max(1);
    match roll.template_id.as_str() {
        "__ancient_relic_random__" => (0..count)
            .filter_map(|_| {
                let template = relic_pool.sample(ctx.source_class, rng.next_u64())?;
                template.to_item_instance(allocator).ok()
            })
            .collect(),
        "__origin_keyed_key__" => ctx
            .guarding_container_kind
            .and_then(|kind| match kind {
                ContainerKind::RelicCore => Some("key_array_core"),
                ContainerKind::StoneCasket => Some("key_stone_casket"),
                _ => None,
            })
            .and_then(|template_id| template_as_item(template_id, count, item_registry, allocator))
            .into_iter()
            .collect(),
        template_id => template_as_item(template_id, count, item_registry, allocator)
            .into_iter()
            .collect(),
    }
}

fn template_as_item(
    template_id: &str,
    count: u32,
    item_registry: &ItemRegistry,
    allocator: &mut InventoryInstanceIdAllocator,
) -> Option<ItemInstance> {
    let template = item_registry.get(template_id)?;
    let instance_id = allocator.next_id().ok()?;
    Some(ItemInstance {
        instance_id,
        template_id: template.id.clone(),
        display_name: template.display_name.clone(),
        grid_w: template.grid_w,
        grid_h: template.grid_h,
        weight: template.base_weight,
        rarity: template.rarity,
        description: template.description.clone(),
        stack_count: count,
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
    })
}

fn source_class_for_family(family_id: &str) -> AncientRelicSource {
    match TsyOrigin::from_zone_name(family_id) {
        Some(TsyOrigin::DanengLuoluo) => AncientRelicSource::DaoLord,
        Some(TsyOrigin::ZhanchangChendian) => AncientRelicSource::BattleSediment,
        Some(TsyOrigin::ZongmenYiji | TsyOrigin::GaoshouShichu) | None => {
            AncientRelicSource::SectRuins
        }
    }
}

fn jittered_drop_pos(pos: DVec3, seed: u64, idx: u64) -> [f64; 3] {
    let jx = (((seed.wrapping_add(idx * 17) & 0xFF) as f64) / 255.0 - 0.5) * 0.5;
    let jz = ((((seed >> 8).wrapping_add(idx * 31) & 0xFF) as f64) / 255.0 - 0.5) * 0.5;
    [pos.x + jx, pos.y, pos.z + jz]
}

fn sample_hostile_position(
    zone: &Zone,
    family_id: &str,
    depth: TsyDepth,
    kind: &str,
    index: u32,
    tick: u64,
) -> Option<DVec3> {
    let seed = stable_seed_u64(family_id, kind, tick, ((depth as u32) << 16) | index);
    let anchor = if zone.patrol_anchors.is_empty() {
        zone.center()
    } else {
        zone.patrol_anchors[index as usize % zone.patrol_anchors.len()]
    };
    let x = (((seed & 0xFFF) as f64) / 4096.0 - 0.5) * 12.0;
    let z = ((((seed >> 16) & 0xFFF) as f64) / 4096.0 - 0.5) * 12.0;
    let candidate = zone.clamp_position(DVec3::new(anchor.x + x, anchor.y, anchor.z + z));
    let tile = (candidate.x.floor() as i32, candidate.z.floor() as i32);
    if zone.blocked_tiles.contains(&tile) {
        None
    } else {
        Some(candidate)
    }
}

fn depth_suffix(depth: TsyDepth) -> &'static str {
    match depth {
        TsyDepth::Shallow => "shallow",
        TsyDepth::Mid => "mid",
        TsyDepth::Deep => "deep",
    }
}

fn stable_seed_u64(a: &str, b: &str, tick: u64, index: u32) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    a.hash(&mut hasher);
    b.hash(&mut hasher);
    tick.hash(&mut hasher);
    index.hash(&mut hasher);
    hasher.finish()
}

struct SeedRng {
    state: u64,
}

impl SeedRng {
    fn new(seed: u64) -> Self {
        Self {
            state: seed.wrapping_add(0x9E37_79B9_7F4A_7C15),
        }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn unit_f32(&mut self) -> f32 {
        ((self.next_u64() >> 40) & 0x00FF_FFFF) as f32 / (1u32 << 24) as f32
    }

    fn range_u32(&mut self, min: u32, max: u32) -> u32 {
        if max <= min {
            return min;
        }
        min + (self.next_u64() % u64::from(max - min + 1)) as u32
    }
}

#[derive(Deserialize)]
struct TsySpawnPoolsJson {
    by_origin: HashMap<String, TsyOriginSpawnPoolJson>,
    #[serde(default)]
    sentinel_count_by_origin: HashMap<String, u32>,
}

#[derive(Deserialize)]
struct TsyOriginSpawnPoolJson {
    shallow: TsyLayerSpawnCountsJson,
    mid: TsyLayerSpawnCountsJson,
    deep: TsyLayerSpawnCountsJson,
}

impl TsyOriginSpawnPoolJson {
    fn into_pool(self) -> TsyOriginSpawnPool {
        TsyOriginSpawnPool {
            shallow: self.shallow.into_counts(),
            mid: self.mid.into_counts(),
            deep: self.deep.into_counts(),
        }
    }
}

#[derive(Deserialize)]
struct TsyLayerSpawnCountsJson {
    #[serde(default)]
    daoxiang: u32,
    #[serde(default)]
    zhinian: u32,
    #[serde(default)]
    fuya: u32,
}

impl TsyLayerSpawnCountsJson {
    fn into_counts(self) -> TsyLayerSpawnCounts {
        TsyLayerSpawnCounts {
            daoxiang: self.daoxiang,
            zhinian: self.zhinian,
            fuya: self.fuya,
        }
    }
}

#[derive(Deserialize)]
struct TsyDropsJson {
    #[serde(flatten)]
    tables: HashMap<String, TsyDropTableEntryJson>,
}

#[derive(Deserialize)]
struct TsyDropTableEntryJson {
    #[serde(default)]
    guaranteed: Vec<TsyDropRollJson>,
    #[serde(default)]
    rolls: Vec<TsyDropRollJson>,
    #[serde(default)]
    max_rolls: u32,
}

impl TsyDropTableEntryJson {
    fn into_entry(self) -> TsyDropTableEntry {
        TsyDropTableEntry {
            guaranteed: self
                .guaranteed
                .into_iter()
                .map(TsyDropRollJson::into_roll)
                .collect(),
            rolls: self
                .rolls
                .into_iter()
                .map(TsyDropRollJson::into_roll)
                .collect(),
            max_rolls: self.max_rolls,
        }
    }
}

#[derive(Deserialize)]
struct TsyDropRollJson {
    template_id: String,
    #[serde(default = "default_chance")]
    chance: f32,
    count: [u32; 2],
}

impl TsyDropRollJson {
    fn into_roll(self) -> TsyDropRoll {
        let min = self.count[0].max(1);
        let max = self.count[1].max(min);
        TsyDropRoll {
            template_id: self.template_id,
            chance: self.chance.clamp(0.0, 1.0),
            count: (min, max),
        }
    }
}

fn default_chance() -> f32 {
    1.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::{ItemCategory, ItemRarity, ItemTemplate};

    fn template(id: &str) -> ItemTemplate {
        ItemTemplate {
            id: id.to_string(),
            display_name: id.to_string(),
            category: ItemCategory::Misc,
            max_stack_count: 1,
            grid_w: 1,
            grid_h: 1,
            base_weight: 0.1,
            rarity: ItemRarity::Common,
            spirit_quality_initial: 0.0,
            description: id.to_string(),
            effect: None,
            cast_duration_ms: crate::inventory::DEFAULT_CAST_DURATION_MS,
            cooldown_ms: crate::inventory::DEFAULT_COOLDOWN_MS,
            weapon_spec: None,
            forge_station_spec: None,
            blueprint_scroll_spec: None,
            inscription_scroll_spec: None,
        }
    }

    #[test]
    fn default_spawn_pool_matches_plan_counts() {
        let registry = load_tsy_spawn_pool_registry().expect("spawn pools should parse");
        let zongmen = registry
            .get_for_family("tsy_zongmen_01")
            .expect("zongmen pool exists");
        assert_eq!(zongmen.shallow.daoxiang, 3);
        assert_eq!(zongmen.mid.zhinian, 1);
        assert_eq!(zongmen.deep.daoxiang, 8);
        assert_eq!(registry.sentinel_count_for_family("tsy_zongmen_01"), 3);
        assert_eq!(registry.sentinel_count_for_family("tsy_zhanchang_01"), 0);
    }

    #[test]
    fn tsy_hostile_spawns_use_custom_visual_entity_kinds() {
        let scenario = valence::testing::ScenarioSingleClient::new();
        let layer = scenario.layer;
        let mut app = scenario.app;

        let daoxiang = spawn_tsy_daoxiang_at(
            &mut app.world_mut().commands(),
            layer,
            "tsy_zongmen_01",
            "tsy_zongmen_01_shallow",
            DVec3::new(1.0, 64.0, 1.0),
            DVec3::new(2.0, 64.0, 2.0),
        );
        let zhinian = spawn_tsy_zhinian_at(
            &mut app.world_mut().commands(),
            layer,
            "tsy_zongmen_01",
            "tsy_zongmen_01_mid",
            DVec3::new(3.0, 64.0, 3.0),
            DVec3::new(4.0, 64.0, 4.0),
        );
        let fuya = spawn_tsy_fuya_at(
            &mut app.world_mut().commands(),
            layer,
            "tsy_zongmen_01",
            "tsy_zongmen_01_deep",
            DVec3::new(5.0, 64.0, 5.0),
            DVec3::new(6.0, 64.0, 6.0),
        );
        let container = app.world_mut().spawn_empty().id();
        let sentinel = spawn_tsy_sentinel_at(
            &mut app.world_mut().commands(),
            layer,
            "tsy_zongmen_01",
            "tsy_zongmen_01_deep",
            DVec3::new(7.0, 64.0, 7.0),
            container,
        );
        app.world_mut().flush();

        assert_eq!(
            app.world().get::<valence::prelude::EntityKind>(daoxiang),
            Some(&DAOXIANG_ENTITY_KIND)
        );
        assert_eq!(
            app.world().get::<valence::prelude::EntityKind>(zhinian),
            Some(&ZHINIAN_ENTITY_KIND)
        );
        assert_eq!(
            app.world().get::<valence::prelude::EntityKind>(fuya),
            Some(&FUYA_ENTITY_KIND)
        );
        assert_eq!(
            app.world().get::<valence::prelude::EntityKind>(sentinel),
            Some(&TSY_SENTINEL_ENTITY_KIND)
        );
        assert_eq!(
            app.world().get::<FaunaVisualKind>(fuya),
            Some(&FaunaVisualKind::Fuya)
        );
    }

    #[test]
    fn fuya_pressure_hum_audio_emits_on_aura_spawn() {
        let mut app = valence::prelude::App::new();
        app.add_event::<PlaySoundRecipeRequest>();
        app.add_systems(
            valence::prelude::Update,
            emit_fuya_pressure_hum_audio_system,
        );
        let fuya = app
            .world_mut()
            .spawn((Position::new([0.0, 64.0, 0.0]), FuyaAura::default()))
            .id();

        app.update();

        let events = app
            .world()
            .resource::<valence::prelude::Events<PlaySoundRecipeRequest>>();
        let event = events
            .iter_current_update_events()
            .next()
            .expect("new Fuya aura should emit pressure hum");
        assert_eq!(event.recipe_id, "fauna_fuya_pressure_hum");
        assert_eq!(event.instance_id, fuya_pressure_audio_instance_id(fuya));
        assert_eq!(
            event.flag.as_deref(),
            Some(fuya_pressure_audio_flag(fuya).as_str())
        );
    }

    #[test]
    fn fuya_pressure_hum_stops_on_death() {
        let mut app = valence::prelude::App::new();
        app.add_event::<crate::combat::events::DeathEvent>();
        app.add_event::<StopSoundRecipeRequest>();
        app.add_systems(
            valence::prelude::Update,
            stop_fuya_pressure_hum_audio_on_death_system,
        );
        let fuya = app
            .world_mut()
            .spawn((Position::new([0.0, 64.0, 0.0]), FuyaAura::default()))
            .id();
        app.world_mut()
            .send_event(crate::combat::events::DeathEvent {
                target: fuya,
                cause: "test".to_string(),
                attacker: None,
                attacker_player_id: None,
                at_tick: 1,
            });

        app.update();

        let events = app
            .world()
            .resource::<valence::prelude::Events<StopSoundRecipeRequest>>();
        let event = events
            .iter_current_update_events()
            .next()
            .expect("Fuya death should stop pressure hum loop");
        assert_eq!(event.instance_id, fuya_pressure_audio_instance_id(fuya));
        assert_eq!(event.fade_out_ticks, 20);
    }

    #[test]
    fn fuya_aura_multiplier_stacks_multiplicatively() {
        let aura = FuyaAura::default();
        let a = Position::new([0.0, 64.0, 0.0]);
        let b = Position::new([3.0, 64.0, 0.0]);
        let c = Position::new([30.0, 64.0, 0.0]);
        let multiplier = compute_fuya_aura_drain_multiplier(
            DVec3::new(1.0, 64.0, 0.0),
            [(&a, &aura), (&b, &aura), (&c, &aura)],
        );
        assert!((multiplier - 2.25).abs() < 1e-9);
    }

    #[test]
    fn fuya_aura_multiplier_returns_one_without_hits() {
        let aura = FuyaAura::default();
        let far = Position::new([100.0, 64.0, 0.0]);
        let multiplier =
            compute_fuya_aura_drain_multiplier(DVec3::new(1.0, 64.0, 0.0), [(&far, &aura)]);
        assert!((multiplier - 1.0).abs() < 1e-9);

        let empty: [(&Position, &FuyaAura); 0] = [];
        let multiplier = compute_fuya_aura_drain_multiplier(DVec3::ZERO, empty);
        assert!((multiplier - 1.0).abs() < 1e-9);
    }

    #[test]
    fn fuya_aura_multiplier_pins_nonlinear_three_stack() {
        let aura = FuyaAura::default();
        let a = Position::new([0.0, 64.0, 0.0]);
        let b = Position::new([1.0, 64.0, 0.0]);
        let c = Position::new([2.0, 64.0, 0.0]);
        let multiplier = compute_fuya_aura_drain_multiplier(
            DVec3::new(1.0, 64.0, 0.0),
            [(&a, &aura), (&b, &aura), (&c, &aura)],
        );
        assert!((multiplier - 3.375).abs() < 1e-9);
    }

    #[test]
    fn fuya_aura_multiplier_ignores_zero_radius_and_clamps_sub_one_boost() {
        let zero_radius = FuyaAura {
            radius_blocks: 0.0,
            drain_boost_multiplier: 99.0,
        };
        let weakening = FuyaAura {
            radius_blocks: 8.0,
            drain_boost_multiplier: 0.25,
        };
        let pos = Position::new([0.0, 64.0, 0.0]);
        let multiplier = compute_fuya_aura_drain_multiplier(
            DVec3::new(0.0, 64.0, 0.0),
            [(&pos, &zero_radius), (&pos, &weakening)],
        );
        assert!((multiplier - 1.0).abs() < 1e-9);
    }

    #[test]
    fn sentinel_phase_thresholds_match_three_stage_health_bands() {
        let mut wounds = Wounds {
            entries: Vec::new(),
            health_current: 100.0,
            health_max: 100.0,
        };
        assert_eq!(sentinel_phase_for_wounds(&wounds, 3), 0);
        wounds.health_current = 66.0;
        assert_eq!(sentinel_phase_for_wounds(&wounds, 3), 1);
        wounds.health_current = 67.0;
        assert_eq!(sentinel_phase_for_wounds(&wounds, 3), 0);
        wounds.health_current = 33.0;
        assert_eq!(sentinel_phase_for_wounds(&wounds, 3), 2);
        wounds.health_current = 0.0;
        assert_eq!(sentinel_phase_for_wounds(&wounds, 3), 2);
    }

    #[test]
    fn sentinel_phase_clamps_to_max_phase_minus_one() {
        let wounds = Wounds {
            entries: Vec::new(),
            health_current: 0.0,
            health_max: 100.0,
        };
        assert_eq!(sentinel_phase_for_wounds(&wounds, 2), 1);
        assert_eq!(sentinel_phase_for_wounds(&wounds, 1), 0);
    }

    #[test]
    fn daoxiang_instinct_requires_opening_and_cooldown() {
        let profile = NpcMeleeProfile::fist();
        assert_eq!(
            daoxiang_instinct_score(0.8, &profile, 0.1, false, true),
            1.0
        );
        assert_eq!(daoxiang_instinct_score(0.8, &profile, 0.9, true, true), 1.0);
        assert_eq!(
            daoxiang_instinct_score(0.8, &profile, 0.9, false, true),
            0.0
        );
        assert_eq!(
            daoxiang_instinct_score(0.8, &profile, 0.1, false, false),
            0.0
        );
    }

    #[test]
    fn origin_keyed_key_resolves_relic_core_key() {
        let item_registry = ItemRegistry::from_map(HashMap::from([(
            "key_array_core".to_string(),
            template("key_array_core"),
        )]));
        let relic_pool = AncientRelicPool::from_seed();
        let mut allocator = InventoryInstanceIdAllocator::default();
        let mut rng = SeedRng::new(1);
        let ctx = DropContext {
            source_class: AncientRelicSource::SectRuins,
            guarding_container_kind: Some(ContainerKind::RelicCore),
        };
        let roll = TsyDropRoll {
            template_id: "__origin_keyed_key__".to_string(),
            chance: 1.0,
            count: (1, 1),
        };
        let items = resolve_drop_roll(
            &roll,
            &ctx,
            &item_registry,
            &relic_pool,
            &mut allocator,
            &mut rng,
        );
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].template_id, "key_array_core");
    }
}
