use serde::{Deserialize, Serialize};
use valence::prelude::{bevy_ecs, Component};

#[derive(Debug, Clone, Component, Serialize, Deserialize, PartialEq)]
pub struct KnownTechniques {
    pub entries: Vec<KnownTechnique>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KnownTechnique {
    pub id: String,
    pub proficiency: f32,
    pub active: bool,
}

impl Default for KnownTechniques {
    fn default() -> Self {
        Self {
            entries: TECHNIQUE_IDS
                .iter()
                .map(|id| KnownTechnique {
                    id: (*id).to_string(),
                    proficiency: 0.5,
                    active: true,
                })
                .collect(),
        }
    }
}

const TECHNIQUE_IDS: [&str; 5] = [
    "burst_meridian.beng_quan",
    "burst_meridian.tie_shan_kao",
    "burst_meridian.xue_beng_bu",
    "burst_meridian.ni_mai_hu_ti",
    "woliu.vortex",
];

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TechniqueDefinition {
    pub id: &'static str,
    pub display_name: &'static str,
    pub grade: &'static str,
    pub description: &'static str,
    pub required_realm: &'static str,
    pub required_meridians: &'static [TechniqueRequiredMeridian],
    pub qi_cost: f32,
    pub cast_ticks: u32,
    pub cooldown_ticks: u32,
    pub range: f32,
    pub icon_texture: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TechniqueRequiredMeridian {
    pub channel: &'static str,
    pub min_health: f32,
}

pub const TECHNIQUE_DEFINITIONS: [TechniqueDefinition; 5] = [
    TechniqueDefinition {
        id: "burst_meridian.beng_quan",
        display_name: "崩拳",
        grade: "yellow",
        description: "主动撕裂右臂手三阳，零距灌入一记沉重短拳。",
        required_realm: "Induce",
        required_meridians: &[
            TechniqueRequiredMeridian {
                channel: "LargeIntestine",
                min_health: 0.01,
            },
            TechniqueRequiredMeridian {
                channel: "SmallIntestine",
                min_health: 0.01,
            },
            TechniqueRequiredMeridian {
                channel: "TripleEnergizer",
                min_health: 0.01,
            },
        ],
        qi_cost: 0.4,
        cast_ticks: 8,
        cooldown_ticks: 60,
        range: 1.3,
        icon_texture: "bong:textures/gui/skill/beng_quan.png",
    },
    TechniqueDefinition {
        id: "burst_meridian.tie_shan_kao",
        display_name: "贴山靠",
        grade: "yellow",
        description: "沉肩压步，以躯干经脉短爆撞开近身敌。",
        required_realm: "Condense",
        required_meridians: &[TechniqueRequiredMeridian {
            channel: "Stomach",
            min_health: 0.5,
        }],
        qi_cost: 35.0,
        cast_ticks: 10,
        cooldown_ticks: 70,
        range: 1.5,
        icon_texture: "bong:textures/gui/skill/tie_shan_kao.png",
    },
    TechniqueDefinition {
        id: "burst_meridian.xue_beng_bu",
        display_name: "血崩步",
        grade: "yellow",
        description: "以腿经裂响换取短距突进，适合抢入战圈。",
        required_realm: "Condense",
        required_meridians: &[TechniqueRequiredMeridian {
            channel: "GallBladder",
            min_health: 0.4,
        }],
        qi_cost: 25.0,
        cast_ticks: 6,
        cooldown_ticks: 50,
        range: 4.0,
        icon_texture: "bong:textures/gui/skill/xue_beng_bu.png",
    },
    TechniqueDefinition {
        id: "burst_meridian.ni_mai_hu_ti",
        display_name: "逆脉护体",
        grade: "profound",
        description: "逆转真元护住要害，短时压住外伤冲击。",
        required_realm: "Solidify",
        required_meridians: &[TechniqueRequiredMeridian {
            channel: "Pericardium",
            min_health: 0.55,
        }],
        qi_cost: 45.0,
        cast_ticks: 12,
        cooldown_ticks: 120,
        range: 0.0,
        icon_texture: "bong:textures/gui/skill/ni_mai_hu_ti.png",
    },
    TechniqueDefinition {
        id: "woliu.vortex",
        display_name: "绝灵涡流",
        grade: "profound",
        description: "掌心强造相对负灵域，持涡抽干飞入真元，久持则反噬手经。",
        required_realm: "Condense",
        required_meridians: &[TechniqueRequiredMeridian {
            channel: "Lung",
            min_health: 0.01,
        }],
        qi_cost: 0.0,
        cast_ticks: 1,
        cooldown_ticks: 20,
        range: 0.0,
        icon_texture: "bong:textures/gui/skill/woliu_vortex.png",
    },
];

pub fn technique_definition(id: &str) -> Option<&'static TechniqueDefinition> {
    TECHNIQUE_DEFINITIONS
        .iter()
        .find(|definition| definition.id == id)
}
