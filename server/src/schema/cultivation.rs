//! 修炼 IPC schema（plan §6.2 / §6.3）— Rust 侧。
//!
//! 与 TypeScript `agent/packages/schema/src/` 期望保持 1:1。TS 端文件命名：
//!   - insight-request.ts / insight-offer.ts
//!   - breakthrough-event.ts / forge-event.ts / biography.ts
//!   - death-event.ts 由战斗 plan 维护
//!
//! 注意：TS 尚未实现（跨仓库变更），此处先定 Rust 端以便 server 内部使用，
//! 等 TS 侧就绪再对齐。

use serde::{Deserialize, Serialize};

use crate::cultivation::components::{
    ColorKind, Cultivation, MeridianId, MeridianSystem, QiColor, Realm,
};
use crate::cultivation::life_record::SkillMilestone;

/// WorldStateV1.players[].cultivation（plan §6.3）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CultivationSnapshotV1 {
    pub realm: String,
    pub qi_current: f64,
    pub qi_max: f64,
    pub qi_max_frozen: f64,
    pub meridians_opened: u32,
    pub meridians_total: u32,
    pub qi_color_main: String,
    pub qi_color_secondary: Option<String>,
    pub qi_color_chaotic: bool,
    pub qi_color_hunyuan: bool,
    pub composure: f64,
}

impl CultivationSnapshotV1 {
    /// 从 ECS 组件构建下发快照（plan §6.3）。`meridians_total` 恒定 20。
    pub fn from_components(c: &Cultivation, m: &MeridianSystem, q: &QiColor) -> Self {
        Self {
            realm: realm_to_string(c.realm).to_string(),
            qi_current: c.qi_current,
            qi_max: c.qi_max,
            qi_max_frozen: c.qi_max_frozen.unwrap_or(0.0),
            meridians_opened: m.opened_count() as u32,
            meridians_total: 20,
            qi_color_main: color_kind_to_string(q.main).to_string(),
            qi_color_secondary: q.secondary.map(|s| color_kind_to_string(s).to_string()),
            qi_color_chaotic: q.is_chaotic,
            qi_color_hunyuan: q.is_hunyuan,
            composure: c.composure,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillMilestoneSnapshotV1 {
    pub skill: String,
    pub new_lv: u8,
    pub achieved_at: u64,
    pub narration: String,
    pub total_xp_at: u64,
}

impl SkillMilestoneSnapshotV1 {
    pub fn from_runtime(milestone: &SkillMilestone) -> Self {
        Self {
            skill: match milestone.skill {
                crate::skill::components::SkillId::Herbalism => "herbalism",
                crate::skill::components::SkillId::Alchemy => "alchemy",
                crate::skill::components::SkillId::Forging => "forging",
                crate::skill::components::SkillId::Combat => "combat",
                crate::skill::components::SkillId::Mineral => "mineral",
                crate::skill::components::SkillId::Cultivation => "cultivation",
            }
            .to_string(),
            new_lv: milestone.new_lv,
            achieved_at: milestone.achieved_at,
            narration: milestone.narration.clone(),
            total_xp_at: milestone.total_xp_at,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LifeRecordSnapshotV1 {
    pub recent_biography_summary: String,
    pub recent_skill_milestones_summary: String,
    pub skill_milestones: Vec<SkillMilestoneSnapshotV1>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InsightRequestV1 {
    pub trigger_id: String,
    pub character_id: String,
    pub realm: String,
    pub qi_color_state: QiColorStateV1,
    pub recent_biography: Vec<String>,
    pub composure: f64,
    pub available_categories: Vec<String>,
    pub global_caps: std::collections::HashMap<String, f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QiColorStateV1 {
    pub main: String,
    pub secondary: Option<String>,
    pub is_chaotic: bool,
    pub is_hunyuan: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InsightOfferV1 {
    pub offer_id: String,
    pub trigger_id: String,
    pub character_id: String,
    pub choices: Vec<InsightChoiceV1>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InsightChoiceV1 {
    pub category: String,
    pub effect_kind: String,
    pub magnitude: f64,
    pub flavor_text: String,
    pub narrator_voice: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeartDemonPregenRequestV1 {
    pub trigger_id: String,
    pub character_id: String,
    pub actor_name: String,
    pub realm: String,
    pub qi_color_state: QiColorStateV1,
    pub recent_biography: Vec<String>,
    pub composure: f64,
    pub started_tick: u64,
    pub waves_total: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BreakthroughEventV1 {
    pub kind: String, // Started/Succeeded/Failed
    pub from_realm: String,
    pub to_realm: Option<String>,
    pub success_rate: Option<f64>,
    pub severity: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForgeEventV1 {
    pub meridian: String,
    pub axis: String,
    pub from_tier: u8,
    pub to_tier: u8,
    pub success: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BiographyEntryV1 {
    BreakthroughStarted {
        realm_target: String,
        tick: u64,
    },
    BreakthroughSucceeded {
        realm: String,
        tick: u64,
    },
    BreakthroughFailed {
        realm_target: String,
        severity: f64,
        tick: u64,
    },
    MeridianOpened {
        id: String,
        tick: u64,
    },
    MeridianClosed {
        id: String,
        tick: u64,
        reason: String,
    },
    ForgedRate {
        id: String,
        tier: u8,
        tick: u64,
    },
    ForgedCapacity {
        id: String,
        tier: u8,
        tick: u64,
    },
    ColorShift {
        main: String,
        secondary: Option<String>,
        tick: u64,
    },
    InsightTaken {
        trigger: String,
        choice: String,
        tick: u64,
    },
    Rebirth {
        prior_realm: String,
        new_realm: String,
        tick: u64,
    },
    FalseSkinShed {
        kind: String,
        layers_shed: u8,
        contam_absorbed: f64,
        contam_overflow: f64,
        attacker_id: Option<String>,
        tick: u64,
    },
    SpawnTutorialCompleted {
        minutes_since_spawn: u32,
        tick: u64,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CultivationDeathV1 {
    pub cause: String,
    pub context: serde_json::Value,
}

/// 辅助：domain -> snapshot
pub fn realm_to_string(r: Realm) -> &'static str {
    match r {
        Realm::Awaken => "Awaken",
        Realm::Induce => "Induce",
        Realm::Condense => "Condense",
        Realm::Solidify => "Solidify",
        Realm::Spirit => "Spirit",
        Realm::Void => "Void",
    }
}

/// 辅助：snapshot -> domain
pub fn realm_from_string(name: &str) -> Realm {
    match name {
        "Awaken" => Realm::Awaken,
        "Induce" => Realm::Induce,
        "Condense" => Realm::Condense,
        "Solidify" => Realm::Solidify,
        "Spirit" => Realm::Spirit,
        "Void" => Realm::Void,
        _ => Realm::Awaken,
    }
}

pub fn meridian_id_to_string(id: MeridianId) -> &'static str {
    use MeridianId::*;
    match id {
        Lung => "Lung",
        LargeIntestine => "LargeIntestine",
        Stomach => "Stomach",
        Spleen => "Spleen",
        Heart => "Heart",
        SmallIntestine => "SmallIntestine",
        Bladder => "Bladder",
        Kidney => "Kidney",
        Pericardium => "Pericardium",
        TripleEnergizer => "TripleEnergizer",
        Gallbladder => "Gallbladder",
        Liver => "Liver",
        Ren => "Ren",
        Du => "Du",
        Chong => "Chong",
        Dai => "Dai",
        YinQiao => "YinQiao",
        YangQiao => "YangQiao",
        YinWei => "YinWei",
        YangWei => "YangWei",
    }
}

pub fn color_kind_to_string(c: ColorKind) -> &'static str {
    use ColorKind::*;
    match c {
        Sharp => "Sharp",
        Heavy => "Heavy",
        Mellow => "Mellow",
        Solid => "Solid",
        Light => "Light",
        Intricate => "Intricate",
        Gentle => "Gentle",
        Insidious => "Insidious",
        Violent => "Violent",
        Turbid => "Turbid",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn realm_name_stable() {
        assert_eq!(realm_to_string(Realm::Awaken), "Awaken");
        assert_eq!(realm_to_string(Realm::Void), "Void");
    }

    #[test]
    fn snapshot_from_components_matches_fields() {
        use crate::cultivation::components::{Cultivation, MeridianId, MeridianSystem, QiColor};
        let c = Cultivation {
            qi_current: 7.0,
            qi_max: 30.0,
            qi_max_frozen: Some(3.0),
            ..Default::default()
        };
        let mut m = MeridianSystem::default();
        m.get_mut(MeridianId::Lung).opened = true;
        m.get_mut(MeridianId::LargeIntestine).opened = true;
        let q = QiColor::default();
        let snap = CultivationSnapshotV1::from_components(&c, &m, &q);
        assert_eq!(snap.realm, "Awaken");
        assert_eq!(snap.meridians_opened, 2);
        assert_eq!(snap.meridians_total, 20);
        assert_eq!(snap.qi_max_frozen, 3.0);
        assert_eq!(snap.qi_color_main, "Mellow");
        assert!(snap.qi_color_secondary.is_none());
        assert!(!snap.qi_color_chaotic);
        assert!(!snap.qi_color_hunyuan);
    }

    #[test]
    fn snapshot_serde_round_trip() {
        let s = CultivationSnapshotV1 {
            realm: "Induce".into(),
            qi_current: 10.0,
            qi_max: 30.0,
            qi_max_frozen: 0.0,
            meridians_opened: 2,
            meridians_total: 20,
            qi_color_main: "Sharp".into(),
            qi_color_secondary: None,
            qi_color_chaotic: false,
            qi_color_hunyuan: false,
            composure: 0.85,
        };
        let json = serde_json::to_string(&s).unwrap();
        let back: CultivationSnapshotV1 = serde_json::from_str(&json).unwrap();
        assert_eq!(back.realm, "Induce");
    }
}
