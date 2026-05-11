use serde::{Deserialize, Serialize};
use valence::prelude::{
    bevy_ecs, Component, Entity, EventReader, EventWriter, Events, Position, Query, Res, ResMut,
};

use super::components::Realm;
use super::death_hooks::{CultivationDeathCause, CultivationDeathTrigger};
use super::tick::CultivationClock;
use crate::combat::components::{ActiveStatusEffect, Lifecycle, LifecycleState, StatusEffects};
use crate::combat::events::StatusEffectKind;
use crate::cultivation::components::Cultivation;
use crate::cultivation::life_record::{BiographyEntry, LifeRecord};
use crate::persistence::{persist_lifespan_event, LifespanEventRecord, PersistenceSettings};
use crate::player::gameplay::PendingGameplayNarrations;
use crate::player::state::{
    player_username_from_character_id, PlayerState, PlayerStatePersistence,
};
use crate::schema::common::NarrationStyle;
use crate::schema::death_lifecycle::{
    AgingEventKindV1, AgingEventV1, LifespanEventKindV1, LifespanEventV1,
};
use crate::world::season::{query_season, Season};
use crate::world::zone::{Zone, ZoneRegistry};

use super::tick::frailty_qi_recovery_multiplier_for_realm;

pub const KARMA_REBIRTH_THRESHOLD: f64 = 0.5;
pub const REBIRTH_SAFE_WINDOW_TICKS: u64 = 24 * 60 * 60 * 20;
pub const REBIRTH_BASE_CHANCE: f64 = 0.80;
pub const REBIRTH_STEP_PER_DEATH: f64 = 0.15;
pub const REBIRTH_MIN_CHANCE: f64 = 0.05;
pub const WIND_CANDLE_THRESHOLD: f64 = 0.10;
pub const LIFESPAN_TICKS_PER_YEAR: u64 = 60 * 60 * 20;
pub const LIFESPAN_SECONDS_PER_YEAR: u64 = 60 * 60;
pub const LIFESPAN_ONLINE_MULTIPLIER: f64 = 1.0;
pub const LIFESPAN_NEGATIVE_ZONE_MULTIPLIER: f64 = 2.0;
pub const LIFESPAN_OFFLINE_MULTIPLIER: f64 = 0.1;
pub const LIFESPAN_WIND_CANDLE_NARRATION_INTERVAL_TICKS: u64 = REBIRTH_SAFE_WINDOW_TICKS;
pub const LIFESPAN_EXTENSION_HARD_CAP_FACTOR: f64 = 2.0;
pub const LIFESPAN_EXTENSION_PILL_QI_MAX_COST_PER_YEAR: f64 = 0.01;
pub const LIFESPAN_ENLIGHTENMENT_TARGET_REMAINING_RATIO: f64 = 0.30;

const UNASSIGNED_DEATH_REGISTRY_ID: &str = "unassigned:death_registry";
const NEGATIVE_ZONE_SPIRIT_QI_THRESHOLD: f64 = -0.2;
const ENLIGHTENMENT_EXTENSION_MARKER: &str = "lifespan_extension:enlightenment_used";

#[derive(Debug, Clone, bevy_ecs::event::Event, Serialize, Deserialize, PartialEq)]
pub struct LifespanEventEmitted {
    pub payload: LifespanEventV1,
}

#[derive(Debug, Clone, bevy_ecs::event::Event, Serialize, Deserialize, PartialEq)]
pub struct AgingEventEmitted {
    pub payload: AgingEventV1,
}

#[derive(Debug, Clone, bevy_ecs::event::Event, PartialEq)]
pub struct LifespanExtensionIntent {
    pub entity: Entity,
    pub requested_years: u32,
    pub source: String,
}

#[derive(Debug, Clone, Component, Default, Serialize, Deserialize, PartialEq)]
pub struct LifespanExtensionLedger {
    pub accumulated_years: f64,
    #[serde(default)]
    pub enlightenment_used: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ExtensionCost {
    pub karma_delta: f64,
    pub qi_cap_delta: f64,
    pub realm_progress_delta: f64,
    pub enlightenment_slot: bool,
}

pub trait ExtensionContract {
    fn source(&self) -> &'static str;
    fn requested_years(&self, lifespan: &LifespanComponent) -> u32;
    fn cost(&self, years: u32, accumulated_years: f64, cap_by_realm: u32) -> ExtensionCost {
        let pressure = lifespan_extension_cost_pressure(accumulated_years, cap_by_realm);
        ExtensionCost {
            qi_cap_delta: -(years as f64 * self.qi_cap_cost_factor() * pressure),
            enlightenment_slot: self.consumes_enlightenment(),
            ..Default::default()
        }
    }
    fn qi_cap_cost_factor(&self) -> f64 {
        0.0
    }
    fn consumes_enlightenment(&self) -> bool {
        false
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PillExtensionContract {
    pub years: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EnlightenmentExtensionContract;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CollapseCoreExtensionContract {
    pub years: u32,
}

impl ExtensionContract for PillExtensionContract {
    fn source(&self) -> &'static str {
        "life_extension_pill"
    }

    fn requested_years(&self, _lifespan: &LifespanComponent) -> u32 {
        self.years
    }

    fn qi_cap_cost_factor(&self) -> f64 {
        LIFESPAN_EXTENSION_PILL_QI_MAX_COST_PER_YEAR
    }
}

impl ExtensionContract for EnlightenmentExtensionContract {
    fn source(&self) -> &'static str {
        "enlightenment_extension"
    }

    fn requested_years(&self, lifespan: &LifespanComponent) -> u32 {
        let target_remaining =
            lifespan.cap_by_realm as f64 * LIFESPAN_ENLIGHTENMENT_TARGET_REMAINING_RATIO;
        (target_remaining - lifespan.remaining_years())
            .ceil()
            .max(0.0) as u32
    }

    fn consumes_enlightenment(&self) -> bool {
        true
    }
}

impl ExtensionContract for CollapseCoreExtensionContract {
    fn source(&self) -> &'static str {
        "collapse_core"
    }

    fn requested_years(&self, _lifespan: &LifespanComponent) -> u32 {
        self.years
    }

    fn cost(&self, years: u32, accumulated_years: f64, cap_by_realm: u32) -> ExtensionCost {
        let pressure = lifespan_extension_cost_pressure(accumulated_years, cap_by_realm);
        ExtensionCost {
            realm_progress_delta: -(years as f64 * 100.0 * pressure).round(),
            ..Default::default()
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ZoneDeathKind {
    Ordinary,
    Death,
    Negative,
}

impl ZoneDeathKind {
    pub fn skips_fortune(self) -> bool {
        matches!(self, Self::Death | Self::Negative)
    }
}

#[derive(Debug, Clone, Component, Serialize, Deserialize, PartialEq)]
pub struct LifespanComponent {
    pub born_at_tick: u64,
    pub years_lived: f64,
    pub cap_by_realm: u32,
    #[serde(default)]
    pub offline_pause_tick: Option<u64>,
}

impl Default for LifespanComponent {
    fn default() -> Self {
        Self::new(LifespanCapTable::MORTAL)
    }
}

impl LifespanComponent {
    pub fn new(cap_by_realm: u32) -> Self {
        Self {
            born_at_tick: 0,
            years_lived: 0.0,
            cap_by_realm,
            offline_pause_tick: None,
        }
    }

    pub fn for_realm(realm: Realm) -> Self {
        Self::new(LifespanCapTable::for_realm(realm))
    }

    pub fn remaining_years(&self) -> f64 {
        (self.cap_by_realm as f64 - self.years_lived).max(0.0)
    }

    pub fn is_wind_candle(&self) -> bool {
        self.remaining_years() <= self.cap_by_realm as f64 * WIND_CANDLE_THRESHOLD
    }

    pub fn apply_cap(&mut self, cap_by_realm: u32) {
        self.cap_by_realm = cap_by_realm;
    }
}

pub struct LifespanCapTable;

impl LifespanCapTable {
    pub const MORTAL: u32 = 80;
    pub const AWAKEN: u32 = 120;
    pub const INDUCE: u32 = 200;
    pub const CONDENSE: u32 = 350;
    pub const SOLIDIFY: u32 = 600;
    pub const SPIRIT: u32 = 1000;
    pub const VOID: u32 = 2000;

    pub fn for_realm(realm: Realm) -> u32 {
        match realm {
            Realm::Awaken => Self::AWAKEN,
            Realm::Induce => Self::INDUCE,
            Realm::Condense => Self::CONDENSE,
            Realm::Solidify => Self::SOLIDIFY,
            Realm::Spirit => Self::SPIRIT,
            Realm::Void => Self::VOID,
        }
    }

    pub fn for_player_state_realm(player_realm: Option<&str>, cultivation_realm: Realm) -> u32 {
        match player_realm {
            Some("mortal") => Self::MORTAL,
            _ => Self::for_realm(cultivation_realm),
        }
    }

    pub fn death_penalty_years_for_cap(cap_by_realm: u32) -> u32 {
        cap_by_realm / 20
    }

    pub fn death_penalty_years_for_realm(realm: Realm) -> u32 {
        Self::death_penalty_years_for_cap(Self::for_realm(realm))
    }
}

#[derive(Debug, Clone, Component, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeathRegistry {
    pub char_id: String,
    pub death_count: u32,
    #[serde(default)]
    pub last_death_tick: Option<u64>,
    /// 上一次死亡的 tick（不含当前已记录的死亡）。
    ///
    /// 用于实现 plan-death-lifecycle-v1 §2 的「死前 24h 内未死过」判定。
    /// 当 includes_current_death=true 时，last_death_tick 代表“当前死亡”，
    /// 需回看 prev_death_tick 才能判断前一次死亡是否发生在 24h 之外。
    #[serde(default)]
    pub prev_death_tick: Option<u64>,
    #[serde(default)]
    pub last_death_zone: Option<ZoneDeathKind>,
}

impl Default for DeathRegistry {
    fn default() -> Self {
        Self::new(UNASSIGNED_DEATH_REGISTRY_ID)
    }
}

impl DeathRegistry {
    pub fn new(char_id: impl Into<String>) -> Self {
        Self {
            char_id: char_id.into(),
            death_count: 0,
            last_death_tick: None,
            prev_death_tick: None,
            last_death_zone: None,
        }
    }

    pub fn next_death_number(&self) -> u32 {
        self.death_count.saturating_add(1)
    }

    pub fn record_death(&mut self, at_tick: u64, zone: ZoneDeathKind) {
        self.death_count = self.death_count.saturating_add(1);
        self.prev_death_tick = self.last_death_tick;
        self.last_death_tick = Some(at_tick);
        self.last_death_zone = Some(zone);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RebirthChanceInput {
    pub registry: DeathRegistry,
    pub at_tick: u64,
    pub death_zone: ZoneDeathKind,
    pub karma: f64,
    pub has_shrine: bool,
    #[serde(default)]
    pub includes_current_death: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RebirthStage {
    Fortune,
    Tribulation,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RebirthChanceResult {
    pub death_number: u32,
    pub stage: RebirthStage,
    pub chance: f64,
    pub guaranteed: bool,
    pub fortune_charge_cost: u8,
    pub skip_fortune_due_to_zone: bool,
    pub no_recent_death: bool,
    pub low_karma: bool,
    pub has_shrine: bool,
}

pub fn calculate_rebirth_chance(input: &RebirthChanceInput) -> RebirthChanceResult {
    let death_number = if input.includes_current_death {
        input.registry.death_count.max(1)
    } else {
        input.registry.next_death_number()
    };
    let skip_fortune_due_to_zone = input.death_zone.skips_fortune();
    // 若 registry 已包含“当前死亡”（includes_current_death=true），last_death_tick
    // 指向当前 at_tick，不能用于判断“死前 24h 内是否死过”。此时应回看 prev_death_tick。
    let last_tick_for_window = if input.includes_current_death {
        input.registry.prev_death_tick
    } else {
        input.registry.last_death_tick
    };
    let no_recent_death = last_tick_for_window
        .is_none_or(|tick| input.at_tick.saturating_sub(tick) >= REBIRTH_SAFE_WINDOW_TICKS);
    let low_karma = input.karma < KARMA_REBIRTH_THRESHOLD;
    let guaranteed = death_number <= 3
        && !skip_fortune_due_to_zone
        && (no_recent_death || low_karma || input.has_shrine);

    if guaranteed {
        return RebirthChanceResult {
            death_number,
            stage: RebirthStage::Fortune,
            chance: 1.0,
            guaranteed: true,
            fortune_charge_cost: 1,
            skip_fortune_due_to_zone,
            no_recent_death,
            low_karma,
            has_shrine: input.has_shrine,
        };
    }

    let chance = tribulation_rebirth_chance(death_number);
    RebirthChanceResult {
        death_number,
        stage: RebirthStage::Tribulation,
        chance,
        guaranteed: false,
        fortune_charge_cost: 0,
        skip_fortune_due_to_zone,
        no_recent_death,
        low_karma,
        has_shrine: input.has_shrine,
    }
}

pub fn tribulation_rebirth_chance(death_number: u32) -> f64 {
    let step = death_number.saturating_sub(3) as f64;
    (REBIRTH_BASE_CHANCE - REBIRTH_STEP_PER_DEATH * step)
        .clamp(REBIRTH_MIN_CHANCE, REBIRTH_BASE_CHANCE)
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn lifespan_aging_tick(
    clock: Res<CultivationClock>,
    persistence: Option<Res<PersistenceSettings>>,
    zones: Option<Res<ZoneRegistry>>,
    mut pending_narrations: Option<ResMut<PendingGameplayNarrations>>,
    mut lifespan_events: Option<ResMut<Events<LifespanEventEmitted>>>,
    mut aging_events: Option<ResMut<Events<AgingEventEmitted>>>,
    mut deaths: EventWriter<CultivationDeathTrigger>,
    mut actors: Query<(
        Entity,
        &mut LifespanComponent,
        Option<&Cultivation>,
        Option<&PlayerState>,
        Option<&Position>,
        Option<&Lifecycle>,
        Option<&LifeRecord>,
        Option<&crate::npc::lifecycle::NpcArchetype>,
    )>,
) {
    let persistence = persistence.as_deref();
    let zones = zones.as_deref();

    for (
        entity,
        mut lifespan,
        cultivation,
        player_state,
        position,
        lifecycle,
        life_record,
        npc_archetype,
    ) in &mut actors
    {
        if npc_archetype.is_some_and(|archetype| !archetype.uses_lifespan_aging()) {
            continue;
        }
        if lifecycle.is_some_and(|lifecycle| lifecycle.state == LifecycleState::Terminated) {
            continue;
        }

        let previous_years_lived = lifespan.years_lived;
        let previous_remaining = lifespan.remaining_years();
        let cap = match npc_archetype.copied() {
            Some(crate::npc::lifecycle::NpcArchetype::Commoner) => LifespanCapTable::MORTAL,
            _ => lifespan_cap_for_actor(cultivation, player_state),
        };
        lifespan.apply_cap(cap);

        let multiplier = lifespan_tick_rate_multiplier(position, zones)
            * season_aging_modifier(lifespan_season(position, zones, clock.tick));
        let delta_years = lifespan_delta_years_for_ticks(1, multiplier);
        lifespan.years_lived =
            (lifespan.years_lived + delta_years).min(lifespan.cap_by_realm as f64);

        let crossed_years =
            lifespan_whole_years_crossed(previous_years_lived, lifespan.years_lived);
        if crossed_years > 0 {
            if let Some(char_id) = lifespan_event_char_id(life_record, lifecycle) {
                let event = LifespanEventRecord {
                    at_tick: clock.tick,
                    kind: "aging".to_string(),
                    delta_years: crossed_years,
                    source: lifespan_event_source(multiplier).to_string(),
                };
                emit_lifespan_event(lifespan_events.as_deref_mut(), char_id, &event);
                if (multiplier - LIFESPAN_ONLINE_MULTIPLIER).abs() > f64::EPSILON {
                    emit_aging_event(
                        aging_events.as_deref_mut(),
                        char_id,
                        AgingEventKindV1::TickRate,
                        clock.tick,
                        &lifespan,
                        multiplier,
                    );
                }
                if let Some(settings) = persistence {
                    if let Err(error) = persist_lifespan_event(settings, char_id, &event) {
                        tracing::warn!(
                            "[bong][lifespan] failed to persist aging event for {char_id}: {error}"
                        );
                    }
                }
            }
        }

        if should_emit_wind_candle_narration(&lifespan, clock.tick) {
            if let Some(char_id) = lifespan_event_char_id(life_record, lifecycle) {
                emit_aging_event(
                    aging_events.as_deref_mut(),
                    char_id,
                    AgingEventKindV1::WindCandle,
                    clock.tick,
                    &lifespan,
                    multiplier,
                );
            }
            if let (Some(pending_narrations), Some(target)) = (
                pending_narrations.as_deref_mut(),
                lifespan_narration_target(life_record, lifecycle),
            ) {
                pending_narrations.push_player(
                    target,
                    wind_candle_narration_text(&lifespan),
                    NarrationStyle::Perception,
                );
            }
        }

        if previous_remaining > f64::EPSILON && lifespan.remaining_years() <= f64::EPSILON {
            if let Some(char_id) = lifespan_event_char_id(life_record, lifecycle) {
                emit_aging_event(
                    aging_events.as_deref_mut(),
                    char_id,
                    AgingEventKindV1::NaturalDeath,
                    clock.tick,
                    &lifespan,
                    multiplier,
                );
            }
            deaths.send(CultivationDeathTrigger {
                entity,
                cause: CultivationDeathCause::NaturalAging,
                context: serde_json::json!({
                    "years_lived": lifespan.years_lived,
                    "cap_by_realm": lifespan.cap_by_realm,
                    "tick_rate_multiplier": multiplier,
                    "at_tick": clock.tick,
                }),
            });
        }
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn process_lifespan_extension_intents(
    clock: Res<CultivationClock>,
    persistence: Option<Res<PersistenceSettings>>,
    player_persistence: Option<Res<PlayerStatePersistence>>,
    mut intents: EventReader<LifespanExtensionIntent>,
    mut lifespan_events: Option<ResMut<Events<LifespanEventEmitted>>>,
    mut actors: Query<(
        &mut LifespanComponent,
        &mut LifespanExtensionLedger,
        Option<&mut Cultivation>,
        Option<&mut PlayerState>,
        Option<&mut LifeRecord>,
        Option<&Lifecycle>,
    )>,
) {
    let persistence = persistence.as_deref();
    let player_persistence = player_persistence.as_deref();

    for intent in intents.read() {
        let Ok((mut lifespan, mut ledger, cultivation, player_state, mut life_record, lifecycle)) =
            actors.get_mut(intent.entity)
        else {
            continue;
        };

        let realm_at_time = cultivation
            .as_deref()
            .map(|cultivation| cultivation.realm)
            .unwrap_or(crate::cultivation::components::Realm::Awaken);
        let contract =
            extension_contract_from_source(intent.source.as_str(), intent.requested_years);
        let requested_years = if intent.requested_years == 0 {
            contract.requested_years(&lifespan)
        } else {
            intent.requested_years
        };
        let accumulated_before = ledger.accumulated_years;
        let Some(applied_years) = apply_lifespan_extension(
            &mut lifespan,
            &mut ledger,
            requested_years,
            contract.consumes_enlightenment(),
        ) else {
            continue;
        };

        apply_extension_cost(
            contract.cost(applied_years, accumulated_before, lifespan.cap_by_realm),
            cultivation,
            player_state,
        );

        let event = LifespanEventRecord {
            at_tick: clock.tick,
            kind: "extension".to_string(),
            delta_years: i64::from(applied_years),
            source: contract.source().to_string(),
        };
        let char_id = lifespan_event_char_id(life_record.as_deref(), lifecycle).map(str::to_string);
        if let Some(char_id) = char_id.as_deref() {
            emit_lifespan_event(lifespan_events.as_deref_mut(), char_id, &event);
            if let Some(settings) = persistence {
                if let Err(error) = persist_lifespan_event(settings, char_id, &event) {
                    tracing::warn!(
                        "[bong][lifespan] failed to persist extension event for {char_id}: {error}"
                    );
                }
            }
        }
        if let Some(life_record) = life_record.as_deref_mut() {
            life_record.push(BiographyEntry::LifespanExtended {
                source: contract.source().to_string(),
                delta_years: i64::from(applied_years),
                tick: clock.tick,
            });
            if contract.consumes_enlightenment()
                && !life_record
                    .insights_taken
                    .iter()
                    .any(|insight| insight.trigger_id == ENLIGHTENMENT_EXTENSION_MARKER)
            {
                life_record
                    .insights_taken
                    .push(crate::cultivation::life_record::TakenInsight {
                        trigger_id: ENLIGHTENMENT_EXTENSION_MARKER.to_string(),
                        choice: "LifespanExtensionEnlightenment".to_string(),
                        magnitude: 0.0,
                        flavor: "悟道延寿已用，悟境天花永久下调。".to_string(),
                        alignment: None,
                        cost_kind: None,
                        taken_at: clock.tick,
                        realm_at_time,
                    });
            }
        }
        if let (Some(player_persistence), Some(char_id)) = (player_persistence, char_id.as_deref())
        {
            if let Some(username) = player_username_from_character_id(char_id) {
                if let Err(error) = crate::player::state::save_player_lifespan_slice(
                    player_persistence,
                    username,
                    &lifespan,
                ) {
                    tracing::warn!(
                        "[bong][lifespan] failed to persist extended lifespan for {char_id}: {error}"
                    );
                }
            }
        }
    }
}

pub fn sync_frailty_status_effects(
    mut actors: Query<(&LifespanComponent, Option<&Cultivation>, &mut StatusEffects)>,
) {
    for (lifespan, cultivation, mut status_effects) in &mut actors {
        if !lifespan.is_wind_candle() {
            status_effects
                .active
                .retain(|effect| effect.kind != StatusEffectKind::Frailty);
            continue;
        }

        let multiplier = cultivation
            .map(|cultivation| frailty_qi_recovery_multiplier_for_realm(cultivation.realm))
            .unwrap_or_else(|| frailty_qi_recovery_multiplier_for_realm(Realm::Awaken));
        let magnitude = (1.0 - multiplier).clamp(0.0, 0.95) as f32;
        if let Some(effect) = status_effects
            .active
            .iter_mut()
            .find(|effect| effect.kind == StatusEffectKind::Frailty)
        {
            effect.magnitude = magnitude;
            effect.remaining_ticks = u64::MAX;
            continue;
        }

        status_effects.active.push(ActiveStatusEffect {
            kind: StatusEffectKind::Frailty,
            magnitude,
            remaining_ticks: u64::MAX,
        });
    }
}

fn extension_contract_from_source(
    source: &str,
    requested_years: u32,
) -> Box<dyn ExtensionContract> {
    match source {
        "enlightenment_extension" => Box::new(EnlightenmentExtensionContract),
        "collapse_core" => Box::new(CollapseCoreExtensionContract {
            years: requested_years,
        }),
        _ => Box::new(PillExtensionContract {
            years: requested_years,
        }),
    }
}

fn lifespan_extension_cost_pressure(accumulated_years: f64, cap_by_realm: u32) -> f64 {
    (1.0 + accumulated_years.max(0.0) / cap_by_realm.max(1) as f64).powf(1.5)
}

pub fn apply_lifespan_extension(
    lifespan: &mut LifespanComponent,
    ledger: &mut LifespanExtensionLedger,
    requested_years: u32,
    consumes_enlightenment: bool,
) -> Option<u32> {
    if consumes_enlightenment && ledger.enlightenment_used {
        return None;
    }

    let requested_years = if requested_years == 0 && consumes_enlightenment {
        let target_remaining =
            lifespan.cap_by_realm as f64 * LIFESPAN_ENLIGHTENMENT_TARGET_REMAINING_RATIO;
        (target_remaining - lifespan.remaining_years())
            .ceil()
            .max(0.0) as u32
    } else {
        requested_years
    };
    if requested_years == 0 {
        return None;
    }

    let hard_cap = lifespan.cap_by_realm as f64 * LIFESPAN_EXTENSION_HARD_CAP_FACTOR;
    let remaining_extension_budget = (hard_cap - ledger.accumulated_years).max(0.0);
    let fillable_years = lifespan.years_lived.max(0.0);
    let applied = (requested_years as f64)
        .min(remaining_extension_budget)
        .min(fillable_years)
        .floor() as u32;
    if applied == 0 {
        return None;
    }

    lifespan.years_lived = (lifespan.years_lived - applied as f64).max(0.0);
    ledger.accumulated_years += applied as f64;
    if consumes_enlightenment {
        ledger.enlightenment_used = true;
    }
    Some(applied)
}

fn apply_extension_cost(
    cost: ExtensionCost,
    cultivation: Option<valence::prelude::Mut<'_, Cultivation>>,
    player_state: Option<valence::prelude::Mut<'_, PlayerState>>,
) {
    if let Some(mut cultivation) = cultivation {
        if cost.qi_cap_delta < 0.0 {
            let factor = (1.0 + cost.qi_cap_delta).clamp(0.05, 1.0);
            cultivation.qi_max = (cultivation.qi_max * factor).max(1.0);
            cultivation.qi_current = cultivation.qi_current.min(cultivation.qi_max);
        }
    }
    if let Some(mut player_state) = player_state {
        if cost.karma_delta != 0.0 {
            player_state.karma = (player_state.karma + cost.karma_delta).clamp(-1.0, 1.0);
        }
    }
}

pub fn lifespan_cap_for_actor(
    cultivation: Option<&Cultivation>,
    player_state: Option<&PlayerState>,
) -> u32 {
    let _ = player_state;
    cultivation.map_or(LifespanCapTable::MORTAL, |cultivation| {
        LifespanCapTable::for_realm(cultivation.realm)
    })
}

pub fn lifespan_event_payload_from_record(
    character_id: impl Into<String>,
    event: &LifespanEventRecord,
) -> LifespanEventV1 {
    LifespanEventV1 {
        v: 1,
        character_id: character_id.into(),
        at_tick: event.at_tick,
        kind: lifespan_event_kind_from_record(event.kind.as_str()),
        delta_years: event.delta_years,
        source: event.source.clone(),
    }
}

fn emit_lifespan_event(
    events: Option<&mut Events<LifespanEventEmitted>>,
    char_id: &str,
    event: &LifespanEventRecord,
) {
    if let Some(events) = events {
        events.send(LifespanEventEmitted {
            payload: lifespan_event_payload_from_record(char_id.to_string(), event),
        });
    }
}

fn emit_aging_event(
    events: Option<&mut Events<AgingEventEmitted>>,
    char_id: &str,
    kind: AgingEventKindV1,
    at_tick: u64,
    lifespan: &LifespanComponent,
    tick_rate_multiplier: f64,
) {
    if let Some(events) = events {
        events.send(AgingEventEmitted {
            payload: AgingEventV1 {
                v: 1,
                character_id: char_id.to_string(),
                at_tick,
                kind,
                years_lived: lifespan.years_lived,
                cap_by_realm: lifespan.cap_by_realm,
                remaining_years: lifespan.remaining_years(),
                tick_rate_multiplier,
                source: lifespan_event_source(tick_rate_multiplier).to_string(),
            },
        });
    }
}

fn lifespan_event_kind_from_record(kind: &str) -> LifespanEventKindV1 {
    match kind {
        "death_penalty" => LifespanEventKindV1::DeathPenalty,
        "extension" => LifespanEventKindV1::Extension,
        _ => LifespanEventKindV1::Aging,
    }
}

pub fn lifespan_tick_rate_multiplier(
    position: Option<&Position>,
    zones: Option<&ZoneRegistry>,
) -> f64 {
    let Some(position) = position else {
        return LIFESPAN_ONLINE_MULTIPLIER;
    };
    let Some(zone) = zones.and_then(|zones| {
        zones.find_zone(
            crate::world::dimension::DimensionKind::Overworld,
            position.get(),
        )
    }) else {
        return LIFESPAN_ONLINE_MULTIPLIER;
    };
    if zone.spirit_qi < NEGATIVE_ZONE_SPIRIT_QI_THRESHOLD {
        LIFESPAN_NEGATIVE_ZONE_MULTIPLIER
    } else {
        LIFESPAN_ONLINE_MULTIPLIER
    }
}

pub fn season_aging_modifier(season: Season) -> f64 {
    if season.is_xizhuan() {
        1.2
    } else {
        1.0
    }
}

fn lifespan_season(position: Option<&Position>, zones: Option<&ZoneRegistry>, tick: u64) -> Season {
    let zone_name = position
        .and_then(|position| {
            zones.and_then(|zones| {
                zones
                    .find_zone(
                        crate::world::dimension::DimensionKind::Overworld,
                        position.get(),
                    )
                    .map(|zone| zone.name.as_str())
            })
        })
        .unwrap_or("");
    query_season(zone_name, tick).season
}

pub fn is_collapse_abyss_zone(zone: &Zone) -> bool {
    zone.spirit_qi < NEGATIVE_ZONE_SPIRIT_QI_THRESHOLD
        && (zone.name.contains("collapse")
            || zone.name.contains("fu_ling")
            || zone.active_events.iter().any(|event| {
                event.contains("collapse")
                    || event.contains("fu_ling")
                    || event.contains("life_core")
            }))
}

pub fn lifespan_delta_years_for_ticks(ticks: u64, multiplier: f64) -> f64 {
    ticks as f64 * multiplier.max(0.0) / LIFESPAN_TICKS_PER_YEAR as f64
}

pub fn lifespan_delta_years_for_real_seconds(seconds: u64, multiplier: f64) -> f64 {
    seconds as f64 * multiplier.max(0.0) / LIFESPAN_SECONDS_PER_YEAR as f64
}

pub fn lifespan_whole_years_crossed(before: f64, after: f64) -> i64 {
    let before = before.max(0.0).floor() as i64;
    let after = after.max(0.0).floor() as i64;
    (after - before).max(0)
}

fn lifespan_event_char_id<'a>(
    life_record: Option<&'a LifeRecord>,
    lifecycle: Option<&'a Lifecycle>,
) -> Option<&'a str> {
    life_record
        .map(|record| record.character_id.as_str())
        .or_else(|| lifecycle.map(|lifecycle| lifecycle.character_id.as_str()))
}

fn lifespan_event_source(multiplier: f64) -> &'static str {
    if (multiplier - LIFESPAN_NEGATIVE_ZONE_MULTIPLIER).abs() <= f64::EPSILON {
        "zone_negative"
    } else {
        "online"
    }
}

fn lifespan_narration_target<'a>(
    life_record: Option<&'a LifeRecord>,
    lifecycle: Option<&'a Lifecycle>,
) -> Option<&'a str> {
    let char_id = lifespan_event_char_id(life_record, lifecycle)?;
    char_id.starts_with("offline:").then_some(char_id)
}

fn should_emit_wind_candle_narration(lifespan: &LifespanComponent, at_tick: u64) -> bool {
    at_tick > 0
        && at_tick.is_multiple_of(LIFESPAN_WIND_CANDLE_NARRATION_INTERVAL_TICKS)
        && lifespan.remaining_years() > f64::EPSILON
        && lifespan.is_wind_candle()
}

fn wind_candle_narration_text(lifespan: &LifespanComponent) -> String {
    format!(
        "寿火将尽，余寿约 {:.1} 年。真元如残烛摇曳。",
        lifespan.remaining_years()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::{params, Connection};
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};
    use valence::prelude::{App, Events, Update};

    fn persistence_settings(test_name: &str) -> (PersistenceSettings, PathBuf) {
        let unique_suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "bong-lifespan-{test_name}-{}-{unique_suffix}",
            std::process::id(),
        ));
        let db_path = root.join("data").join("bong.db");
        let deceased_dir = root.join("library-web").join("public").join("deceased");
        let settings = PersistenceSettings::with_paths(
            &db_path,
            &deceased_dir,
            format!("lifespan-{test_name}"),
        );
        crate::persistence::bootstrap_sqlite(settings.db_path(), settings.server_run_id())
            .expect("bootstrap should succeed");
        (settings, root)
    }

    #[test]
    fn cap_table_matches_plan_values() {
        assert_eq!(LifespanCapTable::MORTAL, 80);
        assert_eq!(LifespanCapTable::for_realm(Realm::Awaken), 120);
        assert_eq!(LifespanCapTable::for_realm(Realm::Induce), 200);
        assert_eq!(LifespanCapTable::for_realm(Realm::Condense), 350);
        assert_eq!(LifespanCapTable::for_realm(Realm::Solidify), 600);
        assert_eq!(LifespanCapTable::for_realm(Realm::Spirit), 1000);
        assert_eq!(LifespanCapTable::for_realm(Realm::Void), 2000);
    }

    #[test]
    fn death_penalty_uses_five_percent_floor() {
        assert_eq!(LifespanCapTable::death_penalty_years_for_cap(80), 4);
        assert_eq!(
            LifespanCapTable::death_penalty_years_for_realm(Realm::Awaken),
            6
        );
        assert_eq!(
            LifespanCapTable::death_penalty_years_for_realm(Realm::Condense),
            17
        );
        assert_eq!(
            LifespanCapTable::death_penalty_years_for_realm(Realm::Void),
            100
        );
    }

    #[test]
    fn mortal_player_state_keeps_mortal_cap() {
        assert_eq!(
            LifespanCapTable::for_player_state_realm(Some("mortal"), Realm::Awaken),
            LifespanCapTable::MORTAL
        );
    }

    #[test]
    fn death_registry_tracks_latest_death_context() {
        let mut registry = DeathRegistry::new("offline:Azure");
        registry.record_death(1440, ZoneDeathKind::Negative);

        assert_eq!(registry.death_count, 1);
        assert_eq!(registry.last_death_tick, Some(1440));
        assert_eq!(registry.prev_death_tick, None);
        assert_eq!(registry.last_death_zone, Some(ZoneDeathKind::Negative));
    }

    #[test]
    fn normal_zone_first_three_deaths_are_guaranteed() {
        let input = RebirthChanceInput {
            registry: DeathRegistry::new("offline:Azure"),
            at_tick: 1440,
            death_zone: ZoneDeathKind::Ordinary,
            karma: 0.9,
            has_shrine: false,
            includes_current_death: false,
        };

        let result = calculate_rebirth_chance(&input);
        assert_eq!(result.stage, RebirthStage::Fortune);
        assert_eq!(result.chance, 1.0);
        assert!(result.guaranteed);
        assert_eq!(result.fortune_charge_cost, 1);
    }

    #[test]
    fn death_zone_skips_fortune_and_rolls_base_chance() {
        let input = RebirthChanceInput {
            registry: DeathRegistry::new("offline:Azure"),
            at_tick: 1440,
            death_zone: ZoneDeathKind::Death,
            karma: 0.0,
            has_shrine: true,
            includes_current_death: false,
        };

        let result = calculate_rebirth_chance(&input);
        assert_eq!(result.stage, RebirthStage::Tribulation);
        assert_eq!(result.chance, 0.80);
        assert!(result.skip_fortune_due_to_zone);
        assert_eq!(result.fortune_charge_cost, 0);
    }

    #[test]
    fn shrine_allows_fortune_even_when_recent_death_and_high_karma() {
        let mut input = RebirthChanceInput {
            registry: DeathRegistry {
                char_id: "offline:Azure".to_string(),
                death_count: 1,
                last_death_tick: Some(1440),
                prev_death_tick: Some(0),
                last_death_zone: Some(ZoneDeathKind::Ordinary),
            },
            at_tick: 1500,
            death_zone: ZoneDeathKind::Ordinary,
            karma: 0.9,
            has_shrine: false,
            includes_current_death: true,
        };

        let without_shrine = calculate_rebirth_chance(&input);
        assert_eq!(without_shrine.stage, RebirthStage::Tribulation);
        assert!(!without_shrine.guaranteed);
        assert_eq!(without_shrine.chance, 0.80);
        assert!(!without_shrine.has_shrine);
        assert!(!without_shrine.no_recent_death);
        assert!(!without_shrine.low_karma);

        input.has_shrine = true;
        let with_shrine = calculate_rebirth_chance(&input);
        assert_eq!(with_shrine.stage, RebirthStage::Fortune);
        assert!(with_shrine.guaranteed);
        assert_eq!(with_shrine.chance, 1.0);
        assert_eq!(with_shrine.fortune_charge_cost, 1);
        assert!(with_shrine.has_shrine);
    }

    #[test]
    fn already_recorded_third_death_still_counts_as_third_fortune_death() {
        let input = RebirthChanceInput {
            registry: DeathRegistry {
                char_id: "offline:Azure".to_string(),
                death_count: 3,
                last_death_tick: Some(1440),
                prev_death_tick: Some(0),
                last_death_zone: Some(ZoneDeathKind::Ordinary),
            },
            at_tick: 1440,
            death_zone: ZoneDeathKind::Ordinary,
            karma: 0.0,
            has_shrine: false,
            includes_current_death: true,
        };

        let result = calculate_rebirth_chance(&input);
        assert_eq!(result.death_number, 3);
        assert_eq!(result.stage, RebirthStage::Fortune);
        assert!(result.guaranteed);
    }

    #[test]
    fn tribulation_chance_has_five_percent_floor() {
        assert!((tribulation_rebirth_chance(4) - 0.65).abs() < 1e-9);
        assert!((tribulation_rebirth_chance(5) - 0.50).abs() < 1e-9);
        assert!((tribulation_rebirth_chance(6) - 0.35).abs() < 1e-9);
        assert!((tribulation_rebirth_chance(7) - 0.20).abs() < 1e-9);
        assert!((tribulation_rebirth_chance(8) - 0.05).abs() < 1e-9);
        assert!((tribulation_rebirth_chance(12) - 0.05).abs() < 1e-9);
    }

    #[test]
    fn wind_candle_uses_remaining_ten_percent_threshold() {
        let mut lifespan = LifespanComponent::new(LifespanCapTable::SPIRIT);
        lifespan.years_lived = 905.0;
        assert!(lifespan.is_wind_candle());
        assert_eq!(lifespan.remaining_years(), 95.0);
    }

    #[test]
    fn frailty_status_syncs_with_wind_candle_state() {
        let mut app = App::new();
        app.add_systems(Update, sync_frailty_status_effects);

        let mut lifespan = LifespanComponent::new(LifespanCapTable::SOLIDIFY);
        lifespan.years_lived = 545.0;
        let entity = app
            .world_mut()
            .spawn((
                lifespan,
                Cultivation {
                    realm: Realm::Solidify,
                    ..Default::default()
                },
                StatusEffects::default(),
            ))
            .id();

        app.update();

        let statuses = app.world().entity(entity).get::<StatusEffects>().unwrap();
        let frailty = statuses
            .active
            .iter()
            .find(|effect| effect.kind == StatusEffectKind::Frailty)
            .expect("wind-candle actor should receive Frailty status");
        assert!((frailty.magnitude - 0.5).abs() < 1e-6);

        app.world_mut()
            .entity_mut(entity)
            .get_mut::<LifespanComponent>()
            .unwrap()
            .years_lived = 100.0;
        app.update();

        let statuses = app.world().entity(entity).get::<StatusEffects>().unwrap();
        assert!(
            !statuses
                .active
                .iter()
                .any(|effect| effect.kind == StatusEffectKind::Frailty),
            "Frailty should clear after extension moves remaining lifespan above threshold"
        );
    }

    #[test]
    fn lifespan_delta_matches_one_year_per_real_hour() {
        assert_eq!(
            lifespan_delta_years_for_ticks(LIFESPAN_TICKS_PER_YEAR, LIFESPAN_ONLINE_MULTIPLIER),
            1.0
        );
        assert_eq!(
            lifespan_delta_years_for_ticks(LIFESPAN_TICKS_PER_YEAR, LIFESPAN_OFFLINE_MULTIPLIER),
            0.1
        );
        assert_eq!(
            lifespan_delta_years_for_real_seconds(
                LIFESPAN_SECONDS_PER_YEAR,
                LIFESPAN_OFFLINE_MULTIPLIER,
            ),
            0.1
        );
    }

    #[test]
    fn whole_year_crossing_counts_only_new_integer_years() {
        assert_eq!(lifespan_whole_years_crossed(0.1, 0.9), 0);
        assert_eq!(lifespan_whole_years_crossed(0.99, 1.01), 1);
        assert_eq!(lifespan_whole_years_crossed(1.2, 3.01), 2);
        assert_eq!(lifespan_whole_years_crossed(3.0, 2.0), 0);
    }

    #[test]
    fn negative_zone_doubles_lifespan_tick_rate() {
        let mut zones = ZoneRegistry::fallback();
        zones.zones[0].spirit_qi = -0.35;
        let position = Position::new([8.0, 66.0, 8.0]);

        assert_eq!(
            lifespan_tick_rate_multiplier(Some(&position), Some(&zones)),
            LIFESPAN_NEGATIVE_ZONE_MULTIPLIER
        );
    }

    #[test]
    fn xizhuan_phase_accelerates_aging_by_20_percent() {
        assert_eq!(season_aging_modifier(Season::Summer), 1.0);
        assert_eq!(season_aging_modifier(Season::Winter), 1.0);
        assert_eq!(season_aging_modifier(Season::SummerToWinter), 1.2);
        assert_eq!(season_aging_modifier(Season::WinterToSummer), 1.2);
    }

    #[test]
    fn lifespan_extension_clamps_to_fillable_age_and_hard_cap() {
        let mut lifespan = LifespanComponent::new(LifespanCapTable::MORTAL);
        lifespan.years_lived = 30.0;
        let mut ledger = LifespanExtensionLedger {
            accumulated_years: 159.0,
            enlightenment_used: false,
        };

        let applied = apply_lifespan_extension(&mut lifespan, &mut ledger, 10, false);

        assert_eq!(applied, Some(1));
        assert_eq!(lifespan.years_lived, 29.0);
        assert_eq!(ledger.accumulated_years, 160.0);
    }

    #[test]
    fn enlightenment_extension_targets_thirty_percent_remaining_once() {
        let mut lifespan = LifespanComponent::new(LifespanCapTable::MORTAL);
        lifespan.years_lived = 75.0;
        let mut ledger = LifespanExtensionLedger::default();

        let applied = apply_lifespan_extension(&mut lifespan, &mut ledger, 0, true);

        assert_eq!(applied, Some(19));
        assert_eq!(lifespan.remaining_years(), 24.0);
        assert!(ledger.enlightenment_used);
        assert_eq!(
            apply_lifespan_extension(&mut lifespan, &mut ledger, 0, true),
            None
        );
    }

    #[test]
    fn process_lifespan_extension_intent_emits_and_persists_event() {
        let (settings, root) = persistence_settings("extension-event");
        let mut app = App::new();
        app.insert_resource(settings.clone());
        app.insert_resource(CultivationClock { tick: 99 });
        app.add_event::<LifespanExtensionIntent>();
        app.add_event::<LifespanEventEmitted>();
        app.add_systems(Update, process_lifespan_extension_intents);

        let mut lifespan = LifespanComponent::new(LifespanCapTable::MORTAL);
        lifespan.years_lived = 70.0;
        let entity = app
            .world_mut()
            .spawn((
                lifespan,
                LifespanExtensionLedger::default(),
                LifeRecord::new("offline:Azure"),
            ))
            .id();
        app.world_mut()
            .resource_mut::<Events<LifespanExtensionIntent>>()
            .send(LifespanExtensionIntent {
                entity,
                requested_years: 10,
                source: "life_extension_pill".to_string(),
            });

        app.update();

        let lifespan = app
            .world()
            .entity(entity)
            .get::<LifespanComponent>()
            .unwrap();
        assert_eq!(lifespan.years_lived, 60.0);
        let emitted = app.world().resource::<Events<LifespanEventEmitted>>();
        assert_eq!(emitted.len(), 1);

        let connection = Connection::open(settings.db_path()).expect("db should open");
        let (event_type, payload_json): (String, String) = connection
            .query_row(
                "SELECT event_type, payload_json FROM lifespan_events WHERE char_id = ?1",
                params!["offline:Azure"],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("extension lifespan event should persist");
        let payload: LifespanEventRecord =
            serde_json::from_str(&payload_json).expect("lifespan payload should decode");
        assert_eq!(event_type, "extension");
        assert_eq!(payload.kind, "extension");
        assert_eq!(payload.delta_years, 10);
        assert_eq!(payload.source, "life_extension_pill");

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn lifespan_extension_pill_reduces_qi_max_by_cost_curve() {
        let mut app = App::new();
        app.insert_resource(CultivationClock { tick: 1 });
        app.add_event::<LifespanExtensionIntent>();
        app.add_systems(Update, process_lifespan_extension_intents);

        let mut lifespan = LifespanComponent::new(LifespanCapTable::MORTAL);
        lifespan.years_lived = 70.0;
        let entity = app
            .world_mut()
            .spawn((
                lifespan,
                LifespanExtensionLedger::default(),
                Cultivation {
                    qi_current: 80.0,
                    qi_max: 100.0,
                    ..Default::default()
                },
                PlayerState::default(),
                LifeRecord::new("offline:Azure"),
            ))
            .id();

        app.world_mut()
            .resource_mut::<Events<LifespanExtensionIntent>>()
            .send(LifespanExtensionIntent {
                entity,
                requested_years: 10,
                source: "life_extension_pill".to_string(),
            });
        app.update();

        let cultivation = app.world().entity(entity).get::<Cultivation>().unwrap();
        assert!((cultivation.qi_max - 90.0).abs() < 1e-9);
        assert_eq!(cultivation.qi_current, 80.0);
    }

    #[test]
    fn lifespan_extension_pill_cost_increases_with_accumulated_extension() {
        let mut app = App::new();
        app.insert_resource(CultivationClock { tick: 1 });
        app.add_event::<LifespanExtensionIntent>();
        app.add_systems(Update, process_lifespan_extension_intents);

        let mut lifespan = LifespanComponent::new(LifespanCapTable::INDUCE);
        lifespan.years_lived = 120.0;
        let entity = app
            .world_mut()
            .spawn((
                lifespan,
                LifespanExtensionLedger {
                    accumulated_years: 100.0,
                    enlightenment_used: false,
                },
                Cultivation {
                    realm: Realm::Induce,
                    qi_current: 100.0,
                    qi_max: 100.0,
                    ..Default::default()
                },
                PlayerState::default(),
                LifeRecord::new("offline:Azure"),
            ))
            .id();

        app.world_mut()
            .resource_mut::<Events<LifespanExtensionIntent>>()
            .send(LifespanExtensionIntent {
                entity,
                requested_years: 10,
                source: "life_extension_pill".to_string(),
            });
        app.update();

        let cultivation = app.world().entity(entity).get::<Cultivation>().unwrap();
        let pressure = lifespan_extension_cost_pressure(100.0, LifespanCapTable::INDUCE);
        let expected_qi_max =
            100.0 * (1.0 - 10.0 * LIFESPAN_EXTENSION_PILL_QI_MAX_COST_PER_YEAR * pressure);
        assert!((cultivation.qi_max - expected_qi_max).abs() < 1e-9);
        assert!(
            cultivation.qi_max < 90.0,
            "prior extension ledger should make the second pill harsher than the first"
        );
        assert_eq!(cultivation.qi_current, cultivation.qi_max);
    }

    #[test]
    fn lifespan_aging_tick_triggers_natural_death_when_exhausted() {
        let mut app = App::new();
        app.insert_resource(CultivationClock { tick: 42 });
        app.add_event::<CultivationDeathTrigger>();
        app.add_systems(Update, lifespan_aging_tick);

        let mut lifespan = LifespanComponent::new(LifespanCapTable::MORTAL);
        lifespan.years_lived = LifespanCapTable::MORTAL as f64
            - lifespan_delta_years_for_ticks(1, LIFESPAN_ONLINE_MULTIPLIER) / 2.0;
        let entity = app
            .world_mut()
            .spawn((lifespan, Position::new([8.0, 66.0, 8.0])))
            .id();

        app.update();

        let lifespan = app
            .world()
            .entity(entity)
            .get::<LifespanComponent>()
            .unwrap();
        let deaths = app.world().resource::<Events<CultivationDeathTrigger>>();
        assert_eq!(lifespan.remaining_years(), 0.0);
        assert_eq!(deaths.len(), 1);
    }

    #[test]
    fn lifespan_aging_tick_persists_year_boundary_event() {
        let (settings, root) = persistence_settings("aging-event");
        let mut app = App::new();
        app.insert_resource(settings.clone());
        app.insert_resource(CultivationClock { tick: 42 });
        app.add_event::<CultivationDeathTrigger>();
        app.add_systems(Update, lifespan_aging_tick);

        let mut lifespan = LifespanComponent::new(LifespanCapTable::AWAKEN);
        lifespan.years_lived =
            1.0 - lifespan_delta_years_for_ticks(1, LIFESPAN_ONLINE_MULTIPLIER) / 2.0;
        app.world_mut().spawn((
            lifespan,
            LifeRecord::new("offline:Azure"),
            Position::new([8.0, 66.0, 8.0]),
        ));

        app.update();

        let connection = Connection::open(settings.db_path()).expect("db should open");
        let (event_type, payload_json): (String, String) = connection
            .query_row(
                "SELECT event_type, payload_json FROM lifespan_events WHERE char_id = ?1",
                params!["offline:Azure"],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("aging lifespan event should persist");
        let payload: LifespanEventRecord =
            serde_json::from_str(&payload_json).expect("lifespan payload should decode");

        assert_eq!(event_type, "aging");
        assert_eq!(payload.kind, "aging");
        assert_eq!(payload.delta_years, 1);
        assert_eq!(payload.source, "online");
        assert_eq!(payload.at_tick, 42);

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn wind_candle_narration_emits_once_per_day_boundary() {
        let mut app = App::new();
        app.insert_resource(CultivationClock {
            tick: LIFESPAN_WIND_CANDLE_NARRATION_INTERVAL_TICKS,
        });
        app.insert_resource(PendingGameplayNarrations::default());
        app.add_event::<CultivationDeathTrigger>();
        app.add_systems(Update, lifespan_aging_tick);

        let mut lifespan = LifespanComponent::new(LifespanCapTable::MORTAL);
        lifespan.years_lived = 73.0;
        app.world_mut().spawn((
            lifespan,
            LifeRecord::new("offline:Azure"),
            Position::new([8.0, 66.0, 8.0]),
        ));

        app.update();

        let narrations = app
            .world_mut()
            .resource_mut::<PendingGameplayNarrations>()
            .drain();

        assert_eq!(narrations.len(), 1);
        assert_eq!(narrations[0].target.as_deref(), Some("offline:Azure"));
        assert_eq!(narrations[0].style, NarrationStyle::Perception);
        assert!(narrations[0].text.contains("寿火将尽"));
    }

    #[test]
    fn wind_candle_narration_skips_non_daily_ticks() {
        let mut app = App::new();
        app.insert_resource(CultivationClock {
            tick: LIFESPAN_WIND_CANDLE_NARRATION_INTERVAL_TICKS - 1,
        });
        app.insert_resource(PendingGameplayNarrations::default());
        app.add_event::<CultivationDeathTrigger>();
        app.add_systems(Update, lifespan_aging_tick);

        let mut lifespan = LifespanComponent::new(LifespanCapTable::MORTAL);
        lifespan.years_lived = 73.0;
        app.world_mut().spawn((
            lifespan,
            LifeRecord::new("offline:Azure"),
            Position::new([8.0, 66.0, 8.0]),
        ));

        app.update();

        let narrations = app
            .world_mut()
            .resource_mut::<PendingGameplayNarrations>()
            .drain();

        assert!(narrations.is_empty());
    }
}
