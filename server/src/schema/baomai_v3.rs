use serde::{Deserialize, Serialize};

pub const BAOMAI_SKILL_EVENT_TYPE: &str = "baomai_skill_event";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BaomaiSkillIdV1 {
    BengQuan,
    FullPowerCharge,
    FullPowerRelease,
    MountainShake,
    BloodBurn,
    Disperse,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BaomaiSkillEventV1 {
    pub v: u8,
    #[serde(rename = "type")]
    pub event_type: String,
    pub skill_id: BaomaiSkillIdV1,
    pub caster_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_id: Option<String>,
    pub tick: u64,
    pub qi_invested: f64,
    pub damage: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub radius_blocks: Option<f32>,
    pub blood_multiplier: f32,
    pub flow_rate_multiplier: f64,
    pub meridian_ids: Vec<String>,
}

impl BaomaiSkillEventV1 {
    pub fn new(skill_id: BaomaiSkillIdV1, caster_id: String, tick: u64) -> Self {
        Self {
            v: 1,
            event_type: BAOMAI_SKILL_EVENT_TYPE.to_string(),
            skill_id,
            caster_id,
            target_id: None,
            tick,
            qi_invested: 0.0,
            damage: 0.0,
            radius_blocks: None,
            blood_multiplier: 1.0,
            flow_rate_multiplier: 1.0,
            meridian_ids: Vec::new(),
        }
    }
}

// plan-combat-skill-feedback-bridges-v1 P2 — 爆脉 v3 残余事件 schema

/// 山震 AoE 震波事件（bong:baomai_v3/mountain_shake）
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BaomaiV3MountainShakeV1 {
    pub v: u8,
    pub caster_id: String,
    pub affected_count: usize,
    pub tick: u64,
    pub qi_spent: f64,
    pub radius_blocks: f32,
    pub shock_damage: f32,
}

impl BaomaiV3MountainShakeV1 {
    pub fn new(caster_id: String, affected_count: usize, tick: u64) -> Self {
        Self {
            v: 1,
            caster_id,
            affected_count,
            tick,
            qi_spent: 0.0,
            radius_blocks: 0.0,
            shock_damage: 0.0,
        }
    }
}

/// 血燃 HP→真元倍率事件（bong:baomai_v3/blood_burn）
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BaomaiV3BloodBurnV1 {
    pub v: u8,
    pub caster_id: String,
    pub tick: u64,
    pub hp_burned: f32,
    pub qi_multiplier: f32,
    pub active_until_tick: u64,
    pub ended_in_near_death: bool,
}

impl BaomaiV3BloodBurnV1 {
    pub fn new(caster_id: String, tick: u64) -> Self {
        Self {
            v: 1,
            caster_id,
            tick,
            hp_burned: 0.0,
            qi_multiplier: 1.0,
            active_until_tick: 0,
            ended_in_near_death: false,
        }
    }
}

/// 超越到期事件（bong:baomai_v3/transcendence_expired）
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BaomaiV3TranscendenceExpiredV1 {
    pub v: u8,
    pub caster_id: String,
    pub tick: u64,
}

impl BaomaiV3TranscendenceExpiredV1 {
    pub fn new(caster_id: String, tick: u64) -> Self {
        Self {
            v: 1,
            caster_id,
            tick,
        }
    }
}

/// 过载涟漪事件（bong:baomai_v3/overload_ripple）
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BaomaiV3OverloadRippleV1 {
    pub v: u8,
    pub caster_id: String,
    pub tick: u64,
    pub skill_id: BaomaiSkillIdV1,
    pub severity_delta: f64,
    pub total_severity: f64,
    pub meridian_ids: Vec<String>,
}

impl BaomaiV3OverloadRippleV1 {
    pub fn new(caster_id: String, tick: u64, skill_id: BaomaiSkillIdV1) -> Self {
        Self {
            v: 1,
            caster_id,
            tick,
            skill_id,
            severity_delta: 0.0,
            total_severity: 0.0,
            meridian_ids: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn baomai_skill_event_serializes_snake_case_skill_and_type() {
        let mut event =
            BaomaiSkillEventV1::new(BaomaiSkillIdV1::Disperse, "char:7".to_string(), 42);
        event.flow_rate_multiplier = 10.0;
        event.meridian_ids = vec!["Ren".to_string(), "Du".to_string()];

        let json = serde_json::to_value(&event).expect("serialize baomai event");

        assert_eq!(json["type"], BAOMAI_SKILL_EVENT_TYPE);
        assert_eq!(json["skill_id"], "disperse");
        assert_eq!(json["flow_rate_multiplier"], 10.0);
        assert_eq!(json["meridian_ids"][0], "Ren");
    }

    // plan-combat-skill-feedback-bridges-v1 P2 — schema struct pin tests

    #[test]
    fn mountain_shake_v1_serializes_expected_fields() {
        let mut evt = BaomaiV3MountainShakeV1::new("offline:Player".to_string(), 3, 200);
        evt.qi_spent = 1200.0;
        evt.radius_blocks = 5.0;
        evt.shock_damage = 420.0;
        let json = serde_json::to_value(&evt).expect("serialize BaomaiV3MountainShakeV1");
        assert_eq!(json["v"], 1);
        assert_eq!(json["caster_id"], "offline:Player");
        assert_eq!(json["affected_count"], 3);
        assert_eq!(json["qi_spent"], 1200.0);
        assert_eq!(json["radius_blocks"], 5.0);
        assert_eq!(json["shock_damage"], 420.0);
    }

    #[test]
    fn mountain_shake_v1_rejects_unknown_field() {
        let bad_json = r#"{"v":1,"caster_id":"x","affected_count":0,"tick":1,"qi_spent":1.0,"radius_blocks":1.0,"shock_damage":1.0,"extra":"forbidden"}"#;
        let result = serde_json::from_str::<BaomaiV3MountainShakeV1>(bad_json);
        assert!(
            result.is_err(),
            "deny_unknown_fields must reject unknown keys; expected Err but got {:?}",
            result
        );
    }

    #[test]
    fn blood_burn_v1_serializes_near_death_field() {
        let mut evt = BaomaiV3BloodBurnV1::new("offline:Player".to_string(), 300);
        evt.hp_burned = 300.0;
        evt.qi_multiplier = 5.0;
        evt.active_until_tick = 300;
        evt.ended_in_near_death = true;
        let json = serde_json::to_value(&evt).expect("serialize BaomaiV3BloodBurnV1");
        assert_eq!(json["ended_in_near_death"], true);
        assert_eq!(json["qi_multiplier"], 5.0);
    }

    #[test]
    fn blood_burn_v1_normal_path_not_near_death() {
        let mut evt = BaomaiV3BloodBurnV1::new("offline:Player".to_string(), 300);
        evt.hp_burned = 150.0;
        evt.qi_multiplier = 3.5;
        evt.active_until_tick = 360;
        evt.ended_in_near_death = false;
        let json = serde_json::to_value(&evt).expect("serialize BaomaiV3BloodBurnV1 normal");
        assert_eq!(json["ended_in_near_death"], false);
    }

    #[test]
    fn transcendence_expired_v1_minimal_fields() {
        let evt = BaomaiV3TranscendenceExpiredV1::new("offline:Player".to_string(), 700);
        let json = serde_json::to_value(&evt).expect("serialize BaomaiV3TranscendenceExpiredV1");
        assert_eq!(json["v"], 1);
        assert_eq!(json["caster_id"], "offline:Player");
        assert_eq!(json["tick"], 700);
    }

    #[test]
    fn transcendence_expired_v1_rejects_unknown_field() {
        let bad_json = r#"{"v":1,"caster_id":"x","tick":1,"extra":"forbidden"}"#;
        let result = serde_json::from_str::<BaomaiV3TranscendenceExpiredV1>(bad_json);
        assert!(
            result.is_err(),
            "deny_unknown_fields should reject extra field"
        );
    }

    #[test]
    fn overload_ripple_v1_serializes_skill_id_snake_case() {
        let mut evt = BaomaiV3OverloadRippleV1::new(
            "offline:Player".to_string(),
            150,
            BaomaiSkillIdV1::BengQuan,
        );
        evt.severity_delta = 0.05;
        evt.total_severity = 0.35;
        evt.meridian_ids = vec!["LargeIntestine".to_string(), "Lung".to_string()];
        let json = serde_json::to_value(&evt).expect("serialize BaomaiV3OverloadRippleV1");
        assert_eq!(json["skill_id"], "beng_quan");
        assert_eq!(json["severity_delta"], 0.05);
        assert_eq!(json["meridian_ids"][0], "LargeIntestine");
    }

    #[test]
    fn overload_ripple_v1_all_skill_ids_serialize_snake_case() {
        let pairs = [
            (BaomaiSkillIdV1::BengQuan, "beng_quan"),
            (BaomaiSkillIdV1::FullPowerCharge, "full_power_charge"),
            (BaomaiSkillIdV1::FullPowerRelease, "full_power_release"),
            (BaomaiSkillIdV1::MountainShake, "mountain_shake"),
            (BaomaiSkillIdV1::BloodBurn, "blood_burn"),
            (BaomaiSkillIdV1::Disperse, "disperse"),
        ];
        for (skill, expected) in pairs {
            let evt = BaomaiV3OverloadRippleV1::new("x".to_string(), 0, skill);
            let json = serde_json::to_value(&evt).unwrap();
            assert_eq!(
                json["skill_id"], expected,
                "skill_id {skill:?} should serialize as '{expected}'"
            );
        }
    }

    // plan-combat-skill-feedback-bridges-v1 P2 — 双端 sample 对拍（Rust from_str）
    // 与 TS 端 baomai-v3-p2.test.ts 消费同一批 samples/*.json 文件，确保双端一致。

    #[test]
    fn mountain_shake_v1_deserializes_from_shared_sample() {
        // 反序列化共享 sample（与 TS 端 loadSamples 对拍同一文件）
        const RAW: &str =
            include_str!("../../../agent/packages/schema/samples/baomai_v3_mountain_shake.json");
        let samples: Vec<BaomaiV3MountainShakeV1> =
            serde_json::from_str(RAW).expect("baomai_v3_mountain_shake.json must deserialize");

        assert_eq!(samples.len(), 2, "sample file should have 2 entries");

        // entry 0: offline:TestPlayer, affected_count=3
        let s0 = &samples[0];
        assert_eq!(s0.v, 1);
        assert_eq!(s0.caster_id, "offline:TestPlayer");
        assert_eq!(s0.affected_count, 3);
        assert!((s0.qi_spent - 1200.0).abs() < 1e-6, "qi_spent mismatch");
        assert!(
            (s0.radius_blocks - 5.0).abs() < 1e-3,
            "radius_blocks mismatch"
        );

        // entry 1: char:9876543210, affected_count=0
        let s1 = &samples[1];
        assert_eq!(s1.caster_id, "char:9876543210");
        assert_eq!(s1.affected_count, 0);
    }

    #[test]
    fn blood_burn_v1_deserializes_from_shared_sample() {
        const RAW: &str =
            include_str!("../../../agent/packages/schema/samples/baomai_v3_blood_burn.json");
        let samples: Vec<BaomaiV3BloodBurnV1> =
            serde_json::from_str(RAW).expect("baomai_v3_blood_burn.json must deserialize");

        assert_eq!(samples.len(), 2, "sample file should have 2 entries");

        // entry 0: normal path, not near death
        let s0 = &samples[0];
        assert_eq!(s0.v, 1);
        assert_eq!(s0.caster_id, "offline:TestPlayer");
        assert!(!s0.ended_in_near_death, "entry 0 should not be near-death");
        assert!(
            (s0.qi_multiplier - 3.5).abs() < 1e-3,
            "qi_multiplier mismatch"
        );

        // entry 1: near-death path
        let s1 = &samples[1];
        assert!(s1.ended_in_near_death, "entry 1 should be near-death");
        assert!(
            (s1.qi_multiplier - 5.0).abs() < 1e-3,
            "qi_multiplier mismatch"
        );
    }

    #[test]
    fn transcendence_expired_v1_deserializes_from_shared_sample() {
        const RAW: &str = include_str!(
            "../../../agent/packages/schema/samples/baomai_v3_transcendence_expired.json"
        );
        let samples: Vec<BaomaiV3TranscendenceExpiredV1> = serde_json::from_str(RAW)
            .expect("baomai_v3_transcendence_expired.json must deserialize");

        assert_eq!(samples.len(), 2, "sample file should have 2 entries");

        // entry 0
        let s0 = &samples[0];
        assert_eq!(s0.v, 1);
        assert_eq!(s0.caster_id, "offline:TestPlayer");
        assert_eq!(s0.tick, 700);

        // entry 1
        let s1 = &samples[1];
        assert_eq!(s1.caster_id, "char:12345678901234567");
        assert_eq!(s1.tick, 1200);
    }

    #[test]
    fn overload_ripple_v1_deserializes_from_shared_sample() {
        const RAW: &str =
            include_str!("../../../agent/packages/schema/samples/baomai_v3_overload_ripple.json");
        let samples: Vec<BaomaiV3OverloadRippleV1> =
            serde_json::from_str(RAW).expect("baomai_v3_overload_ripple.json must deserialize");

        assert_eq!(samples.len(), 2, "sample file should have 2 entries");

        // entry 0: beng_quan, severity 0.35
        let s0 = &samples[0];
        assert_eq!(s0.v, 1);
        assert_eq!(s0.caster_id, "offline:TestPlayer");
        assert_eq!(s0.skill_id, BaomaiSkillIdV1::BengQuan);
        assert!(
            (s0.total_severity - 0.35).abs() < 1e-6,
            "total_severity mismatch"
        );
        assert_eq!(
            s0.meridian_ids.len(),
            3,
            "entry 0 should have 3 meridian_ids"
        );

        // entry 1: mountain_shake, severity 0.72
        let s1 = &samples[1];
        assert_eq!(s1.skill_id, BaomaiSkillIdV1::MountainShake);
        assert!(
            (s1.total_severity - 0.72).abs() < 1e-6,
            "total_severity mismatch"
        );
        assert_eq!(s1.meridian_ids[0], "Stomach");
    }
}
