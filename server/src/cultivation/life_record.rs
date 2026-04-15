//! 修炼侧生平卷（plan §1.1, §11-4）— 全量保留修炼事件，无 sliding window。
//!
//! 死亡终结 / 亡者博物馆归档由战斗 plan 扩展同一 character_id，本 plan 不感知。

use serde::{Deserialize, Serialize};
use valence::prelude::{bevy_ecs, Component};

use super::components::{ColorKind, MeridianId, Realm};

const UNASSIGNED_CHARACTER_ID: &str = "unassigned:life_record";

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
    NearDeath {
        cause: String,
        tick: u64,
    },
    Terminated {
        cause: String,
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

#[derive(Debug, Clone, Component, Serialize, Deserialize)]
pub struct LifeRecord {
    #[serde(default = "default_character_id")]
    pub character_id: String,
    pub created_at: u64,
    pub biography: Vec<BiographyEntry>,
    pub insights_taken: Vec<TakenInsight>,
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
}

fn default_character_id() -> String {
    UNASSIGNED_CHARACTER_ID.to_string()
}

fn default_combat_hit_wound_kind() -> String {
    "Blunt".to_string()
}

fn format_entry(entry: &BiographyEntry) -> String {
    match entry {
        BiographyEntry::BreakthroughStarted { realm_target, tick } => {
            format!("t{tick}:start→{realm_target:?}")
        }
        BiographyEntry::BreakthroughSucceeded { realm, tick } => format!("t{tick}:reach:{realm:?}"),
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
        BiographyEntry::NearDeath { cause, tick } => format!("t{tick}:near_death:{cause}"),
        BiographyEntry::Terminated { cause, tick } => format!("t{tick}:terminated:{cause}"),
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
    }
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
            "spirit_root_first": null,
        });

        let decoded: LifeRecord =
            serde_json::from_value(legacy).expect("legacy life record should deserialize");

        assert_eq!(decoded.character_id, UNASSIGNED_CHARACTER_ID);
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
            "spirit_root_first": null
        });

        let decoded: LifeRecord =
            serde_json::from_value(legacy).expect("legacy combat hit should deserialize");

        assert_eq!(
            decoded.recent_summary_text(1),
            "t7:combat:offline:Azure:Chest:Blunt:9.0"
        );
    }
}
