//! big-brain `IdentityReactionScorer` — NPC 对玩家 active identity 反应分级的
//! Utility AI 评分（plan-identity-v1 P3）。
//!
//! 接入流程（NPC AI 想加入这套打分）：
//!
//! ```ignore
//! commands.spawn((
//!     Actor(npc),
//!     Score::default(),
//!     IdentityReactionScorer,
//! ));
//! ```
//!
//! 评分逻辑：
//! - NPC `nearest_player` 指向某玩家 → 读其 [`IdentityReactionState`]
//! - score = [`ReactionTier::scorer_value`]（Wanted=1.0 / Low=0.6 / Normal=High=0.0）
//! - 没有 nearest_player / 玩家 entity 已不存在 / 无 IdentityReactionState → 0.0
//!
//! 不直接覆盖既有 ChaseAction / FleeAction 决策——score 通过 big-brain Thinker
//! 与其他 scorer 自然竞争，Wanted 玩家最终更可能触发 Chase / 拒绝交易。

use big_brain::prelude::{Actor, Score, ScorerBuilder};
use valence::prelude::{bevy_ecs, App, Commands, Component, Entity, Query, Update, With};

use super::reaction::IdentityReactionState;
use crate::npc::spawn::{NpcBlackboard, NpcMarker};

/// big-brain Scorer 标记 Component。NPC 想 opt-in 反应分级行为时挂上即可。
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct IdentityReactionScorer;

impl ScorerBuilder for IdentityReactionScorer {
    fn build(&self, cmd: &mut Commands, scorer: Entity, _actor: Entity) {
        cmd.entity(scorer).insert(*self);
    }

    fn label(&self) -> Option<&str> {
        Some("IdentityReactionScorer")
    }
}

/// 注册 scorer 系统。
pub fn register(app: &mut App) {
    app.add_systems(Update, identity_reaction_scorer_system);
}

pub fn identity_reaction_scorer_system(
    npcs: Query<&NpcBlackboard, With<NpcMarker>>,
    players: Query<&IdentityReactionState, With<valence::prelude::Client>>,
    mut scorers: Query<(&Actor, &mut Score), With<IdentityReactionScorer>>,
) {
    for (Actor(actor), mut score) in &mut scorers {
        let value = match npcs.get(*actor) {
            Ok(bb) => match bb.nearest_player.and_then(|e| players.get(e).ok()) {
                Some(state) => state.tier.scorer_value(),
                None => 0.0,
            },
            Err(_) => 0.0,
        };
        score.set(value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::reaction::ReactionTier;

    #[test]
    fn scorer_label_is_named() {
        let scorer = IdentityReactionScorer;
        assert_eq!(scorer.label(), Some("IdentityReactionScorer"));
    }

    #[test]
    fn scorer_value_for_each_tier() {
        // 这层小测把 Tier→score 的映射锁死，确保未来调 scorer_value 不会沉默通过
        assert_eq!(ReactionTier::Wanted.scorer_value(), 1.0);
        assert_eq!(ReactionTier::Low.scorer_value(), 0.6);
        assert_eq!(ReactionTier::Normal.scorer_value(), 0.0);
        assert_eq!(ReactionTier::High.scorer_value(), 0.0);
    }
}
