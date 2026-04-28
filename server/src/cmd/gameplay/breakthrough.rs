use crate::player::gameplay::GameplayAction;

pub fn action() -> GameplayAction {
    GameplayAction::AttemptBreakthrough
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn breakthrough_action_requests_attempt() {
        assert_eq!(action(), GameplayAction::AttemptBreakthrough);
    }
}
