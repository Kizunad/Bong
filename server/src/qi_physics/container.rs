use super::{finite_non_negative, QiPhysicsError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AnqiContainerKind {
    HandSlot,
    Quiver,
    PocketPouch,
    Fenglinghe,
}

impl AnqiContainerKind {
    pub fn capacity(self) -> u8 {
        match self {
            Self::HandSlot => 1,
            Self::Quiver => 12,
            Self::PocketPouch => 4,
            Self::Fenglinghe => 6,
        }
    }

    pub fn tax_rate(self) -> f64 {
        match self {
            Self::HandSlot | Self::Fenglinghe => 0.0,
            Self::Quiver => 0.05,
            Self::PocketPouch => 0.08,
        }
    }

    pub fn allows_combat_swap(self) -> bool {
        !matches!(self, Self::Fenglinghe)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AbrasionDirection {
    Store,
    Draw,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AbrasionOutcome {
    pub before_qi: f64,
    pub lost_qi: f64,
    pub after_qi: f64,
    pub direction: AbrasionDirection,
    pub container: AnqiContainerKind,
}

pub fn abrasion_loss(
    qi_payload: f64,
    container: AnqiContainerKind,
    direction: AbrasionDirection,
) -> Result<AbrasionOutcome, QiPhysicsError> {
    let before = finite_non_negative(qi_payload, "abrasion.qi_payload")?;
    let lost = before * container.tax_rate();
    Ok(AbrasionOutcome {
        before_qi: before,
        lost_qi: lost,
        after_qi: before - lost,
        direction,
        container,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quiver_taxes_five_percent_each_move() {
        let out =
            abrasion_loss(100.0, AnqiContainerKind::Quiver, AbrasionDirection::Store).unwrap();
        assert_eq!(out.lost_qi, 5.0);
        assert_eq!(out.after_qi, 95.0);
    }

    #[test]
    fn hand_and_fenglinghe_are_tax_free() {
        assert_eq!(
            abrasion_loss(80.0, AnqiContainerKind::HandSlot, AbrasionDirection::Draw)
                .unwrap()
                .lost_qi,
            0.0
        );
        assert_eq!(
            abrasion_loss(
                80.0,
                AnqiContainerKind::Fenglinghe,
                AbrasionDirection::Store
            )
            .unwrap()
            .lost_qi,
            0.0
        );
    }

    #[test]
    fn fenglinghe_cannot_swap_in_combat() {
        assert!(!AnqiContainerKind::Fenglinghe.allows_combat_swap());
        assert!(AnqiContainerKind::Quiver.allows_combat_swap());
    }
}
