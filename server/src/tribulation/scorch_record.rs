//! plan-terrain-tribulation-scorch-v1 P3 — 实时天劫落点写回焦土标记的纯模型。
//!
//! 真正的块写入由后续 world persistence 消费；这里先锁定事件命中 zone 后应生成
//! `glass_fulgurite` 记号的契约，避免把天劫地理后果散落到 narration 或天气层。

pub const GLASS_FULGURITE_MARKER_ID: &str = "glass_fulgurite";
pub const TRIBULATION_SCORCH_EVENT: &str = "tribulation_scorch";

#[derive(Debug, Clone, PartialEq)]
pub struct ScorchRecord {
    pub zone_id: String,
    pub marker_id: String,
    pub pos_xyz: [f64; 3],
    pub created_at_tick: u64,
    pub source_event: String,
}

pub fn build_scorch_record(
    zone_id: &str,
    zone_active_events: &[String],
    epicenter: Option<[f64; 3]>,
    created_at_tick: u64,
) -> Option<ScorchRecord> {
    if !is_tribulation_scorch_zone(zone_id, zone_active_events) {
        return None;
    }
    let pos_xyz = epicenter?;
    if !pos_xyz.iter().all(|value| value.is_finite()) {
        return None;
    }
    Some(ScorchRecord {
        zone_id: zone_id.to_string(),
        marker_id: GLASS_FULGURITE_MARKER_ID.to_string(),
        pos_xyz,
        created_at_tick,
        source_event: TRIBULATION_SCORCH_EVENT.to_string(),
    })
}

pub fn is_tribulation_scorch_zone(zone_id: &str, zone_active_events: &[String]) -> bool {
    zone_id.contains("scorch")
        || zone_active_events
            .iter()
            .any(|event| event == TRIBULATION_SCORCH_EVENT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tribulation_hit_in_scorch_zone_records_glass_fulgurite_marker() {
        let record = build_scorch_record(
            "north_waste_east_scorch",
            &[TRIBULATION_SCORCH_EVENT.to_string()],
            Some([2100.0, 80.0, -8000.0]),
            42,
        )
        .expect("scorch zone should record terrain marker");

        assert_eq!(record.marker_id, GLASS_FULGURITE_MARKER_ID);
        assert_eq!(record.pos_xyz, [2100.0, 80.0, -8000.0]);
        assert_eq!(record.created_at_tick, 42);
    }

    #[test]
    fn non_scorch_zone_does_not_record_marker() {
        let record = build_scorch_record("spawn", &[], Some([0.0, 70.0, 0.0]), 1);

        assert!(record.is_none());
    }

    #[test]
    fn missing_or_invalid_epicenter_is_not_recorded() {
        assert!(build_scorch_record("drift_scorch_001", &[], None, 1).is_none());
        assert!(
            build_scorch_record("drift_scorch_001", &[], Some([f64::NAN, 70.0, 0.0]), 1).is_none()
        );
    }
}
