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

const TECHNIQUE_IDS: [&str; 33] = [
    "burst_meridian.beng_quan",
    "burst_meridian.tie_shan_kao",
    "burst_meridian.xue_beng_bu",
    "burst_meridian.ni_mai_hu_ti",
    "bao_mai.full_power_charge",
    "bao_mai.full_power_release",
    "zhenmai.parry",
    "zhenmai.neutralize",
    "zhenmai.multipoint",
    "zhenmai.harden",
    "zhenmai.sever_chain",
    "woliu.vortex",
    "woliu.hold",
    "woliu.burst",
    "woliu.mouth",
    "woliu.pull",
    "woliu.heart",
    "woliu.vacuum_palm",
    "woliu.vortex_shield",
    "woliu.vacuum_lock",
    "woliu.vortex_resonance",
    "woliu.turbulence_burst",
    "dugu.shoot_needle",
    "dugu.infuse_poison",
    "tuike.don",
    "tuike.shed",
    "tuike.transfer_taint",
    "anqi.charge_carrier",
    "anqi.single_snipe",
    "anqi.multi_shot",
    "anqi.soul_inject",
    "anqi.armor_pierce",
    "anqi.echo_fractal",
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

const WOLIU_V3_REQUIRED_MERIDIANS: [TechniqueRequiredMeridian; 2] = [
    TechniqueRequiredMeridian {
        channel: "Lung",
        min_health: 0.01,
    },
    TechniqueRequiredMeridian {
        channel: "Heart",
        min_health: 0.01,
    },
];

pub const TECHNIQUE_DEFINITIONS: [TechniqueDefinition; 33] = [
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
        id: "bao_mai.full_power_charge",
        display_name: "全力一击·蓄",
        grade: "profound",
        description: "把当前真元池逐 tick 灌入一击，蓄力期间被命中会损失部分真元。",
        required_realm: "Induce",
        required_meridians: &[],
        qi_cost: 100.0,
        cast_ticks: 1,
        cooldown_ticks: 0,
        range: 0.0,
        icon_texture: "bong:textures/gui/skill/full_power_charge.png",
    },
    TechniqueDefinition {
        id: "bao_mai.full_power_release",
        display_name: "全力一击·放",
        grade: "profound",
        description: "释放已蓄真元，按双方境界池子差距换算伤害，随后进入虚脱。",
        required_realm: "Induce",
        required_meridians: &[],
        qi_cost: 0.0,
        cast_ticks: 1,
        cooldown_ticks: 20,
        range: 8.0,
        icon_texture: "bong:textures/gui/skill/full_power_release.png",
    },
    TechniqueDefinition {
        id: "zhenmai.parry",
        display_name: "极限弹反",
        grade: "yellow",
        description: "受击前短时预备，皮下震爆异种真元，以血肉自伤换接触式反震。",
        required_realm: "Awaken",
        required_meridians: &[TechniqueRequiredMeridian {
            channel: "Lung",
            min_health: 0.01,
        }],
        qi_cost: 8.0,
        cast_ticks: 1,
        cooldown_ticks: 600,
        range: 0.0,
        icon_texture: "bong-client:textures/gui/skill/zhenmai_parry.png",
    },
    TechniqueDefinition {
        id: "zhenmai.neutralize",
        display_name: "局部中和",
        grade: "yellow",
        description: "点按一条经脉，以自身真元亏损清掉异种污染余响。",
        required_realm: "Awaken",
        required_meridians: &[TechniqueRequiredMeridian {
            channel: "Lung",
            min_health: 0.01,
        }],
        qi_cost: 18.0,
        cast_ticks: 4,
        cooldown_ticks: 200,
        range: 0.0,
        icon_texture: "bong-client:textures/gui/skill/zhenmai_neutralize.png",
    },
    TechniqueDefinition {
        id: "zhenmai.multipoint",
        display_name: "多点反震",
        grade: "profound",
        description: "展开多处皮下震爆点，群战接触时分散反震并承担小额自伤。",
        required_realm: "Awaken",
        required_meridians: &[],
        qi_cost: 12.0,
        cast_ticks: 6,
        cooldown_ticks: 600,
        range: 0.0,
        icon_texture: "bong-client:textures/gui/skill/zhenmai_multipoint.png",
    },
    TechniqueDefinition {
        id: "zhenmai.harden",
        display_name: "护脉",
        grade: "profound",
        description: "临时硬化选定经脉，降低经脉伤损但持续消耗真元。",
        required_realm: "Awaken",
        required_meridians: &[TechniqueRequiredMeridian {
            channel: "Lung",
            min_health: 0.01,
        }],
        qi_cost: 8.0,
        cast_ticks: 5,
        cooldown_ticks: 300,
        range: 0.0,
        icon_texture: "bong-client:textures/gui/skill/zhenmai_harden.png",
    },
    TechniqueDefinition {
        id: "zhenmai.sever_chain",
        display_name: "绝脉断链",
        grade: "yellow",
        description: "主动永久断一条经脉；通灵以上获得 60s 指定攻击类型反震放大。",
        required_realm: "Awaken",
        required_meridians: &[],
        qi_cost: 50.0,
        cast_ticks: 8,
        cooldown_ticks: 1200,
        range: 1.0,
        icon_texture: "bong-client:textures/gui/skill/zhenmai_sever_chain.png",
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
    TechniqueDefinition {
        id: "woliu.hold",
        display_name: "持涡",
        grade: "profound",
        description: "掌心维持涡流伞，抽干飞行真元载体并将九成九甩成紊流场。",
        required_realm: "Awaken",
        required_meridians: &[TechniqueRequiredMeridian {
            channel: "Lung",
            min_health: 0.01,
        }],
        qi_cost: 0.0,
        cast_ticks: 1,
        cooldown_ticks: 10,
        range: 0.0,
        icon_texture: "bong:textures/gui/skill/woliu_hold.png",
    },
    TechniqueDefinition {
        id: "woliu.burst",
        display_name: "瞬涡",
        grade: "yellow",
        description: "二百毫秒负压弹反窗口，反吸攻方真元并触发差异化涡刃反馈。",
        required_realm: "Awaken",
        required_meridians: &[TechniqueRequiredMeridian {
            channel: "Lung",
            min_health: 0.01,
        }],
        qi_cost: 8.0,
        cast_ticks: 1,
        cooldown_ticks: 100,
        range: 1.0,
        icon_texture: "bong:textures/gui/skill/woliu_burst.png",
    },
    TechniqueDefinition {
        id: "woliu.mouth",
        display_name: "涡口",
        grade: "profound",
        description: "在目标处开远程低压点，按 1/r^2 抽取并在目标所在处留下紊流禁区。",
        required_realm: "Awaken",
        required_meridians: &[TechniqueRequiredMeridian {
            channel: "Lung",
            min_health: 0.01,
        }],
        qi_cost: 12.0,
        cast_ticks: 6,
        cooldown_ticks: 160,
        range: 30.0,
        icon_texture: "bong:textures/gui/skill/woliu_mouth.png",
    },
    TechniqueDefinition {
        id: "woliu.pull",
        display_name: "涡引",
        grade: "profound",
        description: "只拉有真元目标，按 caster/target 真元压强比计算位移并拖出紊流尾迹。",
        required_realm: "Awaken",
        required_meridians: &[TechniqueRequiredMeridian {
            channel: "Lung",
            min_health: 0.01,
        }],
        qi_cost: 25.0,
        cast_ticks: 5,
        cooldown_ticks: 600,
        range: 30.0,
        icon_texture: "bong:textures/gui/skill/woliu_pull.png",
    },
    TechniqueDefinition {
        id: "woliu.heart",
        display_name: "涡心",
        grade: "earth",
        description: "半步化虚以上质变招，主动山谷级负压场；坍缩渊内强制断经反噬。",
        required_realm: "Condense",
        required_meridians: &[TechniqueRequiredMeridian {
            channel: "Lung",
            min_health: 0.01,
        }],
        qi_cost: 50.0,
        cast_ticks: 10,
        cooldown_ticks: 400,
        range: 100.0,
        icon_texture: "bong:textures/gui/skill/woliu_heart.png",
    },
    TechniqueDefinition {
        id: "woliu.vacuum_palm",
        display_name: "吸涡掌",
        grade: "yellow",
        description: "近距展掌开涡，把单个有真元目标拉近并抽取少量真元回掌。",
        required_realm: "Awaken",
        required_meridians: &WOLIU_V3_REQUIRED_MERIDIANS,
        qi_cost: 20.0,
        cast_ticks: 6,
        cooldown_ticks: 60,
        range: 8.0,
        icon_texture: "bong:textures/gui/skill/woliu_mouth.png",
    },
    TechniqueDefinition {
        id: "woliu.vortex_shield",
        display_name: "涡流护体",
        grade: "yellow",
        description: "身周撑开真空层，偏转来袭真元并制造淡紫紊流护罩。",
        required_realm: "Awaken",
        required_meridians: &WOLIU_V3_REQUIRED_MERIDIANS,
        qi_cost: 50.0,
        cast_ticks: 10,
        cooldown_ticks: 240,
        range: 2.0,
        icon_texture: "bong:textures/gui/skill/woliu_hold.png",
    },
    TechniqueDefinition {
        id: "woliu.vacuum_lock",
        display_name: "真空锁",
        grade: "profound",
        description: "在目标周身合拢真空笼，短时锁住行动并加速其真元逸散。",
        required_realm: "Awaken",
        required_meridians: &WOLIU_V3_REQUIRED_MERIDIANS,
        qi_cost: 35.0,
        cast_ticks: 8,
        cooldown_ticks: 300,
        range: 12.0,
        icon_texture: "bong:textures/gui/skill/woliu_pull.png",
    },
    TechniqueDefinition {
        id: "woliu.vortex_resonance",
        display_name: "涡流共振",
        grade: "profound",
        description: "以自身为心铺开群体涡旋，把多目标卷入同一低压场。",
        required_realm: "Awaken",
        required_meridians: &WOLIU_V3_REQUIRED_MERIDIANS,
        qi_cost: 50.0,
        cast_ticks: 80,
        cooldown_ticks: 400,
        range: 6.0,
        icon_texture: "bong:textures/gui/skill/woliu_heart.png",
    },
    TechniqueDefinition {
        id: "woliu.turbulence_burst",
        display_name: "紊流爆发",
        grade: "earth",
        description: "蓄出真空场后瞬间碎裂，向外释放物理冲击与高强紊流。",
        required_realm: "Awaken",
        required_meridians: &WOLIU_V3_REQUIRED_MERIDIANS,
        qi_cost: 80.0,
        cast_ticks: 40,
        cooldown_ticks: 600,
        range: 6.0,
        icon_texture: "bong:textures/gui/skill/woliu_burst.png",
    },
    TechniqueDefinition {
        id: "dugu.shoot_needle",
        display_name: "凝针",
        grade: "yellow",
        description: "以一点真元凝作细针，远距直刺，不带毒蛊即只是普通真元投射。",
        required_realm: "Induce",
        required_meridians: &[],
        qi_cost: 1.0,
        cast_ticks: 1,
        cooldown_ticks: 12,
        range: 50.0,
        icon_texture: "bong:textures/gui/skill/dugu_shoot_needle.png",
    },
    TechniqueDefinition {
        id: "dugu.infuse_poison",
        display_name: "灌毒蛊",
        grade: "profound",
        description: "将失谐真元覆入下一次飞针，命中后慢性蚀损对方经脉。",
        required_realm: "Induce",
        required_meridians: &[],
        qi_cost: 5.0,
        cast_ticks: 1,
        cooldown_ticks: 40,
        range: 0.0,
        icon_texture: "bong:textures/gui/skill/dugu_infuse_poison.png",
    },
    TechniqueDefinition {
        id: "tuike.don",
        display_name: "着壳",
        grade: "yellow",
        description: "把当前伪灵皮贴入气息外壳，形成可蜕落的钱包防线。",
        required_realm: "Awaken",
        required_meridians: &[],
        qi_cost: 0.0,
        cast_ticks: 12,
        cooldown_ticks: 20,
        range: 0.0,
        icon_texture: "bong-client:textures/gui/skill/tuike_don.png",
    },
    TechniqueDefinition {
        id: "tuike.shed",
        display_name: "蜕一层",
        grade: "profound",
        description: "弃掉最外层伪皮，带走承载的伤与污染，启动成本走当前真元池。",
        required_realm: "Awaken",
        required_meridians: &[],
        qi_cost: 0.0,
        cast_ticks: 8,
        cooldown_ticks: 160,
        range: 0.0,
        icon_texture: "bong-client:textures/gui/skill/tuike_shed.png",
    },
    TechniqueDefinition {
        id: "tuike.transfer_taint",
        display_name: "转移污染",
        grade: "profound",
        description: "把经脉里已入侵的异种真元推到伪皮，化虚上古皮可吸永久标记。",
        required_realm: "Awaken",
        required_meridians: &[],
        qi_cost: 0.0,
        cast_ticks: 10,
        cooldown_ticks: 100,
        range: 0.0,
        icon_texture: "bong-client:textures/gui/skill/tuike_transfer_taint.png",
    },
    TechniqueDefinition {
        id: "anqi.charge_carrier",
        display_name: "封骨",
        grade: "yellow",
        description: "静坐二十息，将真元封入手中异变兽骨，备作远射暗器。",
        required_realm: "Induce",
        required_meridians: &[TechniqueRequiredMeridian {
            channel: "Lung",
            min_health: 0.01,
        }],
        qi_cost: 0.0,
        cast_ticks: 400,
        cooldown_ticks: 400,
        range: 0.0,
        icon_texture: "bong:textures/gui/skill/anqi_charge_carrier.png",
    },
    TechniqueDefinition {
        id: "anqi.single_snipe",
        display_name: "单射狙击",
        grade: "yellow",
        description: "以单根载体封元远射，释放瞬间读取目向，命中后注入封存真元。",
        required_realm: "Awaken",
        required_meridians: &[
            TechniqueRequiredMeridian {
                channel: "Lung",
                min_health: 0.01,
            },
            TechniqueRequiredMeridian {
                channel: "Heart",
                min_health: 0.01,
            },
            TechniqueRequiredMeridian {
                channel: "Pericardium",
                min_health: 0.01,
            },
        ],
        qi_cost: 0.25,
        cast_ticks: 6,
        cooldown_ticks: 60,
        range: 80.0,
        icon_texture: "bong:textures/gui/skill/anqi_single_snipe.png",
    },
    TechniqueDefinition {
        id: "anqi.multi_shot",
        display_name: "多发齐射",
        grade: "yellow",
        description: "五支灵木载体扇形齐发，每条弹道独立结算。",
        required_realm: "Awaken",
        required_meridians: &[TechniqueRequiredMeridian {
            channel: "Pericardium",
            min_health: 0.01,
        }],
        qi_cost: 0.40,
        cast_ticks: 30,
        cooldown_ticks: 240,
        range: 30.0,
        icon_texture: "bong:textures/gui/skill/anqi_multi_shot.png",
    },
    TechniqueDefinition {
        id: "anqi.soul_inject",
        display_name: "凝魂注射",
        grade: "profound",
        description: "凝实色载体高密度封存，命中后按颜色匹配放大伤口与污染。",
        required_realm: "Condense",
        required_meridians: &[TechniqueRequiredMeridian {
            channel: "Spleen",
            min_health: 0.01,
        }],
        qi_cost: 0.35,
        cast_ticks: 20,
        cooldown_ticks: 360,
        range: 50.0,
        icon_texture: "bong:textures/gui/skill/anqi_soul_inject.png",
    },
    TechniqueDefinition {
        id: "anqi.armor_pierce",
        display_name: "破甲注射",
        grade: "profound",
        description: "封灵匣骨超功率封存，穿透目标防御后有载体碎裂风险。",
        required_realm: "Solidify",
        required_meridians: &[TechniqueRequiredMeridian {
            channel: "LargeIntestine",
            min_health: 0.01,
        }],
        qi_cost: 0.45,
        cast_ticks: 40,
        cooldown_ticks: 500,
        range: 80.0,
        icon_texture: "bong:textures/gui/skill/anqi_armor_pierce.png",
    },
    TechniqueDefinition {
        id: "anqi.echo_fractal",
        display_name: "诱饵分形",
        grade: "earth",
        description: "化虚真元浓度场将上古残骨分形为多条真实 echo 弹道。",
        required_realm: "Void",
        required_meridians: &[TechniqueRequiredMeridian {
            channel: "Du",
            min_health: 0.01,
        }],
        qi_cost: 0.60,
        cast_ticks: 60,
        cooldown_ticks: 6000,
        range: 150.0,
        icon_texture: "bong:textures/gui/skill/anqi_echo_fractal.png",
    },
];

pub fn technique_definition(id: &str) -> Option<&'static TechniqueDefinition> {
    TECHNIQUE_DEFINITIONS
        .iter()
        .find(|definition| definition.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn technique_ids_match_definitions_and_default_entries() {
        let ids = TECHNIQUE_IDS.iter().copied().collect::<BTreeSet<_>>();
        let definitions = TECHNIQUE_DEFINITIONS
            .iter()
            .map(|definition| definition.id)
            .collect::<BTreeSet<_>>();
        let default_techniques = KnownTechniques::default();
        let default_entries = default_techniques
            .entries
            .iter()
            .map(|entry| entry.id.as_str())
            .collect::<BTreeSet<_>>();

        assert_eq!(ids, definitions);
        assert_eq!(ids, default_entries);
        for id in ids {
            assert!(
                technique_definition(id).is_some(),
                "default technique id must have a definition: {id}"
            );
        }
    }

    #[test]
    fn woliu_v3_techniques_require_breath_and_heart_meridians() {
        for id in [
            "woliu.vacuum_palm",
            "woliu.vortex_shield",
            "woliu.vacuum_lock",
            "woliu.vortex_resonance",
            "woliu.turbulence_burst",
        ] {
            let definition = technique_definition(id).expect("woliu-v3 technique must exist");
            let channels = definition
                .required_meridians
                .iter()
                .map(|meridian| meridian.channel)
                .collect::<Vec<_>>();
            assert_eq!(channels, ["Lung", "Heart"]);
        }
    }
}
