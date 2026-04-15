//! plan-forge-v1 §1.4 LearnedBlueprints。
//!
//! 类比 cultivation 的 LearnedRecipes —— 拖【图谱残卷】item 到图谱卷轴区学习。

use serde::{Deserialize, Serialize};
use valence::prelude::{bevy_ecs, Component};

use super::blueprint::BlueprintId;

#[derive(Debug, Clone, Default, Component, Serialize, Deserialize)]
pub struct LearnedBlueprints {
    pub ids: Vec<BlueprintId>,
    #[serde(default)]
    pub current_index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LearnOutcome {
    Learned,
    AlreadyKnown,
}

impl LearnedBlueprints {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn learn(&mut self, id: BlueprintId) -> LearnOutcome {
        if self.ids.iter().any(|x| x == &id) {
            LearnOutcome::AlreadyKnown
        } else {
            self.ids.push(id);
            LearnOutcome::Learned
        }
    }

    pub fn knows(&self, id: &str) -> bool {
        self.ids.iter().any(|x| x == id)
    }

    pub fn current(&self) -> Option<&BlueprintId> {
        self.ids.get(self.current_index)
    }

    pub fn next_page(&mut self) {
        if self.ids.is_empty() {
            return;
        }
        self.current_index = (self.current_index + 1) % self.ids.len();
    }

    pub fn prev_page(&mut self) {
        if self.ids.is_empty() {
            return;
        }
        if self.current_index == 0 {
            self.current_index = self.ids.len() - 1;
        } else {
            self.current_index -= 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn learn_dedup() {
        let mut lb = LearnedBlueprints::new();
        assert_eq!(lb.learn("a".into()), LearnOutcome::Learned);
        assert_eq!(lb.learn("a".into()), LearnOutcome::AlreadyKnown);
        assert!(lb.knows("a"));
    }

    #[test]
    fn pagination_wraps() {
        let mut lb = LearnedBlueprints::new();
        lb.learn("a".into());
        lb.learn("b".into());
        lb.learn("c".into());
        assert_eq!(lb.current().unwrap(), "a");
        lb.next_page();
        assert_eq!(lb.current().unwrap(), "b");
        lb.next_page();
        lb.next_page(); // wraps
        assert_eq!(lb.current().unwrap(), "a");
        lb.prev_page();
        assert_eq!(lb.current().unwrap(), "c");
    }
}
