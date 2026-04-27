use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LifespanEventKindV1 {
    Aging,
    DeathPenalty,
    Extension,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LifespanEventV1 {
    pub v: u8,
    pub character_id: String,
    pub at_tick: u64,
    pub kind: LifespanEventKindV1,
    pub delta_years: i64,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgingEventKindV1 {
    WindCandle,
    NaturalDeath,
    TickRate,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgingEventV1 {
    pub v: u8,
    pub character_id: String,
    pub at_tick: u64,
    pub kind: AgingEventKindV1,
    pub years_lived: f64,
    pub cap_by_realm: u32,
    pub remaining_years: f64,
    pub tick_rate_multiplier: f64,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DuoSheEventV1 {
    pub v: u8,
    pub host_id: String,
    pub target_id: String,
    pub at_tick: u64,
    pub karma_delta: f64,
    pub host_prev_age: f64,
    pub target_age: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lifecycle_events_serialize_snake_case_contracts() {
        let lifespan = serde_json::to_value(LifespanEventV1 {
            v: 1,
            character_id: "offline:Ancestor".to_string(),
            at_tick: 84_000,
            kind: LifespanEventKindV1::DeathPenalty,
            delta_years: -50,
            source: "bleed_out".to_string(),
        })
        .expect("lifespan event should serialize");
        assert_eq!(lifespan["kind"], "death_penalty");

        let aging = serde_json::to_value(AgingEventV1 {
            v: 1,
            character_id: "offline:Ancestor".to_string(),
            at_tick: 84_000,
            kind: AgingEventKindV1::WindCandle,
            years_lived: 940.5,
            cap_by_realm: 1000,
            remaining_years: 59.5,
            tick_rate_multiplier: 2.0,
            source: "zone_negative".to_string(),
        })
        .expect("aging event should serialize");
        assert_eq!(aging["kind"], "wind_candle");

        let duoshe = serde_json::to_value(DuoSheEventV1 {
            v: 1,
            host_id: "offline:Host".to_string(),
            target_id: "npc_1v0".to_string(),
            at_tick: 90_000,
            karma_delta: 100.0,
            host_prev_age: 77.0,
            target_age: 18.0,
        })
        .expect("duoshe event should serialize");
        assert_eq!(duoshe["host_id"], "offline:Host");
    }
}
