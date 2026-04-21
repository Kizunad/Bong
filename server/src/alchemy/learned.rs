//! 方子学习与切换（plan-alchemy-v1 §1.4）。

use serde::{Deserialize, Serialize};
use valence::prelude::{bevy_ecs, Component};

use super::recipe::RecipeId;

/// 已学方子（玩家组件）。初始空；学习一张残卷 → push RecipeId。
#[derive(Debug, Clone, Default, Component, Serialize, Deserialize)]
pub struct LearnedRecipes {
    pub ids: Vec<RecipeId>,
    /// 当前卷轴翻到第几张。0 = 尚未翻页；总是 < ids.len()。
    #[serde(default)]
    pub current_index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LearnResult {
    Learned,
    AlreadyKnown,
}

impl LearnedRecipes {
    pub fn learn(&mut self, id: RecipeId) -> LearnResult {
        if self.ids.iter().any(|x| x == &id) {
            return LearnResult::AlreadyKnown;
        }
        self.ids.push(id);
        LearnResult::Learned
    }

    pub fn current(&self) -> Option<&RecipeId> {
        self.ids.get(self.current_index)
    }

    pub fn next(&mut self) -> Option<&RecipeId> {
        if self.ids.is_empty() {
            return None;
        }
        self.current_index = (self.current_index + 1) % self.ids.len();
        self.current()
    }

    pub fn prev(&mut self) -> Option<&RecipeId> {
        if self.ids.is_empty() {
            return None;
        }
        self.current_index = if self.current_index == 0 {
            self.ids.len() - 1
        } else {
            self.current_index - 1
        };
        self.current()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn learn_new_id_records_and_returns_learned() {
        let mut lr = LearnedRecipes::default();
        assert_eq!(lr.learn("a".into()), LearnResult::Learned);
        assert_eq!(lr.ids, vec!["a".to_string()]);
    }

    #[test]
    fn learn_duplicate_returns_already_known() {
        let mut lr = LearnedRecipes::default();
        lr.learn("a".into());
        assert_eq!(lr.learn("a".into()), LearnResult::AlreadyKnown);
    }

    #[test]
    fn next_and_prev_cycle() {
        let mut lr = LearnedRecipes::default();
        lr.learn("a".into());
        lr.learn("b".into());
        lr.learn("c".into());
        assert_eq!(lr.current().unwrap(), "a");
        assert_eq!(lr.next().unwrap(), "b");
        assert_eq!(lr.next().unwrap(), "c");
        assert_eq!(lr.next().unwrap(), "a"); // wrap
        assert_eq!(lr.prev().unwrap(), "c"); // wrap-back
    }

    #[test]
    fn next_on_empty_returns_none() {
        let mut lr = LearnedRecipes::default();
        assert!(lr.next().is_none());
    }
}
