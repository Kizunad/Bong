use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DeathCinematicPhaseV1 {
    Predeath,
    DeathMoment,
    Roll,
    InsightOverlay,
    Darkness,
    Rebirth,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DeathRollResultV1 {
    Pending,
    Survive,
    Fall,
    Final,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DeathCinematicZoneKindV1 {
    Ordinary,
    Death,
    Negative,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct DeathCinematicRollV1 {
    pub probability: f64,
    pub threshold: f64,
    pub luck_value: f64,
    pub result: DeathRollResultV1,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct DeathCinematicS2cV1 {
    pub v: u8,
    pub character_id: String,
    pub phase: DeathCinematicPhaseV1,
    pub phase_tick: u64,
    pub phase_duration_ticks: u64,
    pub total_elapsed_ticks: u64,
    pub total_duration_ticks: u64,
    pub roll: DeathCinematicRollV1,
    pub insight_text: Vec<String>,
    pub is_final: bool,
    pub death_number: u32,
    pub zone_kind: DeathCinematicZoneKindV1,
    pub tsy_death: bool,
    pub rebirth_weakened_ticks: u64,
    pub skip_predeath: bool,
}
