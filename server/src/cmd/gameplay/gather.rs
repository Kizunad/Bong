use crate::player::gameplay::{GameplayAction, GatherAction};

pub fn action(resource: String) -> GameplayAction {
    GameplayAction::Gather(GatherAction {
        resource,
        target_entity: None,
        mode: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gather_action_uses_resource_without_target_or_mode() {
        assert_eq!(
            action("spirit_herb".to_string()),
            GameplayAction::Gather(GatherAction {
                resource: "spirit_herb".to_string(),
                target_entity: None,
                mode: None,
            })
        );
    }
}
