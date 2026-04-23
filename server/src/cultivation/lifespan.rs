use serde::{Deserialize, Serialize};
use valence::prelude::{bevy_ecs, Component};

use super::components::Realm;

pub const KARMA_REBIRTH_THRESHOLD: f64 = 0.5;
pub const REBIRTH_SAFE_WINDOW_TICKS: u64 = 24 * 60 * 60 * 20;
pub const REBIRTH_BASE_CHANCE: f64 = 0.80;
pub const REBIRTH_STEP_PER_DEATH: f64 = 0.15;
pub const REBIRTH_MIN_CHANCE: f64 = 0.05;
pub const WIND_CANDLE_THRESHOLD: f64 = 0.10;

const UNASSIGNED_DEATH_REGISTRY_ID: &str = "unassigned:death_registry";

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
            last_death_zone: None,
        }
    }

    pub fn next_death_number(&self) -> u32 {
        self.death_count.saturating_add(1)
    }

    pub fn record_death(&mut self, at_tick: u64, zone: ZoneDeathKind) {
        self.death_count = self.death_count.saturating_add(1);
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
    let death_number = input.registry.next_death_number();
    let skip_fortune_due_to_zone = input.death_zone.skips_fortune();
    let no_recent_death = input
        .registry
        .last_death_tick
        .is_none_or(|tick| input.at_tick.saturating_sub(tick) >= REBIRTH_SAFE_WINDOW_TICKS);
    let low_karma = input.karma < KARMA_REBIRTH_THRESHOLD;
    let guaranteed = death_number <= 3 && !skip_fortune_due_to_zone;

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

#[cfg(test)]
mod tests {
    use super::*;

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
        };

        let result = calculate_rebirth_chance(&input);
        assert_eq!(result.stage, RebirthStage::Tribulation);
        assert_eq!(result.chance, 0.80);
        assert!(result.skip_fortune_due_to_zone);
        assert_eq!(result.fortune_charge_cost, 0);
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
}
