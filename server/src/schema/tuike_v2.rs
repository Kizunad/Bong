use serde::{Deserialize, Serialize};

pub const TUIKE_V2_SKILL_EVENT_TYPE: &str = "tuike_v2_skill_event";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TuikeSkillIdV1 {
    Don,
    Shed,
    TransferTaint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FalseSkinTierV1 {
    Fan,
    Light,
    Mid,
    Heavy,
    Ancient,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TuikeSkillEventV1 {
    #[serde(default = "default_version")]
    pub v: u8,
    #[serde(rename = "type", default = "default_event_type")]
    pub event_type: String,
    pub caster_id: String,
    pub skill_id: TuikeSkillIdV1,
    pub tier: FalseSkinTierV1,
    pub layers_after: u8,
    pub contam_moved_percent: f64,
    pub permanent_absorbed: f64,
    pub qi_cost: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub damage_absorbed: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub damage_overflow: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contam_load: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_shed: Option<bool>,
    pub tick: u64,
    pub animation_id: String,
    pub particle_id: String,
    pub sound_recipe_id: String,
    pub icon_texture: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TuikeSkillVisualContractV1 {
    pub animation_id: String,
    pub particle_id: String,
    pub sound_recipe_id: String,
    pub icon_texture: String,
}

impl TuikeSkillVisualContractV1 {
    pub fn new(
        animation_id: impl Into<String>,
        particle_id: impl Into<String>,
        sound_recipe_id: impl Into<String>,
        icon_texture: impl Into<String>,
    ) -> Self {
        Self {
            animation_id: non_empty_visual_field("animation_id", animation_id.into()),
            particle_id: non_empty_visual_field("particle_id", particle_id.into()),
            sound_recipe_id: non_empty_visual_field("sound_recipe_id", sound_recipe_id.into()),
            icon_texture: non_empty_visual_field("icon_texture", icon_texture.into()),
        }
    }
}

impl TuikeSkillEventV1 {
    pub fn new(
        caster_id: String,
        skill_id: TuikeSkillIdV1,
        tier: FalseSkinTierV1,
        layers_after: u8,
        tick: u64,
        visual: TuikeSkillVisualContractV1,
    ) -> Self {
        let visual = visual.validated();
        Self {
            v: default_version(),
            event_type: default_event_type(),
            caster_id,
            skill_id,
            tier,
            layers_after,
            contam_moved_percent: 0.0,
            permanent_absorbed: 0.0,
            qi_cost: 0.0,
            damage_absorbed: None,
            damage_overflow: None,
            contam_load: None,
            active_shed: None,
            tick,
            animation_id: visual.animation_id,
            particle_id: visual.particle_id,
            sound_recipe_id: visual.sound_recipe_id,
            icon_texture: visual.icon_texture,
        }
    }
}

impl TuikeSkillVisualContractV1 {
    fn validated(self) -> Self {
        Self::new(
            self.animation_id,
            self.particle_id,
            self.sound_recipe_id,
            self.icon_texture,
        )
    }
}

fn non_empty_visual_field(field: &str, value: String) -> String {
    assert!(!value.is_empty(), "{field} must not be empty");
    value
}

const fn default_version() -> u8 {
    1
}

fn default_event_type() -> String {
    TUIKE_V2_SKILL_EVENT_TYPE.to_string()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FalseSkinStackStateV1 {
    pub owner: String,
    pub layers: Vec<FalseSkinLayerStateV1>,
    pub naked_until_tick: u64,
    pub transfer_permanent_cooldown_until_tick: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FalseSkinLayerStateV1 {
    pub tier: FalseSkinTierV1,
    pub spirit_quality: f64,
    pub damage_capacity: f64,
    pub contam_load: f64,
    pub permanent_taint_load: f64,
}

/// plan-combat-skill-feedback-bridges-v1 P6 — 蜕壳灰烬入包事件（server → agent Redis）。
///
/// 触发条件：FalseSkinResidue 超时自然腐烂时产出灰烬物品，回收给皮的原主人。
/// agent 叙事：「衰败的假皮化为灰烬，XXX 回收了 <output_item_id>」。
/// 守恒说明：此事件只记录 add_item_to_player_inventory 已完成的结果，不重算真元。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TuikeAshDecayV1 {
    /// 接收灰烬的玩家 id（offline:<username> 或 char:<bits>）
    pub owner_id: String,
    /// 产出物品 id（普通档 = FALSE_SKIN_ASH_ITEM_ID；上古档 = FALSE_SKIN_ANCIENT_RELIC_SHARD_ITEM_ID）
    pub output_item_id: String,
    /// 皮的档级（agent 叙事差异化：上古档有专属文案）
    pub tier: FalseSkinTierV1,
    /// 服务端 tick
    pub tick: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    // plan-combat-skill-feedback-bridges-v1 P6 — server↔TS 双端 sample-pin。
    // 字段名漂移（如 owner_id → ownerId）会让此测试红，无需人工对比。
    const SAMPLE_TUIKE_ASH_DECAY: &str =
        include_str!("../../../agent/packages/schema/samples/tuike_v2_ash_decay.json");

    #[test]
    fn tuike_ash_decay_sample_pin_all_tiers() {
        // sample 是 JSON array，每条均反序列化为 TuikeAshDecayV1 并 roundtrip。
        let values: Vec<serde_json::Value> = serde_json::from_str(SAMPLE_TUIKE_ASH_DECAY)
            .expect("tuike_v2_ash_decay.json 必须是合法 JSON array");
        assert!(
            !values.is_empty(),
            "tuike_v2_ash_decay.json sample array 不能为空"
        );
        for (i, value) in values.iter().enumerate() {
            let payload: TuikeAshDecayV1 =
                serde_json::from_value(value.clone()).unwrap_or_else(|e| {
                    panic!(
                        "tuike_v2_ash_decay.json sample[{i}] 反序列化失败：{e}\n\
                        原因通常是 TS↔Rust 字段名漂移或新增必填字段未同步。\n\
                        value={value}"
                    )
                });
            let back = serde_json::to_value(&payload).unwrap();
            let _: TuikeAshDecayV1 = serde_json::from_value(back).unwrap_or_else(|e| {
                panic!("sample[{i}] roundtrip 失败：{e}");
            });
        }
        // 精确覆盖 5 档（fan/light/mid/heavy/ancient）
        assert_eq!(
            values.len(),
            5,
            "tuike_v2_ash_decay.json 应包含 5 条 sample（fan/light/mid/heavy/ancient 各 1 条）；\
            实际 {} 条。新增或删减 tier 时必须同步更新 sample 文件。",
            values.len()
        );
    }

    #[test]
    fn tuike_skill_event_roundtrip_preserves_visual_contract() {
        let event = TuikeSkillEventV1 {
            v: 1,
            event_type: TUIKE_V2_SKILL_EVENT_TYPE.to_string(),
            caster_id: "offline:Azure".to_string(),
            skill_id: TuikeSkillIdV1::TransferTaint,
            tier: FalseSkinTierV1::Ancient,
            layers_after: 2,
            contam_moved_percent: 15.0,
            permanent_absorbed: 0.2,
            qi_cost: 105.0,
            damage_absorbed: None,
            damage_overflow: None,
            contam_load: Some(15.0),
            active_shed: None,
            tick: 9,
            animation_id: "bong:tuike_taint_transfer".to_string(),
            particle_id: "bong:ancient_skin_glow".to_string(),
            sound_recipe_id: "contam_transfer_hum".to_string(),
            icon_texture: "bong-client:textures/gui/items/skill_scroll_tuike_transfer_taint.png"
                .to_string(),
        };
        let json = serde_json::to_string(&event).expect("serialize");
        let parsed: TuikeSkillEventV1 = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, event);
    }

    #[test]
    #[should_panic(expected = "animation_id must not be empty")]
    fn tuike_skill_event_constructor_rejects_empty_visual_contract() {
        let _ = TuikeSkillEventV1::new(
            "offline:Azure".to_string(),
            TuikeSkillIdV1::Don,
            FalseSkinTierV1::Fan,
            1,
            9,
            TuikeSkillVisualContractV1 {
                animation_id: String::new(),
                particle_id: "bong:false_skin_don_dust".to_string(),
                sound_recipe_id: "don_skin_low_thud".to_string(),
                icon_texture: "bong-client:textures/gui/items/skill_scroll_tuike_don.png"
                    .to_string(),
            },
        );
    }
}
