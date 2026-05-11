use std::collections::{HashMap, VecDeque};

use serde_json::Value;
use valence::prelude::{bevy_ecs, App, Resource};

use crate::world::season::Season;

pub const EVENT_THUNDER_TRIBULATION: &str = "thunder_tribulation";
pub const EVENT_BEAST_TIDE: &str = "beast_tide";
pub const EVENT_REALM_COLLAPSE: &str = "realm_collapse";
pub const EVENT_KARMA_BACKLASH: &str = "karma_backlash";
pub const EVENT_POISON_MIASMA: &str = "poison_miasma";
pub const EVENT_MERIDIAN_SEAL: &str = "meridian_seal";
pub const EVENT_DAOXIANG_WAVE: &str = "daoxiang_wave";
pub const EVENT_HEAVENLY_FIRE: &str = "heavenly_fire";
pub const EVENT_PRESSURE_INVERT: &str = "pressure_invert";
pub const EVENT_ALL_WITHER: &str = "all_wither";

pub const CALAMITY_TARGET_WINDOW_TICKS: u64 = 10 * 60 * 20;
pub const CALAMITY_TARGET_WINDOW_LIMIT: usize = 3;
pub const CALAMITY_ZONE_CONCURRENCY_LIMIT: usize = 2;
pub const TIANDAO_POWER_MAX_SPEND_LOG: usize = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CalamityKind {
    Thunder,
    PoisonMiasma,
    MeridianSeal,
    DaoxiangWave,
    HeavenlyFire,
    PressureInvert,
    AllWither,
    RealmCollapse,
}

impl CalamityKind {
    pub const ALL: [Self; 8] = [
        Self::Thunder,
        Self::PoisonMiasma,
        Self::MeridianSeal,
        Self::DaoxiangWave,
        Self::HeavenlyFire,
        Self::PressureInvert,
        Self::AllWither,
        Self::RealmCollapse,
    ];

    pub fn from_event_name(event_name: &str) -> Option<Self> {
        match event_name.trim() {
            EVENT_THUNDER_TRIBULATION | "thunder" | "calamity_thunder" => Some(Self::Thunder),
            EVENT_POISON_MIASMA | "calamity_miasma" => Some(Self::PoisonMiasma),
            EVENT_MERIDIAN_SEAL | "calamity_meridian_seal" => Some(Self::MeridianSeal),
            EVENT_DAOXIANG_WAVE | "calamity_daoxiang_wave" => Some(Self::DaoxiangWave),
            EVENT_HEAVENLY_FIRE | "calamity_heavenly_fire" => Some(Self::HeavenlyFire),
            EVENT_PRESSURE_INVERT | "calamity_pressure_invert" => Some(Self::PressureInvert),
            EVENT_ALL_WITHER | "calamity_all_wither" => Some(Self::AllWither),
            EVENT_REALM_COLLAPSE | "calamity_realm_collapse" => Some(Self::RealmCollapse),
            _ => None,
        }
    }

    pub const fn event_name(self) -> &'static str {
        match self {
            Self::Thunder => EVENT_THUNDER_TRIBULATION,
            Self::PoisonMiasma => EVENT_POISON_MIASMA,
            Self::MeridianSeal => EVENT_MERIDIAN_SEAL,
            Self::DaoxiangWave => EVENT_DAOXIANG_WAVE,
            Self::HeavenlyFire => EVENT_HEAVENLY_FIRE,
            Self::PressureInvert => EVENT_PRESSURE_INVERT,
            Self::AllWither => EVENT_ALL_WITHER,
            Self::RealmCollapse => EVENT_REALM_COLLAPSE,
        }
    }

    pub const fn schema_kind(self) -> &'static str {
        match self {
            Self::Thunder => "thunder",
            Self::PoisonMiasma => EVENT_POISON_MIASMA,
            Self::MeridianSeal => EVENT_MERIDIAN_SEAL,
            Self::DaoxiangWave => EVENT_DAOXIANG_WAVE,
            Self::HeavenlyFire => EVENT_HEAVENLY_FIRE,
            Self::PressureInvert => EVENT_PRESSURE_INVERT,
            Self::AllWither => EVENT_ALL_WITHER,
            Self::RealmCollapse => EVENT_REALM_COLLAPSE,
        }
    }

    pub const fn power_cost(self) -> f64 {
        match self {
            Self::Thunder => 15.0,
            Self::PoisonMiasma => 20.0,
            Self::MeridianSeal => 25.0,
            Self::DaoxiangWave => 30.0,
            Self::HeavenlyFire => 35.0,
            Self::PressureInvert => 40.0,
            Self::AllWither => 25.0,
            Self::RealmCollapse => 60.0,
        }
    }

    pub const fn minimum_attention(self) -> AttentionTier {
        match self {
            Self::Thunder => AttentionTier::Watch,
            Self::PoisonMiasma | Self::MeridianSeal | Self::DaoxiangWave | Self::AllWither => {
                AttentionTier::Pressure
            }
            Self::HeavenlyFire | Self::PressureInvert => AttentionTier::Tribulation,
            Self::RealmCollapse => AttentionTier::Annihilate,
        }
    }

    pub const fn base_duration_ticks(self) -> u64 {
        match self {
            Self::Thunder => 60 * 20,
            Self::PoisonMiasma => 180 * 20,
            Self::MeridianSeal => 120 * 20,
            Self::DaoxiangWave => 300 * 20,
            Self::HeavenlyFire => 100 * 20,
            Self::PressureInvert => 45 * 20,
            Self::AllWither => 1,
            Self::RealmCollapse => 30 * 20,
        }
    }

    pub fn duration_ticks(self, season: Season) -> u64 {
        match (self, season) {
            (Self::MeridianSeal, Season::Winter) => self.base_duration_ticks() * 3 / 2,
            _ => self.base_duration_ticks(),
        }
    }

    pub const fn radius_blocks(self) -> f64 {
        match self {
            Self::Thunder => 30.0,
            Self::PoisonMiasma => 80.0,
            Self::MeridianSeal => 50.0,
            Self::DaoxiangWave => 80.0,
            Self::HeavenlyFire => 40.0,
            Self::PressureInvert => 60.0,
            Self::AllWither => 80.0,
            Self::RealmCollapse => 80.0,
        }
    }

    pub const fn vfx_event_id(self) -> &'static str {
        match self {
            Self::Thunder => "bong:calamity_thunder",
            Self::PoisonMiasma => "bong:calamity_miasma",
            Self::MeridianSeal => "bong:calamity_meridian_seal",
            Self::DaoxiangWave => "bong:calamity_daoxiang_wave",
            Self::HeavenlyFire => "bong:calamity_heavenly_fire",
            Self::PressureInvert => "bong:calamity_pressure_invert",
            Self::AllWither => "bong:calamity_all_wither",
            Self::RealmCollapse => "bong:realm_collapse_boundary",
        }
    }

    pub const fn audio_recipe_id(self) -> &'static str {
        match self {
            Self::Thunder => "calamity_thunder",
            Self::PoisonMiasma => "calamity_miasma",
            Self::MeridianSeal => "calamity_meridian_seal",
            Self::DaoxiangWave => "calamity_daoxiang_spawn",
            Self::HeavenlyFire => "calamity_heavenly_fire",
            Self::PressureInvert => "calamity_pressure_invert",
            Self::AllWither => "calamity_all_wither",
            Self::RealmCollapse => "calamity_realm_collapse",
        }
    }

    pub const fn vfx_color(self) -> &'static str {
        match self {
            Self::Thunder => "#E0E8FF",
            Self::PoisonMiasma => "#305020",
            Self::MeridianSeal => "#A0C8D8",
            Self::DaoxiangWave => "#C0B090",
            Self::HeavenlyFire => "#D0E0FF",
            Self::PressureInvert => "#102040",
            Self::AllWither => "#806040",
            Self::RealmCollapse => "#2B2B31",
        }
    }

    pub fn allowed_in_season(self, season: Season) -> bool {
        match self {
            Self::HeavenlyFire => season == Season::Summer,
            Self::PressureInvert => season.is_xizhuan(),
            Self::AllWither => season == Season::Winter,
            _ => true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AttentionTier {
    Watch,
    Pressure,
    Tribulation,
    Annihilate,
}

impl AttentionTier {
    pub fn from_wire(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "watch" => Some(Self::Watch),
            "pressure" => Some(Self::Pressure),
            "tribulation" => Some(Self::Tribulation),
            "annihilate" => Some(Self::Annihilate),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CalamitySpec {
    pub kind: CalamityKind,
    pub cost: f64,
    pub minimum_attention: AttentionTier,
    pub base_duration_ticks: u64,
    pub radius_blocks: f64,
}

#[derive(Debug, Clone, Resource)]
pub struct CalamityArsenal {
    specs: HashMap<CalamityKind, CalamitySpec>,
}

impl Default for CalamityArsenal {
    fn default() -> Self {
        let specs = CalamityKind::ALL
            .into_iter()
            .map(|kind| {
                (
                    kind,
                    CalamitySpec {
                        kind,
                        cost: kind.power_cost(),
                        minimum_attention: kind.minimum_attention(),
                        base_duration_ticks: kind.base_duration_ticks(),
                        radius_blocks: kind.radius_blocks(),
                    },
                )
            })
            .collect();
        Self { specs }
    }
}

impl CalamityArsenal {
    pub fn spec(&self, kind: CalamityKind) -> Option<&CalamitySpec> {
        self.specs.get(&kind)
    }

    pub fn allows(
        &self,
        kind: CalamityKind,
        attention: AttentionTier,
        season: Season,
    ) -> Result<&CalamitySpec, CalamityRejectReason> {
        let spec = self.spec(kind).ok_or(CalamityRejectReason::UnknownKind)?;
        if attention < spec.minimum_attention {
            return Err(CalamityRejectReason::AttentionTooLow {
                required: spec.minimum_attention,
                actual: attention,
            });
        }
        if !kind.allowed_in_season(season) {
            return Err(CalamityRejectReason::SeasonBlocked { kind, season });
        }
        Ok(spec)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum CalamityRejectReason {
    UnknownKind,
    AttentionTooLow {
        required: AttentionTier,
        actual: AttentionTier,
    },
    SeasonBlocked {
        kind: CalamityKind,
        season: Season,
    },
    PowerInsufficient {
        current: f64,
        required: f64,
    },
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq)]
pub struct CalamityIntent {
    pub kind: Option<CalamityKind>,
    pub target_zone: Option<String>,
    pub target_player: Option<String>,
    pub intensity: f64,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PowerSpendEntry {
    pub tick: u64,
    pub calamity: CalamityKind,
    pub cost: f64,
    pub target: String,
    pub reason: String,
}

#[derive(Debug, Clone, Resource)]
pub struct TiandaoPower {
    pub current: f64,
    pub max: f64,
    pub regen_per_tick: f64,
    pub last_spend_tick: u64,
    pub spend_log: VecDeque<PowerSpendEntry>,
    last_regen_tick: u64,
}

impl Default for TiandaoPower {
    fn default() -> Self {
        Self {
            current: 100.0,
            max: 100.0,
            regen_per_tick: 0.005,
            last_spend_tick: 0,
            spend_log: VecDeque::new(),
            last_regen_tick: 0,
        }
    }
}

impl TiandaoPower {
    pub fn regen_to_tick(
        &mut self,
        tick: u64,
        average_zone_qi: f64,
        active_players: usize,
        season: Season,
    ) {
        if tick <= self.last_regen_tick {
            return;
        }

        let elapsed = tick - self.last_regen_tick;
        let mut multiplier = 1.0;
        if average_zone_qi > 0.6 {
            multiplier *= 0.8;
        } else if average_zone_qi < 0.25 {
            multiplier *= 1.5;
        }
        if active_players >= 10 {
            multiplier *= 1.2;
        }
        if season.is_xizhuan() {
            multiplier *= 0.7;
        }

        self.current =
            (self.current + self.regen_per_tick * elapsed as f64 * multiplier).clamp(0.0, self.max);
        self.last_regen_tick = tick;
    }

    pub fn try_spend(
        &mut self,
        kind: CalamityKind,
        cost: f64,
        target: impl Into<String>,
        reason: impl Into<String>,
        tick: u64,
    ) -> Result<(), CalamityRejectReason> {
        if self.current + f64::EPSILON < cost {
            return Err(CalamityRejectReason::PowerInsufficient {
                current: self.current,
                required: cost,
            });
        }

        self.current = (self.current - cost).max(0.0);
        self.last_spend_tick = tick;
        self.last_regen_tick = self.last_regen_tick.max(tick);
        self.spend_log.push_back(PowerSpendEntry {
            tick,
            calamity: kind,
            cost,
            target: target.into(),
            reason: reason.into(),
        });
        while self.spend_log.len() > TIANDAO_POWER_MAX_SPEND_LOG {
            self.spend_log.pop_front();
        }
        Ok(())
    }
}

pub fn attention_from_params(params: &HashMap<String, Value>) -> Option<AttentionTier> {
    params
        .get("attention_level")
        .or_else(|| params.get("attention"))
        .and_then(Value::as_str)
        .and_then(AttentionTier::from_wire)
}

pub fn reason_from_params(params: &HashMap<String, Value>) -> String {
    params
        .get("reason")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("agent spawn_event")
        .chars()
        .take(100)
        .collect()
}

pub fn target_key(zone_name: &str, target_player: Option<&str>) -> String {
    target_player
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| format!("player:{value}"))
        .unwrap_or_else(|| format!("zone:{zone_name}"))
}

pub fn register(app: &mut App) {
    app.insert_resource(CalamityArsenal::default());
    app.insert_resource(TiandaoPower::default());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_arsenal_registers_eight_calamities() {
        let arsenal = CalamityArsenal::default();
        assert_eq!(CalamityKind::ALL.len(), 8);
        assert!(CalamityKind::ALL
            .into_iter()
            .all(|kind| arsenal.spec(kind).is_some()));
        assert_eq!(
            arsenal.spec(CalamityKind::Thunder).map(|spec| spec.cost),
            Some(15.0)
        );
        assert_eq!(
            arsenal
                .spec(CalamityKind::RealmCollapse)
                .map(|spec| spec.minimum_attention),
            Some(AttentionTier::Annihilate)
        );
    }

    #[test]
    fn season_gates_exclusive_calamities() {
        let arsenal = CalamityArsenal::default();
        assert!(arsenal
            .allows(
                CalamityKind::HeavenlyFire,
                AttentionTier::Tribulation,
                Season::Summer
            )
            .is_ok());
        assert!(matches!(
            arsenal.allows(
                CalamityKind::HeavenlyFire,
                AttentionTier::Tribulation,
                Season::Winter
            ),
            Err(CalamityRejectReason::SeasonBlocked { .. })
        ));
        assert!(arsenal
            .allows(
                CalamityKind::PressureInvert,
                AttentionTier::Tribulation,
                Season::SummerToWinter
            )
            .is_ok());
        assert!(arsenal
            .allows(
                CalamityKind::AllWither,
                AttentionTier::Pressure,
                Season::Winter
            )
            .is_ok());
    }

    #[test]
    fn power_spend_regen_and_log_are_bounded() {
        let mut power = TiandaoPower {
            current: 0.0,
            ..Default::default()
        };
        power.regen_to_tick(20_000, 0.4, 1, Season::Summer);
        assert!((power.current - 100.0).abs() < f64::EPSILON);

        for tick in 20_001..20_030 {
            power
                .try_spend(CalamityKind::Thunder, 1.0, "zone:spawn", "unit-test", tick)
                .expect("spend should fit");
        }
        assert_eq!(power.spend_log.len(), TIANDAO_POWER_MAX_SPEND_LOG);
        assert_eq!(power.last_spend_tick, 20_029);
    }
}
