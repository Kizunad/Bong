use serde::{Deserialize, Serialize};
use valence::prelude::{bevy_ecs, Component};

#[derive(Debug, Clone, Component, Serialize, Deserialize, PartialEq)]
#[cfg_attr(not(feature = "dev-techniques"), derive(Default))]
pub struct KnownTechniques {
    pub entries: Vec<KnownTechnique>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KnownTechnique {
    pub id: String,
    pub proficiency: f32,
    pub active: bool,
}

#[cfg(feature = "dev-techniques")]
impl Default for KnownTechniques {
    fn default() -> Self {
        Self::dev_default()
    }
}

impl KnownTechniques {
    pub fn dev_default() -> Self {
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

const TECHNIQUE_IDS: [&str; 48] = [
    "sword.cleave",
    "sword.thrust",
    "sword.parry",
    "sword.infuse",
    "movement.dash",
    "shield_block",
    "burst_meridian.beng_quan",
    "burst_meridian.tie_shan_kao",
    "burst_meridian.xue_beng_bu",
    "burst_meridian.ni_mai_hu_ti",
    "baomai.full_power_charge",
    "baomai.full_power_release",
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
    "body.guangbo_ticao",
    "sword_path.condense_edge",
    "sword_path.qi_slash",
    "sword_path.resonance",
    "sword_path.manifest",
    "sword_path.heaven_gate",
    "npc.heal_basic",
    "npc.buff_speed",
    "npc.buff_defense",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SkillCategory {
    Attack,
    Heal,
    Buff,
    Control,
    Defense,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TechniqueDefinition {
    pub id: &'static str,
    pub display_name: &'static str,
    pub grade: &'static str,
    pub description: &'static str,
    pub required_realm: &'static str,
    pub required_meridians: &'static [TechniqueRequiredMeridian],
    pub qi_cost: f32,
    pub stamina_cost: f32,
    pub cast_ticks: u32,
    pub cooldown_ticks: u32,
    pub range: f32,
    pub icon_texture: &'static str,
    pub category: SkillCategory,
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

pub const TECHNIQUE_DEFINITIONS: [TechniqueDefinition; 48] = [
    TechniqueDefinition {
        id: "sword.cleave",
        display_name: "劈",
        grade: "common",
        description: "基础劈砍。举剑过顶，顺势劈下。",
        required_realm: "Awaken",
        required_meridians: &[],
        qi_cost: 0.0,
        stamina_cost: 8.0,
        cast_ticks: 16,
        cooldown_ticks: 30,
        range: 3.0,
        icon_texture: "bong:textures/gui/skill/sword_cleave.png",
        category: SkillCategory::Attack,
    },
    TechniqueDefinition {
        id: "sword.thrust",
        display_name: "刺",
        grade: "common",
        description: "基础突刺。收肘蓄力，直线捅出。",
        required_realm: "Awaken",
        required_meridians: &[],
        qi_cost: 0.0,
        stamina_cost: 4.0,
        cast_ticks: 10,
        cooldown_ticks: 20,
        range: 3.5,
        icon_texture: "bong:textures/gui/skill/sword_thrust.png",
        category: SkillCategory::Attack,
    },
    TechniqueDefinition {
        id: "sword.parry",
        display_name: "格",
        grade: "common",
        description: "基础格挡。以剑身格开来袭，时机精准可反震对手。",
        required_realm: "Awaken",
        required_meridians: &[],
        qi_cost: 0.0,
        stamina_cost: 6.0,
        cast_ticks: 4,
        cooldown_ticks: 40,
        range: 0.0,
        icon_texture: "bong:textures/gui/skill/sword_parry.png",
        category: SkillCategory::Defense,
    },
    TechniqueDefinition {
        id: "sword.infuse",
        display_name: "注剑",
        grade: "common",
        description: "将真元注入剑身。持续期间命中附带真元污染。",
        required_realm: "Induce",
        required_meridians: &[],
        qi_cost: 0.0,
        stamina_cost: 3.0,
        cast_ticks: 40,
        cooldown_ticks: 100,
        range: 0.0,
        icon_texture: "bong:textures/gui/skill/sword_infuse.png",
        category: SkillCategory::Buff,
    },
    TechniqueDefinition {
        id: "movement.dash",
        display_name: "闪避",
        grade: "common",
        description: "短距闪身，熟练后体力消耗与冷却下降、位移距离增加。",
        required_realm: "Awaken",
        required_meridians: &[],
        qi_cost: 0.0,
        stamina_cost: 15.0,
        cast_ticks: 0,
        cooldown_ticks: 40,
        range: 2.8,
        icon_texture: "bong:textures/gui/skill/movement_dash.png",
        category: SkillCategory::Attack,
    },
    // plan-shield-block-v1 P4 — 盾牌格挡熟练度，无经脉前置，持盾即可习得。
    TechniqueDefinition {
        id: "shield_block",
        display_name: "盾挡",
        grade: "common",
        description: "以盾承受敌方攻击。熟练后格挡比例上升、体力消耗下降。",
        required_realm: "Awaken",
        required_meridians: &[],
        qi_cost: 0.0,
        stamina_cost: 0.0, // 耗体力由 ShieldDrainOverride / stamina_tick 管理，非 cast 消耗
        cast_ticks: 0,
        cooldown_ticks: 0,
        range: 0.0,
        icon_texture: "bong:textures/gui/skill/shield_block.png",
        category: SkillCategory::Defense,
    },
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
        stamina_cost: 0.0,
        cast_ticks: 8,
        cooldown_ticks: 60,
        range: 1.3,
        icon_texture: "bong:textures/gui/skill/beng_quan.png",
        category: SkillCategory::Attack,
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
        stamina_cost: 0.0,
        cast_ticks: 10,
        cooldown_ticks: 70,
        range: 1.5,
        icon_texture: "bong:textures/gui/skill/tie_shan_kao.png",
        category: SkillCategory::Attack,
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
        stamina_cost: 0.0,
        cast_ticks: 6,
        cooldown_ticks: 50,
        range: 4.0,
        icon_texture: "bong:textures/gui/skill/xue_beng_bu.png",
        category: SkillCategory::Attack,
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
        stamina_cost: 0.0,
        cast_ticks: 12,
        cooldown_ticks: 120,
        range: 0.0,
        icon_texture: "bong:textures/gui/skill/ni_mai_hu_ti.png",
        category: SkillCategory::Defense,
    },
    TechniqueDefinition {
        id: "baomai.full_power_charge",
        display_name: "全力一击·蓄",
        grade: "profound",
        description: "把当前真元池逐 tick 灌入一击，蓄力期间被命中会损失部分真元。",
        required_realm: "Induce",
        required_meridians: &[],
        qi_cost: 100.0,
        stamina_cost: 0.0,
        cast_ticks: 1,
        cooldown_ticks: 0,
        range: 0.0,
        icon_texture: "bong:textures/gui/skill/full_power_charge.png",
        category: SkillCategory::Attack,
    },
    TechniqueDefinition {
        id: "baomai.full_power_release",
        display_name: "全力一击·放",
        grade: "profound",
        description: "释放已蓄真元，按双方境界池子差距换算伤害，随后进入虚脱。",
        required_realm: "Induce",
        required_meridians: &[],
        qi_cost: 0.0,
        stamina_cost: 0.0,
        cast_ticks: 1,
        cooldown_ticks: 20,
        range: 8.0,
        icon_texture: "bong:textures/gui/skill/full_power_release.png",
        category: SkillCategory::Attack,
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
        stamina_cost: 0.0,
        cast_ticks: 1,
        cooldown_ticks: 600,
        range: 0.0,
        icon_texture: "bong-client:textures/gui/skill/zhenmai_parry.png",
        category: SkillCategory::Defense,
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
        stamina_cost: 0.0,
        cast_ticks: 4,
        cooldown_ticks: 200,
        range: 0.0,
        icon_texture: "bong-client:textures/gui/skill/zhenmai_neutralize.png",
        category: SkillCategory::Heal,
    },
    TechniqueDefinition {
        id: "zhenmai.multipoint",
        display_name: "多点反震",
        grade: "profound",
        description: "展开多处皮下震爆点，群战接触时分散反震并承担小额自伤。",
        required_realm: "Awaken",
        required_meridians: &[],
        qi_cost: 12.0,
        stamina_cost: 0.0,
        cast_ticks: 6,
        cooldown_ticks: 600,
        range: 0.0,
        icon_texture: "bong-client:textures/gui/skill/zhenmai_multipoint.png",
        category: SkillCategory::Defense,
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
        stamina_cost: 0.0,
        cast_ticks: 5,
        cooldown_ticks: 300,
        range: 0.0,
        icon_texture: "bong-client:textures/gui/skill/zhenmai_harden.png",
        category: SkillCategory::Defense,
    },
    TechniqueDefinition {
        id: "zhenmai.sever_chain",
        display_name: "绝脉断链",
        grade: "yellow",
        description: "主动永久断一条经脉；通灵以上获得 60s 指定攻击类型反震放大。",
        required_realm: "Awaken",
        required_meridians: &[],
        qi_cost: 50.0,
        stamina_cost: 0.0,
        cast_ticks: 8,
        cooldown_ticks: 1200,
        range: 1.0,
        icon_texture: "bong-client:textures/gui/skill/zhenmai_sever_chain.png",
        category: SkillCategory::Buff,
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
        stamina_cost: 0.0,
        cast_ticks: 1,
        cooldown_ticks: 20,
        range: 0.0,
        icon_texture: "bong:textures/gui/skill/woliu_vortex.png",
        category: SkillCategory::Buff,
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
        stamina_cost: 0.0,
        cast_ticks: 1,
        cooldown_ticks: 10,
        range: 0.0,
        icon_texture: "bong:textures/gui/skill/woliu_hold.png",
        category: SkillCategory::Buff,
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
        stamina_cost: 0.0,
        cast_ticks: 1,
        cooldown_ticks: 100,
        range: 1.0,
        icon_texture: "bong:textures/gui/skill/woliu_burst.png",
        category: SkillCategory::Attack,
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
        stamina_cost: 0.0,
        cast_ticks: 6,
        cooldown_ticks: 160,
        range: 30.0,
        icon_texture: "bong:textures/gui/skill/woliu_mouth.png",
        category: SkillCategory::Attack,
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
        stamina_cost: 0.0,
        cast_ticks: 5,
        cooldown_ticks: 600,
        range: 30.0,
        icon_texture: "bong:textures/gui/skill/woliu_pull.png",
        category: SkillCategory::Attack,
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
        stamina_cost: 0.0,
        cast_ticks: 10,
        cooldown_ticks: 400,
        range: 100.0,
        icon_texture: "bong:textures/gui/skill/woliu_heart.png",
        category: SkillCategory::Attack,
    },
    TechniqueDefinition {
        id: "woliu.vacuum_palm",
        display_name: "吸涡掌",
        grade: "yellow",
        description: "近距展掌开涡，把单个有真元目标拉近并抽取少量真元回掌。",
        required_realm: "Awaken",
        required_meridians: &WOLIU_V3_REQUIRED_MERIDIANS,
        qi_cost: 20.0,
        stamina_cost: 0.0,
        cast_ticks: 6,
        cooldown_ticks: 60,
        range: 8.0,
        icon_texture: "bong:textures/gui/skill/woliu_mouth.png",
        category: SkillCategory::Attack,
    },
    TechniqueDefinition {
        id: "woliu.vortex_shield",
        display_name: "涡流护体",
        grade: "yellow",
        description: "身周撑开真空层，偏转来袭真元并制造淡紫紊流护罩。",
        required_realm: "Awaken",
        required_meridians: &WOLIU_V3_REQUIRED_MERIDIANS,
        qi_cost: 50.0,
        stamina_cost: 0.0,
        cast_ticks: 10,
        cooldown_ticks: 240,
        range: 2.0,
        icon_texture: "bong:textures/gui/skill/woliu_hold.png",
        category: SkillCategory::Defense,
    },
    TechniqueDefinition {
        id: "woliu.vacuum_lock",
        display_name: "真空锁",
        grade: "profound",
        description: "在目标周身合拢真空笼，短时锁住行动并加速其真元逸散。",
        required_realm: "Awaken",
        required_meridians: &WOLIU_V3_REQUIRED_MERIDIANS,
        qi_cost: 35.0,
        stamina_cost: 0.0,
        cast_ticks: 8,
        cooldown_ticks: 300,
        range: 12.0,
        icon_texture: "bong:textures/gui/skill/woliu_pull.png",
        category: SkillCategory::Control,
    },
    TechniqueDefinition {
        id: "woliu.vortex_resonance",
        display_name: "涡流共振",
        grade: "profound",
        description: "以自身为心铺开群体涡旋，把多目标卷入同一低压场。",
        required_realm: "Awaken",
        required_meridians: &WOLIU_V3_REQUIRED_MERIDIANS,
        qi_cost: 50.0,
        stamina_cost: 0.0,
        cast_ticks: 80,
        cooldown_ticks: 400,
        range: 6.0,
        icon_texture: "bong:textures/gui/skill/woliu_heart.png",
        category: SkillCategory::Control,
    },
    TechniqueDefinition {
        id: "woliu.turbulence_burst",
        display_name: "紊流爆发",
        grade: "earth",
        description: "蓄出真空场后瞬间碎裂，向外释放物理冲击与高强紊流。",
        required_realm: "Awaken",
        required_meridians: &WOLIU_V3_REQUIRED_MERIDIANS,
        qi_cost: 80.0,
        stamina_cost: 0.0,
        cast_ticks: 40,
        cooldown_ticks: 600,
        range: 6.0,
        icon_texture: "bong:textures/gui/skill/woliu_burst.png",
        category: SkillCategory::Attack,
    },
    TechniqueDefinition {
        id: "dugu.shoot_needle",
        display_name: "凝针",
        grade: "yellow",
        description: "以一点真元凝作细针，远距直刺，不带毒蛊即只是普通真元投射。",
        required_realm: "Induce",
        required_meridians: &[],
        qi_cost: 1.0,
        stamina_cost: 0.0,
        cast_ticks: 1,
        cooldown_ticks: 12,
        range: 50.0,
        icon_texture: "bong:textures/gui/skill/dugu_shoot_needle.png",
        category: SkillCategory::Attack,
    },
    TechniqueDefinition {
        id: "dugu.infuse_poison",
        display_name: "灌毒蛊",
        grade: "profound",
        description: "将失谐真元覆入下一次飞针，命中后慢性蚀损对方经脉。",
        required_realm: "Induce",
        required_meridians: &[],
        qi_cost: 5.0,
        stamina_cost: 0.0,
        cast_ticks: 1,
        cooldown_ticks: 40,
        range: 0.0,
        icon_texture: "bong:textures/gui/skill/dugu_infuse_poison.png",
        category: SkillCategory::Buff,
    },
    TechniqueDefinition {
        id: "tuike.don",
        display_name: "着壳",
        grade: "yellow",
        description: "把当前伪灵皮贴入气息外壳，形成可蜕落的钱包防线。",
        required_realm: "Awaken",
        required_meridians: &[],
        qi_cost: 0.0,
        stamina_cost: 0.0,
        cast_ticks: 12,
        cooldown_ticks: 20,
        range: 0.0,
        icon_texture: "bong-client:textures/gui/skill/tuike_don.png",
        category: SkillCategory::Buff,
    },
    TechniqueDefinition {
        id: "tuike.shed",
        display_name: "蜕一层",
        grade: "profound",
        description: "弃掉最外层伪皮，带走承载的伤与污染，启动成本走当前真元池。",
        required_realm: "Awaken",
        required_meridians: &[],
        qi_cost: 0.0,
        stamina_cost: 0.0,
        cast_ticks: 8,
        cooldown_ticks: 160,
        range: 0.0,
        icon_texture: "bong-client:textures/gui/skill/tuike_shed.png",
        category: SkillCategory::Buff,
    },
    TechniqueDefinition {
        id: "tuike.transfer_taint",
        display_name: "转移污染",
        grade: "profound",
        description: "把经脉里已入侵的异种真元推到伪皮，化虚上古皮可吸永久标记。",
        required_realm: "Awaken",
        required_meridians: &[],
        qi_cost: 0.0,
        stamina_cost: 0.0,
        cast_ticks: 10,
        cooldown_ticks: 100,
        range: 0.0,
        icon_texture: "bong-client:textures/gui/skill/tuike_transfer_taint.png",
        category: SkillCategory::Buff,
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
        stamina_cost: 0.0,
        cast_ticks: 400,
        cooldown_ticks: 400,
        range: 0.0,
        icon_texture: "bong:textures/gui/skill/anqi_charge_carrier.png",
        category: SkillCategory::Buff,
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
        stamina_cost: 0.0,
        cast_ticks: 6,
        cooldown_ticks: 60,
        range: 80.0,
        icon_texture: "bong:textures/gui/skill/anqi_single_snipe.png",
        category: SkillCategory::Attack,
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
        stamina_cost: 0.0,
        cast_ticks: 30,
        cooldown_ticks: 240,
        range: 30.0,
        icon_texture: "bong:textures/gui/skill/anqi_multi_shot.png",
        category: SkillCategory::Attack,
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
        stamina_cost: 0.0,
        cast_ticks: 20,
        cooldown_ticks: 360,
        range: 50.0,
        icon_texture: "bong:textures/gui/skill/anqi_soul_inject.png",
        category: SkillCategory::Attack,
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
        stamina_cost: 0.0,
        cast_ticks: 40,
        cooldown_ticks: 500,
        range: 80.0,
        icon_texture: "bong:textures/gui/skill/anqi_armor_pierce.png",
        category: SkillCategory::Attack,
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
        stamina_cost: 0.0,
        cast_ticks: 60,
        cooldown_ticks: 6000,
        range: 150.0,
        icon_texture: "bong:textures/gui/skill/anqi_echo_fractal.png",
        category: SkillCategory::Attack,
    },
    TechniqueDefinition {
        id: "body.guangbo_ticao",
        display_name: "广播体操",
        grade: "common",
        description: "一套古怪的伸展动作。勤练者四肢筋骨渐强，步履亦轻。",
        required_realm: "Awaken",
        required_meridians: &[],
        qi_cost: 1.0,
        stamina_cost: 5.0,
        cast_ticks: 60,
        cooldown_ticks: 200,
        range: 0.0,
        icon_texture: "bong:textures/gui/skill/body_guangbo_ticao.png",
        category: SkillCategory::Buff,
    },
    // plan-sword-path-v2 P0：剑道五招纳入官方技能注册表，使残卷可被
    // `read_combat_technique_scroll` / `learn_technique_if_allowed` 识别。
    TechniqueDefinition {
        id: "sword_path.condense_edge",
        display_name: "剑意·凝锋",
        grade: "yellow",
        description: "凝聚剑势。下一次命中附带凝实剑意，威力提升并破甲。",
        required_realm: "Induce",
        required_meridians: &SWORD_PATH_BASE_MERIDIANS,
        qi_cost: 0.0,
        stamina_cost: 8.0,
        cast_ticks: 12,
        cooldown_ticks: 40,
        range: 4.0,
        icon_texture: "bong:textures/gui/skill/sword_condense_edge.png",
        category: SkillCategory::Attack,
    },
    TechniqueDefinition {
        id: "sword_path.qi_slash",
        display_name: "剑气·斩",
        grade: "yellow",
        description: "凝脉之上挥剑出气，直线远袭。距离越远剑气越薄。",
        required_realm: "Condense",
        required_meridians: &SWORD_PATH_QI_SLASH_MERIDIANS,
        qi_cost: 3.0,
        stamina_cost: 12.0,
        cast_ticks: 20,
        cooldown_ticks: 60,
        range: 8.0,
        icon_texture: "bong:textures/gui/skill/sword_qi_slash.png",
        category: SkillCategory::Attack,
    },
    TechniqueDefinition {
        id: "sword_path.resonance",
        display_name: "共鸣·剑鸣",
        grade: "profound",
        description: "固元剑修以剑鸣震慑四方，打断敌方法术并使其僵滞。",
        required_realm: "Solidify",
        required_meridians: &SWORD_PATH_QI_SLASH_MERIDIANS,
        qi_cost: 20.0,
        stamina_cost: 15.0,
        cast_ticks: 30,
        cooldown_ticks: 120,
        range: 6.0,
        icon_texture: "bong:textures/gui/skill/sword_resonance.png",
        category: SkillCategory::Control,
    },
    TechniqueDefinition {
        id: "sword_path.manifest",
        display_name: "归一·剑意化形",
        grade: "profound",
        description: "通灵剑修将剑意凝为实体，自动追击最近敌方。结束后人剑共鸣略损。",
        required_realm: "Spirit",
        required_meridians: &SWORD_PATH_QI_SLASH_MERIDIANS,
        qi_cost: 40.0,
        stamina_cost: 20.0,
        cast_ticks: 40,
        cooldown_ticks: 200,
        range: 5.0,
        icon_texture: "bong:textures/gui/skill/sword_manifest.png",
        category: SkillCategory::Attack,
    },
    TechniqueDefinition {
        id: "sword_path.heaven_gate",
        display_name: "天门·一剑开天",
        grade: "earth",
        description: "化虚禁招。倾尽真元一击劈空，事后跌境碎剑、藏于天道盲区五分钟。",
        required_realm: "Void",
        required_meridians: &SWORD_PATH_HEAVEN_GATE_MERIDIANS,
        qi_cost: 0.0,
        stamina_cost: 0.0,
        cast_ticks: 80,
        cooldown_ticks: u32::MAX,
        range: 100.0,
        icon_texture: "bong:textures/gui/skill/sword_heaven_gate.png",
        category: SkillCategory::Attack,
    },
    TechniqueDefinition {
        id: "npc.heal_basic",
        display_name: "基础回血",
        grade: "common",
        description: "NPC 真元调理伤势，降低伤口严重度并恢复生命值。",
        required_realm: "Induce",
        required_meridians: &NPC_HEAL_REQUIRED_MERIDIANS,
        qi_cost: 8.0,
        stamina_cost: 0.0,
        cast_ticks: 20,
        cooldown_ticks: 200,
        range: 0.0,
        icon_texture: "bong:textures/gui/skill/npc_heal_basic.png",
        category: SkillCategory::Heal,
    },
    TechniqueDefinition {
        id: "npc.buff_speed",
        display_name: "疾行术",
        grade: "common",
        description: "NPC 以真元加速经脉运转，短时提升移动速度。",
        required_realm: "Condense",
        required_meridians: &NPC_BUFF_SPEED_REQUIRED_MERIDIANS,
        qi_cost: 5.0,
        stamina_cost: 0.0,
        cast_ticks: 10,
        cooldown_ticks: 400,
        range: 0.0,
        icon_texture: "bong:textures/gui/skill/npc_buff_speed.png",
        category: SkillCategory::Buff,
    },
    TechniqueDefinition {
        id: "npc.buff_defense",
        display_name: "护体术",
        grade: "common",
        description: "NPC 以真元凝护周身，短时减免受到的伤害。",
        required_realm: "Condense",
        required_meridians: &NPC_BUFF_DEFENSE_REQUIRED_MERIDIANS,
        qi_cost: 6.0,
        stamina_cost: 0.0,
        cast_ticks: 10,
        cooldown_ticks: 400,
        range: 0.0,
        icon_texture: "bong:textures/gui/skill/npc_buff_defense.png",
        category: SkillCategory::Buff,
    },
];

const NPC_HEAL_REQUIRED_MERIDIANS: [TechniqueRequiredMeridian; 2] = [
    TechniqueRequiredMeridian {
        channel: "Spleen",
        min_health: 0.01,
    },
    TechniqueRequiredMeridian {
        channel: "Kidney",
        min_health: 0.01,
    },
];

const NPC_BUFF_SPEED_REQUIRED_MERIDIANS: [TechniqueRequiredMeridian; 2] = [
    TechniqueRequiredMeridian {
        channel: "Stomach",
        min_health: 0.01,
    },
    TechniqueRequiredMeridian {
        channel: "Bladder",
        min_health: 0.01,
    },
];

const NPC_BUFF_DEFENSE_REQUIRED_MERIDIANS: [TechniqueRequiredMeridian; 2] = [
    TechniqueRequiredMeridian {
        channel: "Lung",
        min_health: 0.01,
    },
    TechniqueRequiredMeridian {
        channel: "Heart",
        min_health: 0.01,
    },
];

const SWORD_PATH_BASE_MERIDIANS: [TechniqueRequiredMeridian; 2] = [
    TechniqueRequiredMeridian {
        channel: "LargeIntestine",
        min_health: 0.01,
    },
    TechniqueRequiredMeridian {
        channel: "SmallIntestine",
        min_health: 0.01,
    },
];

const SWORD_PATH_QI_SLASH_MERIDIANS: [TechniqueRequiredMeridian; 3] = [
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
];

const SWORD_PATH_HEAVEN_GATE_MERIDIANS: [TechniqueRequiredMeridian; 4] = [
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
    TechniqueRequiredMeridian {
        channel: "Du",
        min_health: 0.01,
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
    fn technique_ids_match_definitions_and_dev_entries() {
        let ids = TECHNIQUE_IDS.iter().copied().collect::<BTreeSet<_>>();
        let definitions = TECHNIQUE_DEFINITIONS
            .iter()
            .map(|definition| definition.id)
            .collect::<BTreeSet<_>>();
        let dev_techniques = KnownTechniques::dev_default();
        let dev_entries = dev_techniques
            .entries
            .iter()
            .map(|entry| entry.id.as_str())
            .collect::<BTreeSet<_>>();

        assert_eq!(ids, definitions);
        assert_eq!(ids, dev_entries);
        for id in ids {
            assert!(
                technique_definition(id).is_some(),
                "dev technique id must have a definition: {id}"
            );
        }
    }

    #[test]
    #[cfg(not(feature = "dev-techniques"))]
    fn default_is_empty_without_dev_feature() {
        assert!(KnownTechniques::default().entries.is_empty());
    }

    #[test]
    #[cfg(feature = "dev-techniques")]
    fn default_uses_dev_entries_with_dev_feature() {
        assert_eq!(
            KnownTechniques::default().entries,
            KnownTechniques::dev_default().entries
        );
    }

    #[test]
    fn dev_default_has_all_47() {
        // plan-shield-block-v1 P4: shield_block 加入后总数升至 48
        let dev = KnownTechniques::dev_default();
        assert_eq!(dev.entries.len(), 48);
        assert!(dev
            .entries
            .iter()
            .all(|entry| entry.active && (entry.proficiency - 0.5).abs() <= f32::EPSILON));
    }

    #[test]
    fn sword_basics_have_no_meridian_gate_and_use_stamina() {
        for (id, stamina_cost) in [
            ("sword.cleave", 8.0),
            ("sword.thrust", 4.0),
            ("sword.parry", 6.0),
            ("sword.infuse", 3.0),
        ] {
            let definition = technique_definition(id).expect("sword technique definition");
            assert!(definition.required_meridians.is_empty());
            assert_eq!(definition.qi_cost, 0.0);
            assert_eq!(definition.stamina_cost, stamina_cost);
        }
    }

    #[test]
    fn sword_path_techniques_registered_with_ascending_realm_gates() {
        // plan-sword-path-v2 P0：五招按境界递增注册 + 残卷依赖经脉对齐
        // worldview §三/§四 + plan §P1.5。
        let expected: &[(&str, &str, &[&str])] = &[
            (
                "sword_path.condense_edge",
                "Induce",
                &["LargeIntestine", "SmallIntestine"],
            ),
            (
                "sword_path.qi_slash",
                "Condense",
                &["LargeIntestine", "SmallIntestine", "TripleEnergizer"],
            ),
            (
                "sword_path.resonance",
                "Solidify",
                &["LargeIntestine", "SmallIntestine", "TripleEnergizer"],
            ),
            (
                "sword_path.manifest",
                "Spirit",
                &["LargeIntestine", "SmallIntestine", "TripleEnergizer"],
            ),
            (
                "sword_path.heaven_gate",
                "Void",
                &["LargeIntestine", "SmallIntestine", "TripleEnergizer", "Du"],
            ),
        ];
        for (id, realm, channels) in expected {
            let def = technique_definition(id).expect("sword_path technique must exist");
            assert_eq!(def.required_realm, *realm, "realm gate for {id}");
            let actual_channels: Vec<&str> =
                def.required_meridians.iter().map(|m| m.channel).collect();
            assert_eq!(
                actual_channels, *channels,
                "meridian deps for {id} should match plan §P1.5"
            );
        }
    }

    #[test]
    fn sword_path_heaven_gate_marks_one_shot_cooldown() {
        // plan-sword-path-v2 §techniques::HEAVEN_GATE：化虚禁招一次性，CD=u32::MAX。
        let def =
            technique_definition("sword_path.heaven_gate").expect("heaven_gate must be registered");
        assert_eq!(
            def.cooldown_ticks,
            u32::MAX,
            "化虚一剑开天应为一次性招式（CD = u32::MAX 哨兵值）"
        );
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

    #[test]
    fn skill_category_pin_attack() {
        for id in [
            "sword.cleave",
            "sword.thrust",
            "movement.dash",
            "burst_meridian.beng_quan",
            "burst_meridian.tie_shan_kao",
            "burst_meridian.xue_beng_bu",
            "baomai.full_power_charge",
            "baomai.full_power_release",
            "woliu.burst",
            "woliu.mouth",
            "woliu.pull",
            "woliu.heart",
            "woliu.vacuum_palm",
            "woliu.turbulence_burst",
            "dugu.shoot_needle",
            "anqi.single_snipe",
            "anqi.multi_shot",
            "anqi.soul_inject",
            "anqi.armor_pierce",
            "anqi.echo_fractal",
            "sword_path.condense_edge",
            "sword_path.qi_slash",
            "sword_path.manifest",
            "sword_path.heaven_gate",
        ] {
            let def = technique_definition(id).unwrap();
            assert_eq!(def.category, SkillCategory::Attack, "{id} should be Attack");
        }
    }

    #[test]
    fn skill_category_pin_defense() {
        for id in [
            "sword.parry",
            "zhenmai.parry",
            "zhenmai.multipoint",
            "zhenmai.harden",
            "burst_meridian.ni_mai_hu_ti",
            "woliu.vortex_shield",
        ] {
            let def = technique_definition(id).unwrap();
            assert_eq!(
                def.category,
                SkillCategory::Defense,
                "{id} should be Defense"
            );
        }
    }

    #[test]
    fn skill_category_pin_buff() {
        for id in [
            "sword.infuse",
            "woliu.vortex",
            "woliu.hold",
            "tuike.don",
            "tuike.shed",
            "tuike.transfer_taint",
            "body.guangbo_ticao",
            "anqi.charge_carrier",
            "dugu.infuse_poison",
            "zhenmai.sever_chain",
            "npc.buff_speed",
            "npc.buff_defense",
        ] {
            let def = technique_definition(id).unwrap();
            assert_eq!(def.category, SkillCategory::Buff, "{id} should be Buff");
        }
    }

    #[test]
    fn skill_category_pin_control() {
        for id in [
            "woliu.vacuum_lock",
            "woliu.vortex_resonance",
            "sword_path.resonance",
        ] {
            let def = technique_definition(id).unwrap();
            assert_eq!(
                def.category,
                SkillCategory::Control,
                "{id} should be Control"
            );
        }
    }

    #[test]
    fn skill_category_pin_heal() {
        for id in ["zhenmai.neutralize", "npc.heal_basic"] {
            let def = technique_definition(id).unwrap();
            assert_eq!(def.category, SkillCategory::Heal, "{id} should be Heal");
        }
    }

    #[test]
    fn skill_category_all_variants_covered() {
        use std::collections::HashSet;
        let categories: HashSet<SkillCategory> = TECHNIQUE_DEFINITIONS
            .iter()
            .map(|def| def.category)
            .collect();
        assert!(
            categories.contains(&SkillCategory::Attack),
            "Attack must have at least one technique"
        );
        assert!(
            categories.contains(&SkillCategory::Defense),
            "Defense must have at least one technique"
        );
        assert!(
            categories.contains(&SkillCategory::Buff),
            "Buff must have at least one technique"
        );
        assert!(
            categories.contains(&SkillCategory::Control),
            "Control must have at least one technique"
        );
        assert!(
            categories.contains(&SkillCategory::Heal),
            "Heal must have at least one technique"
        );
    }

    #[test]
    fn every_technique_has_category() {
        for def in &TECHNIQUE_DEFINITIONS {
            let _ = def.category;
        }
    }

    #[test]
    fn npc_heal_basic_definition_pin() {
        let def = technique_definition("npc.heal_basic").expect("npc.heal_basic must exist");
        assert_eq!(def.category, SkillCategory::Heal);
        assert_eq!(def.required_realm, "Induce");
        assert_eq!(def.qi_cost, 8.0);
        assert_eq!(def.cooldown_ticks, 200);
        assert_eq!(def.cast_ticks, 20);
        assert_eq!(def.range, 0.0);
        let channels: Vec<&str> = def.required_meridians.iter().map(|m| m.channel).collect();
        assert_eq!(channels, ["Spleen", "Kidney"]);
    }

    #[test]
    fn npc_buff_speed_definition_pin() {
        let def = technique_definition("npc.buff_speed").expect("npc.buff_speed must exist");
        assert_eq!(def.category, SkillCategory::Buff);
        assert_eq!(def.required_realm, "Condense");
        assert_eq!(def.qi_cost, 5.0);
        assert_eq!(def.cooldown_ticks, 400);
        assert_eq!(def.cast_ticks, 10);
        let channels: Vec<&str> = def.required_meridians.iter().map(|m| m.channel).collect();
        assert_eq!(channels, ["Stomach", "Bladder"]);
    }

    #[test]
    fn npc_buff_defense_definition_pin() {
        let def = technique_definition("npc.buff_defense").expect("npc.buff_defense must exist");
        assert_eq!(def.category, SkillCategory::Buff);
        assert_eq!(def.required_realm, "Condense");
        assert_eq!(def.qi_cost, 6.0);
        assert_eq!(def.cooldown_ticks, 400);
        assert_eq!(def.cast_ticks, 10);
        let channels: Vec<&str> = def.required_meridians.iter().map(|m| m.channel).collect();
        assert_eq!(channels, ["Lung", "Heart"]);
    }

    // --- bao_mai ↔ baomai ID 一致性 pin (regression: r2-P3 bughunt fix) ---

    #[test]
    fn full_power_charge_id_is_canonical_baomai_no_underscore() {
        // 期望 "baomai.full_power_charge"（无 bao_mai 下划线形式），
        // 因为 baomai_v3/events.rs 中 BAOMAI_FULL_POWER_CHARGE_SKILL_ID 和
        // SkillMeridianDependencies 均以此为键；若 known_techniques 用
        // "bao_mai.*" 则 technique lookup 与 skill dispatch 脱节。
        let def = technique_definition("baomai.full_power_charge").expect(
            "expected 'baomai.full_power_charge' — got None, likely bao_mai/baomai mismatch",
        );
        assert_eq!(
            def.id, "baomai.full_power_charge",
            "technique id must be 'baomai.full_power_charge', not 'bao_mai.full_power_charge'"
        );
        assert_eq!(def.category, SkillCategory::Attack);
    }

    #[test]
    fn full_power_release_id_is_canonical_baomai_no_underscore() {
        // 同上，对 release 做同样的 pin。
        let def = technique_definition("baomai.full_power_release").expect(
            "expected 'baomai.full_power_release' — got None, likely bao_mai/baomai mismatch",
        );
        assert_eq!(
            def.id, "baomai.full_power_release",
            "technique id must be 'baomai.full_power_release', not 'bao_mai.full_power_release'"
        );
        assert_eq!(def.category, SkillCategory::Attack);
    }

    #[test]
    fn known_techniques_list_contains_canonical_baomai_ids() {
        // TECHNIQUE_IDS 列表必须用 "baomai.*" 形式，确保玩家 granted technique
        // 与 skill dispatch / meridian dep 门控键对齐。
        assert!(
            TECHNIQUE_IDS.contains(&"baomai.full_power_charge"),
            "TECHNIQUE_IDS should contain 'baomai.full_power_charge', not 'bao_mai.full_power_charge'"
        );
        assert!(
            TECHNIQUE_IDS.contains(&"baomai.full_power_release"),
            "TECHNIQUE_IDS should contain 'baomai.full_power_release', not 'bao_mai.full_power_release'"
        );
        assert!(
            !TECHNIQUE_IDS.contains(&"bao_mai.full_power_charge"),
            "stale 'bao_mai.full_power_charge' must not appear in TECHNIQUE_IDS"
        );
        assert!(
            !TECHNIQUE_IDS.contains(&"bao_mai.full_power_release"),
            "stale 'bao_mai.full_power_release' must not appear in TECHNIQUE_IDS"
        );
    }
}
