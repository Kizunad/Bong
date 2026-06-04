use super::{finite_non_negative, QiPhysicsError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AnqiContainerKind {
    HandSlot,
    Quiver,
    PocketPouch,
    Fenglinghe,
}

impl AnqiContainerKind {
    /// 返回跨端 wire 契约字符串（snake_case，与 client `AnqiHudServerDataHandler` 期望一致）。
    ///
    /// **穷尽 match，无 catch-all**：新增变体时编译器强制处理，防止 wire 串静默漂移。
    pub fn as_wire_str(self) -> &'static str {
        match self {
            Self::HandSlot => "hand_slot",
            Self::Quiver => "quiver",
            Self::PocketPouch => "pocket_pouch",
            Self::Fenglinghe => "fenglinghe",
        }
    }

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

    // ── wire 契约 pin：as_wire_str 每变体显式锁定，不走 Debug ────

    #[test]
    fn wire_str_hand_slot() {
        assert_eq!(
            AnqiContainerKind::HandSlot.as_wire_str(),
            "hand_slot",
            "HandSlot wire 契约必须为 'hand_slot'（client AnqiHudServerDataHandler 期望值）；实际={}",
            AnqiContainerKind::HandSlot.as_wire_str()
        );
    }

    #[test]
    fn wire_str_quiver() {
        assert_eq!(
            AnqiContainerKind::Quiver.as_wire_str(),
            "quiver",
            "Quiver wire 契约必须为 'quiver'；实际={}",
            AnqiContainerKind::Quiver.as_wire_str()
        );
    }

    #[test]
    fn wire_str_pocket_pouch() {
        assert_eq!(
            AnqiContainerKind::PocketPouch.as_wire_str(),
            "pocket_pouch",
            "PocketPouch wire 契约必须为 'pocket_pouch'（非 'pocketpouch'，Debug 输出不可作契约）；实际={}",
            AnqiContainerKind::PocketPouch.as_wire_str()
        );
    }

    #[test]
    fn wire_str_fenglinghe() {
        assert_eq!(
            AnqiContainerKind::Fenglinghe.as_wire_str(),
            "fenglinghe",
            "Fenglinghe wire 契约必须为 'fenglinghe'；实际={}",
            AnqiContainerKind::Fenglinghe.as_wire_str()
        );
    }

    /// Debug 输出与 wire_str 的偏差验证：
    /// HandSlot/PocketPouch 的 Debug 会产生不含下划线的拼合串，
    /// 此测试明确记录 Debug 不等于 wire_str，作为不可回退的保护网。
    #[test]
    fn debug_output_differs_from_wire_str_for_multiword_variants() {
        // HandSlot: Debug="HandSlot", wire="hand_slot"
        assert_ne!(
            format!("{:?}", AnqiContainerKind::HandSlot).to_lowercase(),
            AnqiContainerKind::HandSlot.as_wire_str(),
            "HandSlot Debug 输出（无下划线）与 wire_str（有下划线）必须不同——\
             此测试防止未来有人误用 Debug 作 wire 契约"
        );
        // PocketPouch: Debug="PocketPouch", wire="pocket_pouch"
        assert_ne!(
            format!("{:?}", AnqiContainerKind::PocketPouch).to_lowercase(),
            AnqiContainerKind::PocketPouch.as_wire_str(),
            "PocketPouch Debug 输出（无下划线）与 wire_str（有下划线）必须不同"
        );
    }

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
