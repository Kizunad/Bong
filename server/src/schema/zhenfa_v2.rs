use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ZhenfaArrayKindV2 {
    Trap,
    Ward,
    ShrineWard,
    Lingju,
    DeceiveHeaven,
    Illusion,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ZhenfaV2EventKind {
    Deploy,
    Decay,
    Breakthrough,
    DeceiveHeavenExposed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ZhenfaV2EventV1 {
    pub v: u8,
    pub event: ZhenfaV2EventKind,
    pub array_id: u64,
    pub kind: ZhenfaArrayKindV2,
    pub owner: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zone: Option<String>,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub tick: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub radius: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub density_multiplier: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tiandao_gaze_weight: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reveal_chance_per_tick: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reveal_threshold: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub self_weight_multiplier: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_weight_multiplier: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub force_break: Option<bool>,
}

impl ZhenfaV2EventV1 {
    pub fn deploy(
        array_id: u64,
        kind: ZhenfaArrayKindV2,
        owner: impl Into<String>,
        pos: [i32; 3],
        tick: u64,
    ) -> Self {
        Self {
            v: 1,
            event: ZhenfaV2EventKind::Deploy,
            array_id,
            kind,
            owner: owner.into(),
            zone: None,
            x: pos[0],
            y: pos[1],
            z: pos[2],
            tick,
            radius: None,
            density_multiplier: None,
            tiandao_gaze_weight: None,
            reveal_chance_per_tick: None,
            reveal_threshold: None,
            self_weight_multiplier: None,
            target_weight_multiplier: None,
            force_break: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zhenfa_v2_event_uses_snake_case_contract() {
        let mut event = ZhenfaV2EventV1::deploy(
            7,
            ZhenfaArrayKindV2::DeceiveHeaven,
            "offline:Azure",
            [1, 64, -2],
            20,
        );
        event.event = ZhenfaV2EventKind::DeceiveHeavenExposed;
        event.reveal_chance_per_tick = Some(0.002);

        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["event"], "deceive_heaven_exposed");
        assert_eq!(json["kind"], "deceive_heaven");
        assert_eq!(json["reveal_chance_per_tick"], 0.002);
        assert!(json.get("zone").is_none());

        event.zone = Some("spawn".to_string());
        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["zone"], "spawn");
    }
}
