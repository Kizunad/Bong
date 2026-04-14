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
}
