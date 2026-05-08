//! 天气事件 IPC schema（plan-lingtian-weather-v1 §4.2 数据契约）。
//!
//! 与 `agent/packages/schema/src/lingtian-weather.ts` 双向对拍：
//! - `WeatherEventKindV1` ↔ TS `WeatherEventKindV1`（snake_case wire）
//! - `WeatherEventDataV1` ↔ TS `WeatherEventDataV1`
//! - `WeatherEventUpdateV1` ↔ TS `WeatherEventUpdateV1`
//!
//! Redis pub channel：`bong:weather_event_update`（CHANNEL_WEATHER_EVENT_UPDATE）。

use serde::{Deserialize, Serialize};

use crate::lingtian::weather::WeatherEvent;

/// plan §3 — 5 类天气事件 wire。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WeatherEventKindV1 {
    Thunderstorm,
    DroughtWind,
    Blizzard,
    HeavyHaze,
    LingMist,
}

impl From<WeatherEvent> for WeatherEventKindV1 {
    fn from(value: WeatherEvent) -> Self {
        match value {
            WeatherEvent::Thunderstorm => Self::Thunderstorm,
            WeatherEvent::DroughtWind => Self::DroughtWind,
            WeatherEvent::Blizzard => Self::Blizzard,
            WeatherEvent::HeavyHaze => Self::HeavyHaze,
            WeatherEvent::LingMist => Self::LingMist,
        }
    }
}

impl From<WeatherEventKindV1> for WeatherEvent {
    fn from(value: WeatherEventKindV1) -> Self {
        match value {
            WeatherEventKindV1::Thunderstorm => Self::Thunderstorm,
            WeatherEventKindV1::DroughtWind => Self::DroughtWind,
            WeatherEventKindV1::Blizzard => Self::Blizzard,
            WeatherEventKindV1::HeavyHaze => Self::HeavyHaze,
            WeatherEventKindV1::LingMist => Self::LingMist,
        }
    }
}

/// plan §4.2 — 单个天气事件的 wire payload。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WeatherEventDataV1 {
    pub v: u8,
    pub zone_id: String,
    pub kind: WeatherEventKindV1,
    pub started_at_lingtian_tick: u64,
    pub expires_at_lingtian_tick: u64,
    pub remaining_ticks: u64,
}

impl WeatherEventDataV1 {
    pub fn new(
        zone_id: impl Into<String>,
        kind: impl Into<WeatherEventKindV1>,
        started_at_lingtian_tick: u64,
        expires_at_lingtian_tick: u64,
        now_lingtian_tick: u64,
    ) -> Self {
        Self {
            v: 1,
            zone_id: zone_id.into(),
            kind: kind.into(),
            started_at_lingtian_tick,
            expires_at_lingtian_tick,
            remaining_ticks: expires_at_lingtian_tick.saturating_sub(now_lingtian_tick),
        }
    }
}

/// plan §4.4 — Redis pub envelope（`bong:weather_event_update`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WeatherEventUpdateKindV1 {
    Started,
    Expired,
    Cleared,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WeatherEventUpdateV1 {
    pub v: u8,
    pub kind: WeatherEventUpdateKindV1,
    pub data: WeatherEventDataV1,
}

impl WeatherEventUpdateV1 {
    pub fn started(data: WeatherEventDataV1) -> Self {
        Self {
            v: 1,
            kind: WeatherEventUpdateKindV1::Started,
            data,
        }
    }

    pub fn expired(data: WeatherEventDataV1) -> Self {
        Self {
            v: 1,
            kind: WeatherEventUpdateKindV1::Expired,
            data,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weather_event_kind_round_trip_all_variants() {
        // 5 个变体的 enum ↔ schema enum 转换 + serde 双向对拍
        for ev in WeatherEvent::all() {
            let kind: WeatherEventKindV1 = ev.into();
            let back: WeatherEvent = kind.into();
            assert_eq!(ev, back, "{ev:?} round-trip 失败");
            // serde wire round-trip
            let json = serde_json::to_string(&kind).expect("serialize");
            let parsed: WeatherEventKindV1 = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(parsed, kind);
        }
    }

    #[test]
    fn weather_event_kind_wire_matches_lingtian_weather_v1_as_wire_str() {
        // 与 `WeatherEvent::as_wire_str` 必须严格一致（双端对齐 source of truth）。
        for ev in WeatherEvent::all() {
            let kind: WeatherEventKindV1 = ev.into();
            let json = serde_json::to_string(&kind).expect("serialize");
            let unquoted = json.trim_matches('"');
            assert_eq!(
                unquoted,
                ev.as_wire_str(),
                "{ev:?} wire 字符串不匹配：schema={unquoted} vs WeatherEvent={}",
                ev.as_wire_str()
            );
        }
    }

    #[test]
    fn weather_event_data_v1_serializes_with_correct_fields() {
        let data = WeatherEventDataV1::new("default", WeatherEvent::Thunderstorm, 1440, 1620, 1500);
        assert_eq!(data.v, 1);
        assert_eq!(data.zone_id, "default");
        assert_eq!(data.kind, WeatherEventKindV1::Thunderstorm);
        assert_eq!(data.remaining_ticks, 120); // 1620 - 1500

        let json = serde_json::to_value(&data).expect("serialize");
        assert_eq!(json["v"], 1);
        assert_eq!(json["zone_id"], "default");
        assert_eq!(json["kind"], "thunderstorm");
        assert_eq!(json["started_at_lingtian_tick"], 1440);
        assert_eq!(json["expires_at_lingtian_tick"], 1620);
        assert_eq!(json["remaining_ticks"], 120);
    }

    #[test]
    fn weather_event_update_v1_started_envelope() {
        let data = WeatherEventDataV1::new("default", WeatherEvent::LingMist, 5760, 5820, 5760);
        let env = WeatherEventUpdateV1::started(data.clone());
        assert_eq!(env.v, 1);
        assert_eq!(env.kind, WeatherEventUpdateKindV1::Started);
        let json = serde_json::to_value(&env).expect("serialize");
        assert_eq!(json["kind"], "started");
        assert_eq!(json["data"]["kind"], "ling_mist");
    }

    #[test]
    fn weather_event_update_v1_round_trip_via_json() {
        let data = WeatherEventDataV1::new("default", WeatherEvent::Blizzard, 1000, 2440, 1500);
        let env = WeatherEventUpdateV1::expired(data);
        let json = serde_json::to_string(&env).expect("serialize");
        let parsed: WeatherEventUpdateV1 = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, env);
    }

    #[test]
    fn weather_event_update_v1_loads_sample_from_agent_packages_schema() {
        // 与 agent/packages/schema/samples/weather-event-update.sample.json 双端对拍
        let raw = r#"{
            "v": 1,
            "kind": "started",
            "data": {
                "v": 1,
                "zone_id": "default",
                "kind": "ling_mist",
                "started_at_lingtian_tick": 5760,
                "expires_at_lingtian_tick": 5820,
                "remaining_ticks": 60
            }
        }"#;
        let parsed: WeatherEventUpdateV1 =
            serde_json::from_str(raw).expect("sample 应当通过 Rust serde");
        assert_eq!(parsed.kind, WeatherEventUpdateKindV1::Started);
        assert_eq!(parsed.data.kind, WeatherEventKindV1::LingMist);
        assert_eq!(parsed.data.zone_id, "default");
        assert_eq!(parsed.data.remaining_ticks, 60);
    }
}
