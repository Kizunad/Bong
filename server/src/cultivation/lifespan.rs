use serde::{Deserialize, Serialize};
use valence::prelude::{bevy_ecs, Component, Entity, EventWriter, Position, Query, Res, ResMut};

use super::components::Realm;
use super::death_hooks::{CultivationDeathCause, CultivationDeathTrigger};
use super::tick::CultivationClock;
use crate::combat::components::{Lifecycle, LifecycleState};
use crate::cultivation::components::Cultivation;
use crate::cultivation::life_record::LifeRecord;
use crate::persistence::{persist_lifespan_event, LifespanEventRecord, PersistenceSettings};
use crate::player::gameplay::PendingGameplayNarrations;
use crate::player::state::PlayerState;
use crate::schema::common::NarrationStyle;
use crate::world::zone::ZoneRegistry;

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

const UNASSIGNED_DEATH_REGISTRY_ID: &str = "unassigned:death_registry";
const NEGATIVE_ZONE_SPIRIT_QI_THRESHOLD: f64 = -0.2;

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

#[allow(clippy::type_complexity)]
pub fn lifespan_aging_tick(
    clock: Res<CultivationClock>,
    persistence: Option<Res<PersistenceSettings>>,
    zones: Option<Res<ZoneRegistry>>,
    mut pending_narrations: Option<ResMut<PendingGameplayNarrations>>,
    mut deaths: EventWriter<CultivationDeathTrigger>,
    mut actors: Query<(
        Entity,
        &mut LifespanComponent,
        Option<&Cultivation>,
        Option<&PlayerState>,
        Option<&Position>,
        Option<&Lifecycle>,
        Option<&LifeRecord>,
    )>,
) {
    let persistence = persistence.as_deref();
    let zones = zones.as_deref();

    for (entity, mut lifespan, cultivation, player_state, position, lifecycle, life_record) in
        &mut actors
    {
        if lifecycle.is_some_and(|lifecycle| lifecycle.state == LifecycleState::Terminated) {
            continue;
        }

        let previous_years_lived = lifespan.years_lived;
        let previous_remaining = lifespan.remaining_years();
        let cap = lifespan_cap_for_actor(cultivation, player_state);
        lifespan.apply_cap(cap);

        let multiplier = lifespan_tick_rate_multiplier(position, zones);
        let delta_years = lifespan_delta_years_for_ticks(1, multiplier);
        lifespan.years_lived =
            (lifespan.years_lived + delta_years).min(lifespan.cap_by_realm as f64);

        let crossed_years =
            lifespan_whole_years_crossed(previous_years_lived, lifespan.years_lived);
        if crossed_years > 0 {
            if let (Some(settings), Some(char_id)) =
                (persistence, lifespan_event_char_id(life_record, lifecycle))
            {
                let event = LifespanEventRecord {
                    at_tick: clock.tick,
                    kind: "aging".to_string(),
                    delta_years: crossed_years,
                    source: lifespan_event_source(multiplier).to_string(),
                };
                if let Err(error) = persist_lifespan_event(settings, char_id, &event) {
                    tracing::warn!(
                        "[bong][lifespan] failed to persist aging event for {char_id}: {error}"
                    );
                }
            }
        }

        if should_emit_wind_candle_narration(&lifespan, clock.tick) {
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

pub fn lifespan_cap_for_actor(
    cultivation: Option<&Cultivation>,
    player_state: Option<&PlayerState>,
) -> u32 {
    match (player_state, cultivation) {
        (Some(player_state), Some(cultivation)) => LifespanCapTable::for_player_state_realm(
            Some(player_state.realm.as_str()),
            cultivation.realm,
        ),
        (Some(player_state), None) => LifespanCapTable::for_player_state_realm(
            Some(player_state.realm.as_str()),
            Realm::Awaken,
        ),
        (None, Some(cultivation)) => LifespanCapTable::for_realm(cultivation.realm),
        (None, None) => LifespanCapTable::MORTAL,
    }
}

pub fn lifespan_tick_rate_multiplier(
    position: Option<&Position>,
    zones: Option<&ZoneRegistry>,
) -> f64 {
    let Some(position) = position else {
        return LIFESPAN_ONLINE_MULTIPLIER;
    };
    let Some(zone) = zones.and_then(|zones| zones.find_zone(position.get())) else {
        return LIFESPAN_ONLINE_MULTIPLIER;
    };
    if zone.spirit_qi < NEGATIVE_ZONE_SPIRIT_QI_THRESHOLD {
        LIFESPAN_NEGATIVE_ZONE_MULTIPLIER
    } else {
        LIFESPAN_ONLINE_MULTIPLIER
    }
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
