//! 修炼侧生平卷（plan §1.1, §11-4）— 全量保留修炼事件，无 sliding window。
//!
//! 死亡终结 / 亡者博物馆归档由战斗 plan 扩展同一 character_id，本 plan 不感知。

use serde::{Deserialize, Serialize};
use valence::prelude::{bevy_ecs, Component};

use super::components::{ColorKind, MeridianId, Realm};
use crate::skill::components::SkillId;

const UNASSIGNED_CHARACTER_ID: &str = "unassigned:life_record";
const TRIBULATION_INTERCEPT_TAG: &str = "戮道者 · 截劫";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HeartDemonOutcome {
    Steadfast,
    Obsession,
    NoSolution,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BiographyEntry {
    BreakthroughStarted {
        realm_target: Realm,
        tick: u64,
    },
    BreakthroughSucceeded {
        realm: Realm,
        tick: u64,
    },
    SpiritEyeBreakthrough {
        eye_id: String,
        #[serde(default)]
        zone: Option<String>,
        tick: u64,
    },
    BreakthroughFailed {
        realm_target: Realm,
        severity: f64,
        tick: u64,
    },
    MeridianOpened {
        id: MeridianId,
        tick: u64,
    },
    MeridianClosed {
        id: MeridianId,
        tick: u64,
        reason: String,
    },
    ForgedRate {
        id: MeridianId,
        tier: u8,
        tick: u64,
    },
    ForgedCapacity {
        id: MeridianId,
        tier: u8,
        tick: u64,
    },
    ColorShift {
        main: ColorKind,
        secondary: Option<ColorKind>,
        tick: u64,
    },
    InsightTaken {
        trigger: String,
        choice: String,
        tick: u64,
    },
    Rebirth {
        prior_realm: Realm,
        new_realm: Realm,
        tick: u64,
    },
    CombatHit {
        attacker_id: String,
        body_part: String,
        #[serde(default = "default_combat_hit_wound_kind")]
        wound_kind: String,
        damage: f32,
        tick: u64,
    },
    /// plan-zhenmai-v1 P2：截脉震爆成功弹反，记录防御者对攻击者的战绩。
    JiemaiParry {
        attacker_id: String,
        effectiveness: f32,
        tick: u64,
    },
    NearDeath {
        cause: String,
        tick: u64,
    },
    Terminated {
        cause: String,
        tick: u64,
    },
    LifespanExtended {
        source: String,
        delta_years: i64,
        tick: u64,
    },
    DuoShePerformed {
        target_id: String,
        host_prev_age: f64,
        target_age: f64,
        tick: u64,
    },
    PossessedBy {
        host_id: String,
        tick: u64,
    },
    /// plan-alchemy-v1 §1.3 — 每次炼丹结算写一条（精确或残缺路径）。
    AlchemyAttempt {
        recipe_id: String,
        #[serde(default)]
        pill: Option<String>,
        #[serde(default)]
        flawed_path: bool,
        #[serde(default)]
        side_effect_tag: Option<String>,
        tick: u64,
    },
    /// plan-lingtian-v1 §1.7 — 自家田被他人收（owner 视角）。匿名：只记位置。
    PlotHarvestedByOther {
        plot_pos: [i32; 3],
        plant_id: String,
        tick: u64,
    },
    /// plan-lingtian-v1 §1.7 — 自己收了别人家的田（operator 视角）。
    PlotHarvestedFromOther {
        plot_pos: [i32; 3],
        plant_id: String,
        tick: u64,
    },
    /// plan-lingtian-v1 §1.7 — 自家 plot_qi 被他人吸（owner 视角）。
    PlotQiDrainedByOther {
        plot_pos: [i32; 3],
        amount_drained: f32,
        tick: u64,
    },
    /// plan-lingtian-v1 §1.7 — 自己吸了别人家的 plot_qi（operator 视角）。
    PlotQiDrainedFromOther {
        plot_pos: [i32; 3],
        amount_drained: f32,
        tick: u64,
    },
    /// plan-lingtian-v1 §1.7 — 自家田被铲（owner 视角）。
    PlotDestroyedByOther {
        plot_pos: [i32; 3],
        tick: u64,
    },
    /// plan-tribulation-v1 §2.6 — 截胡杀死渡虚劫者，获得“戮道者 · 截劫”战绩。
    TribulationIntercepted {
        victim_id: String,
        #[serde(default = "default_tribulation_intercept_tag")]
        tag: String,
        tick: u64,
    },
    /// plan-tribulation-v1 §2.6 — 下线/逃离劫场，按首波失败处理并公开记档。
    TribulationFled {
        wave: u32,
        tick: u64,
    },
    /// plan-tribulation-v1 §2.4 — 心魔劫抉择公开记档。
    HeartDemonRecord {
        outcome: HeartDemonOutcome,
        choice_idx: Option<u32>,
        tick: u64,
    },
    /// plan-social-v1 §6.2 — 交易写入双方生平卷。只记物品摘要与匿名对手。
    TradeCompleted {
        counterparty_id: String,
        offered_item: String,
        received_item: String,
        tick: u64,
    },
    /// plan-woliu-v1 §3.1.D / P2：涡流抽干飞入真元投射物。
    VortexProjectileDrained {
        projectile_id: String,
        drained_amount: f32,
        tick: u64,
    },
    /// plan-woliu-v1 §3.1.E / P2：涡流维持或环境失败导致反噬。
    VortexBackfired {
        cause: String,
        tick: u64,
    },
    /// plan-anqi-v1 P2：暗器载体命中后的生平锚点。
    AnqiSniped {
        attacker_id: String,
        distance_blocks: f32,
        sealed_qi: f32,
        hit_qi: f32,
        tick: u64,
    },
    /// plan-spawn-tutorial-v1 P2 — 出生沉默教学完成：醒灵突破至引气。
    SpawnTutorialCompleted {
        minutes_since_spawn: u32,
        tick: u64,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TakenInsight {
    pub trigger_id: String,
    pub choice: String,
    pub magnitude: f64,
    pub flavor: String,
    pub taken_at: u64,
    pub realm_at_time: Realm,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeathInsightRecord {
    pub tick: u64,
    pub text: String,
    pub style: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillMilestone {
    pub skill: SkillId,
    pub new_lv: u8,
    pub achieved_at: u64,
    pub narration: String,
    pub total_xp_at: u64,
}

#[derive(Debug, Clone, Component, Serialize, Deserialize)]
pub struct LifeRecord {
    #[serde(default = "default_character_id")]
    pub character_id: String,
    pub created_at: u64,
    pub biography: Vec<BiographyEntry>,
    pub insights_taken: Vec<TakenInsight>,
    #[serde(default)]
    pub death_insights: Vec<DeathInsightRecord>,
    #[serde(default)]
    pub skill_milestones: Vec<SkillMilestone>,
    pub spirit_root_first: Option<MeridianId>,
}

impl Default for LifeRecord {
    fn default() -> Self {
        Self::new_unassigned()
    }
}

impl LifeRecord {
    pub fn new(character_id: impl Into<String>) -> Self {
        Self {
            character_id: character_id.into(),
            created_at: 0,
            biography: Vec::new(),
            insights_taken: Vec::new(),
            death_insights: Vec::new(),
            skill_milestones: Vec::new(),
            spirit_root_first: None,
        }
    }

    pub fn new_unassigned() -> Self {
        Self::new(default_character_id())
    }

    pub fn push(&mut self, entry: BiographyEntry) {
        self.biography.push(entry);
    }

    pub fn recent_summary(&self, n: usize) -> Vec<&BiographyEntry> {
        let len = self.biography.len();
        let start = len.saturating_sub(n);
        self.biography[start..].iter().collect()
    }

    /// 人类可读的尾部摘要（plan §6.3 `recent_biography_summary`）。
    pub fn recent_summary_text(&self, n: usize) -> String {
        self.recent_summary(n)
            .iter()
            .map(|e| format_entry(e))
            .collect::<Vec<_>>()
            .join(" | ")
    }

    pub fn push_death_insight(&mut self, text: impl Into<String>, style: impl Into<String>) {
        let text = text.into();
        let tick = self.latest_death_tick().unwrap_or(self.created_at);
        if self
            .death_insights
            .last()
            .is_some_and(|record| record.tick == tick && record.text == text)
        {
            return;
        }

        self.death_insights.push(DeathInsightRecord {
            tick,
            text,
            style: style.into(),
        });
    }

    fn latest_death_tick(&self) -> Option<u64> {
        self.biography.iter().rev().find_map(|entry| match entry {
            BiographyEntry::NearDeath { tick, .. } | BiographyEntry::Terminated { tick, .. } => {
                Some(*tick)
            }
            _ => None,
        })
    }

    pub fn push_skill_milestone(&mut self, milestone: SkillMilestone) {
        self.skill_milestones.push(milestone);
    }

    pub fn recent_skill_milestones_summary_text(&self, n: usize) -> String {
        let len = self.skill_milestones.len();
        let start = len.saturating_sub(n);
        self.skill_milestones[start..]
            .iter()
            .map(format_skill_milestone)
            .collect::<Vec<_>>()
            .join(" | ")
    }
}

fn default_character_id() -> String {
    UNASSIGNED_CHARACTER_ID.to_string()
}

fn default_combat_hit_wound_kind() -> String {
    "Blunt".to_string()
}

fn default_tribulation_intercept_tag() -> String {
    TRIBULATION_INTERCEPT_TAG.to_string()
}

fn format_entry(entry: &BiographyEntry) -> String {
    match entry {
        BiographyEntry::BreakthroughStarted { realm_target, tick } => {
            format!("t{tick}:start→{realm_target:?}")
        }
        BiographyEntry::BreakthroughSucceeded { realm, tick } => format!("t{tick}:reach:{realm:?}"),
        BiographyEntry::SpiritEyeBreakthrough { eye_id, zone, tick } => {
            let zone = zone.as_deref().unwrap_or("-");
            format!("t{tick}:spirit_eye_breakthrough:{eye_id}:{zone}")
        }
        BiographyEntry::BreakthroughFailed {
            realm_target,
            severity,
            tick,
        } => format!("t{tick}:fail:{realm_target:?}:{severity:.2}"),
        BiographyEntry::MeridianOpened { id, tick } => format!("t{tick}:open:{id:?}"),
        BiographyEntry::MeridianClosed { id, tick, reason } => {
            format!("t{tick}:close:{id:?}:{reason}")
        }
        BiographyEntry::ForgedRate { id, tier, tick } => format!("t{tick}:rate:{id:?}→{tier}"),
        BiographyEntry::ForgedCapacity { id, tier, tick } => format!("t{tick}:cap:{id:?}→{tier}"),
        BiographyEntry::ColorShift {
            main,
            secondary,
            tick,
        } => format!("t{tick}:color:{main:?}/{secondary:?}"),
        BiographyEntry::InsightTaken {
            trigger,
            choice,
            tick,
        } => format!("t{tick}:insight:{trigger}:{choice}"),
        BiographyEntry::Rebirth {
            prior_realm,
            new_realm,
            tick,
        } => format!("t{tick}:rebirth:{prior_realm:?}→{new_realm:?}"),
        BiographyEntry::CombatHit {
            attacker_id,
            body_part,
            wound_kind,
            damage,
            tick,
        } => format!("t{tick}:combat:{attacker_id}:{body_part}:{wound_kind}:{damage:.1}"),
        BiographyEntry::JiemaiParry {
            attacker_id,
            effectiveness,
            tick,
        } => format!("t{tick}:jiemai_parry:{attacker_id}:{effectiveness:.2}"),
        BiographyEntry::NearDeath { cause, tick } => format!("t{tick}:near_death:{cause}"),
        BiographyEntry::Terminated { cause, tick } => format!("t{tick}:terminated:{cause}"),
        BiographyEntry::LifespanExtended {
            source,
            delta_years,
            tick,
        } => format!("t{tick}:lifespan_extended:{source}:{delta_years}"),
        BiographyEntry::DuoShePerformed {
            target_id,
            host_prev_age,
            target_age,
            tick,
        } => format!("t{tick}:duoshe:{target_id}:{host_prev_age:.1}->{target_age:.1}"),
        BiographyEntry::PossessedBy { host_id, tick } => format!("t{tick}:possessed_by:{host_id}"),
        BiographyEntry::AlchemyAttempt {
            recipe_id,
            pill,
            flawed_path,
            side_effect_tag,
            tick,
        } => {
            let flag = if *flawed_path { "flawed" } else { "exact" };
            let pill = pill.as_deref().unwrap_or("-");
            let side = side_effect_tag.as_deref().unwrap_or("-");
            format!("t{tick}:alchemy:{recipe_id}:{flag}:{pill}:{side}")
        }
        BiographyEntry::PlotHarvestedByOther {
            plot_pos,
            plant_id,
            tick,
        } => format!(
            "t{tick}:lingtian:harvested_by_other:[{},{},{}]:{plant_id}",
            plot_pos[0], plot_pos[1], plot_pos[2]
        ),
        BiographyEntry::PlotHarvestedFromOther {
            plot_pos,
            plant_id,
            tick,
        } => format!(
            "t{tick}:lingtian:harvested_from_other:[{},{},{}]:{plant_id}",
            plot_pos[0], plot_pos[1], plot_pos[2]
        ),
        BiographyEntry::PlotQiDrainedByOther {
            plot_pos,
            amount_drained,
            tick,
        } => format!(
            "t{tick}:lingtian:drained_by_other:[{},{},{}]:{amount_drained:.2}",
            plot_pos[0], plot_pos[1], plot_pos[2]
        ),
        BiographyEntry::PlotQiDrainedFromOther {
            plot_pos,
            amount_drained,
            tick,
        } => format!(
            "t{tick}:lingtian:drained_from_other:[{},{},{}]:{amount_drained:.2}",
            plot_pos[0], plot_pos[1], plot_pos[2]
        ),
        BiographyEntry::PlotDestroyedByOther { plot_pos, tick } => format!(
            "t{tick}:lingtian:destroyed_by_other:[{},{},{}]",
            plot_pos[0], plot_pos[1], plot_pos[2]
        ),
        BiographyEntry::TribulationIntercepted {
            victim_id,
            tag,
            tick,
        } => {
            format!("t{tick}:tribulation_intercepted:{victim_id}:{tag}")
        }
        BiographyEntry::TribulationFled { wave, tick } => {
            format!("t{tick}:tribulation_fled:wave{wave}:畏劫而逃")
        }
        BiographyEntry::HeartDemonRecord {
            outcome,
            choice_idx,
            tick,
        } => format!("t{tick}:heart_demon:{outcome:?}:{choice_idx:?}"),
        BiographyEntry::TradeCompleted {
            counterparty_id,
            offered_item,
            received_item,
            tick,
        } => format!("t{tick}:trade:{counterparty_id}:{offered_item}->{received_item}"),
        BiographyEntry::VortexProjectileDrained {
            projectile_id,
            drained_amount,
            tick,
        } => format!("t{tick}:woliu:drain:{projectile_id}:{drained_amount:.2}"),
        BiographyEntry::VortexBackfired { cause, tick } => {
            format!("t{tick}:woliu:backfire:{cause}")
        }
        BiographyEntry::AnqiSniped {
            attacker_id,
            distance_blocks,
            sealed_qi,
            hit_qi,
            tick,
        } => format!(
            "t{tick}:anqi_sniped:{attacker_id}:{distance_blocks:.1}:{sealed_qi:.1}:{hit_qi:.1}"
        ),
        BiographyEntry::SpawnTutorialCompleted {
            minutes_since_spawn,
            tick,
        } => format!("t{tick}:spawn_tutorial_completed:{minutes_since_spawn}m"),
    }
}

fn format_skill_milestone(milestone: &SkillMilestone) -> String {
    format!(
        "t{}:skill:{}:lv{}",
        milestone.achieved_at,
        milestone.skill.as_str(),
        milestone.new_lv
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::player::state::canonical_player_id;

    #[test]
    fn push_accumulates_indefinitely() {
        let mut lr = LifeRecord::default();
        for i in 0..1000 {
            lr.push(BiographyEntry::MeridianOpened {
                id: MeridianId::Lung,
                tick: i,
            });
        }
        assert_eq!(lr.biography.len(), 1000);
    }

    #[test]
    fn recent_summary_returns_tail() {
        let mut lr = LifeRecord::default();
        for i in 0..10 {
            lr.push(BiographyEntry::MeridianOpened {
                id: MeridianId::Lung,
                tick: i,
            });
        }
        assert_eq!(lr.recent_summary(3).len(), 3);
        assert_eq!(lr.recent_summary(100).len(), 10);
    }

    #[test]
    fn default_is_safe_with_unassigned_character_anchor() {
        let lr = LifeRecord::default();

        assert_eq!(lr.character_id, UNASSIGNED_CHARACTER_ID);
        assert_eq!(lr.created_at, 0);
        assert!(lr.biography.is_empty());
        assert!(lr.skill_milestones.is_empty());
        assert!(lr.recent_summary_text(5).is_empty());
    }

    #[test]
    fn new_sets_canonical_character_id_without_affecting_summary_text() {
        let mut lr = LifeRecord::new(canonical_player_id("Alice"));
        lr.push(BiographyEntry::MeridianOpened {
            id: MeridianId::Lung,
            tick: 12,
        });

        assert_eq!(lr.character_id, "offline:Alice");
        assert_eq!(lr.recent_summary_text(1), "t12:open:Lung");
    }

    #[test]
    fn combat_hit_summary_includes_wound_kind() {
        let mut lr = LifeRecord::new(canonical_player_id("Alice"));
        lr.push(BiographyEntry::CombatHit {
            attacker_id: "offline:Azure".to_string(),
            body_part: "Chest".to_string(),
            wound_kind: "Cut".to_string(),
            damage: 12.0,
            tick: 18,
        });

        assert_eq!(
            lr.recent_summary_text(1),
            "t18:combat:offline:Azure:Chest:Cut:12.0"
        );
    }

    #[test]
    fn serde_defaults_missing_character_id_for_legacy_records() {
        let legacy = serde_json::json!({
            "created_at": 5,
            "biography": [],
            "insights_taken": [],
            "skill_milestones": [],
            "spirit_root_first": null,
        });

        let decoded: LifeRecord =
            serde_json::from_value(legacy).expect("legacy life record should deserialize");

        assert_eq!(decoded.character_id, UNASSIGNED_CHARACTER_ID);
        assert!(decoded.death_insights.is_empty());
    }

    #[test]
    fn death_insight_records_latest_death_tick_and_dedupes_same_text() {
        let mut lr = LifeRecord::new(canonical_player_id("Alice"));
        lr.push(BiographyEntry::NearDeath {
            cause: "combat:test".to_string(),
            tick: 77,
        });

        lr.push_death_insight("你死前看见血谷东侧有灵气回流。", "perception");
        lr.push_death_insight("你死前看见血谷东侧有灵气回流。", "perception");

        assert_eq!(lr.death_insights.len(), 1);
        assert_eq!(lr.death_insights[0].tick, 77);
        assert_eq!(lr.death_insights[0].style, "perception");
    }

    #[test]
    fn legacy_combat_hit_defaults_wound_kind() {
        let legacy = serde_json::json!({
            "character_id": "offline:Alice",
            "created_at": 5,
            "biography": [{
                "CombatHit": {
                    "attacker_id": "offline:Azure",
                    "body_part": "Chest",
                    "damage": 9.0,
                    "tick": 7
                }
            }],
            "insights_taken": [],
            "skill_milestones": [],
            "spirit_root_first": null
        });

        let decoded: LifeRecord =
            serde_json::from_value(legacy).expect("legacy combat hit should deserialize");

        assert_eq!(
            decoded.recent_summary_text(1),
            "t7:combat:offline:Azure:Chest:Blunt:9.0"
        );
    }

    #[test]
    fn legacy_skill_milestones_default_to_empty() {
        let legacy = serde_json::json!({
            "character_id": "offline:Alice",
            "created_at": 5,
            "biography": [],
            "insights_taken": [],
            "spirit_root_first": null,
        });

        let decoded: LifeRecord =
            serde_json::from_value(legacy).expect("legacy life record should deserialize");

        assert!(decoded.skill_milestones.is_empty());
    }

    #[test]
    fn heart_demon_record_summary_is_public() {
        let mut lr = LifeRecord::new(canonical_player_id("Alice"));
        lr.push(BiographyEntry::HeartDemonRecord {
            outcome: HeartDemonOutcome::Steadfast,
            choice_idx: Some(0),
            tick: 233,
        });

        assert_eq!(
            lr.recent_summary_text(1),
            "t233:heart_demon:Steadfast:Some(0)"
        );
    }

    #[test]
    fn legacy_tribulation_intercept_defaults_ludao_tag() {
        let legacy = serde_json::json!({
            "character_id": "offline:Killer",
            "created_at": 5,
            "biography": [{
                "TribulationIntercepted": {
                    "victim_id": "offline:Victim",
                    "tick": 120
                }
            }],
            "insights_taken": [],
            "skill_milestones": [],
            "spirit_root_first": null
        });

        let decoded: LifeRecord = serde_json::from_value(legacy)
            .expect("legacy tribulation intercept should deserialize");

        assert_eq!(
            decoded.recent_summary_text(1),
            "t120:tribulation_intercepted:offline:Victim:戮道者 · 截劫"
        );
    }

    #[test]
    fn push_skill_milestone_appends() {
        let mut lr = LifeRecord::new(canonical_player_id("Alice"));
        lr.push_skill_milestone(SkillMilestone {
            skill: SkillId::Herbalism,
            new_lv: 3,
            achieved_at: 120,
            narration: "熟能近道，草木不再尽是草木。".to_string(),
            total_xp_at: 250,
        });

        assert_eq!(lr.skill_milestones.len(), 1);
        assert_eq!(lr.skill_milestones[0].new_lv, 3);
    }

    #[test]
    fn recent_skill_milestones_summary_returns_tail() {
        let mut lr = LifeRecord::new(canonical_player_id("Alice"));
        lr.push_skill_milestone(SkillMilestone {
            skill: SkillId::Herbalism,
            new_lv: 2,
            achieved_at: 80,
            narration: "草木渐熟。".to_string(),
            total_xp_at: 120,
        });
        lr.push_skill_milestone(SkillMilestone {
            skill: SkillId::Alchemy,
            new_lv: 3,
            achieved_at: 160,
            narration: "炉火识性稍深。".to_string(),
            total_xp_at: 420,
        });

        assert_eq!(
            lr.recent_skill_milestones_summary_text(1),
            "t160:skill:alchemy:lv3"
        );
        assert_eq!(
            lr.recent_skill_milestones_summary_text(5),
            "t80:skill:herbalism:lv2 | t160:skill:alchemy:lv3"
        );
    }
}
