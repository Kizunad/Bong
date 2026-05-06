use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ZonePressureCrossedV1 {
    pub v: u8,
    pub kind: String,
    pub zone: String,
    pub level: String,
    pub raw_pressure: f32,
    pub at_tick: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zone_pressure_crossed_wire_shape_is_stable() {
        let payload = ZonePressureCrossedV1 {
            v: 1,
            kind: "zone_pressure_crossed".to_string(),
            zone: "spawn".to_string(),
            level: "high".to_string(),
            raw_pressure: 1.25,
            at_tick: 42,
        };

        let value = serde_json::to_value(payload).expect("serialize");
        assert_eq!(value["kind"], "zone_pressure_crossed");
        assert_eq!(value["level"], "high");
        assert_eq!(value["at_tick"], 42);
    }
}
