use crate::player::gameplay::{CombatAction, GameplayAction};

pub fn action(target: String, qi_invest: f64) -> GameplayAction {
    GameplayAction::Combat(CombatAction { target, qi_invest })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn combat_action_preserves_target_and_qi_invest() {
        assert_eq!(
            action("Crimson".to_string(), 12.5),
            GameplayAction::Combat(CombatAction {
                target: "Crimson".to_string(),
                qi_invest: 12.5,
            })
        );
    }
}
