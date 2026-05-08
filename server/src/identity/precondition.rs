//! `/identity` slash command 的 precondition：玩家必须在自己灵龛 5 格内。
//!
//! worldview §十一 灵龛 = 安全空间 + 身份切换 = 洗白仪式；plan §0 设计轴心 Q4
//! "限灵龛 5 格内执行" + dev test 友好（spawn 临时灵龛即可测试）。
//!
//! 命中条件 = 玩家位置 ↔ 玩家自己拥有的某个 active（未被识破）灵龛 ≤ 5 格。
//! 灵龛被识破后失去安全语义（worldview §十一 灵龛识破 ≠ 安全空间），故不接受。

use valence::prelude::DVec3;

use crate::social::{position_is_within_own_active_spirit_niche, SpiritNicheRegistry};

/// 检查玩家是否处于自己未识破灵龛的安全半径内。
///
/// `actor_char_id` 取自 [`crate::combat::components::Lifecycle::character_id`]，
/// 形如 `offline:<username>`。
pub fn within_own_niche(actor_char_id: &str, pos: DVec3, registry: &SpiritNicheRegistry) -> bool {
    if actor_char_id.is_empty() {
        return false; // 无 character_id（极早期连接边角 case）→ 拒绝（命令 fail-safe）
    }
    position_is_within_own_active_spirit_niche(actor_char_id, pos, registry)
}

/// 拒绝原因——给 `/identity` 命令 handler 用，让消息一致。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NichePreconditionError {
    NotInOwnNiche,
    EmptyCharId,
}

impl NichePreconditionError {
    pub fn message(self) -> &'static str {
        match self {
            Self::NotInOwnNiche => "非灵龛内不可操心身份",
            Self::EmptyCharId => "身份未稳，命令异常",
        }
    }
}

/// 同 [`within_own_niche`] 但返回详细错误，便于命令 handler 选择消息。
pub fn check_within_own_niche(
    actor_char_id: &str,
    pos: DVec3,
    registry: &SpiritNicheRegistry,
) -> Result<(), NichePreconditionError> {
    if actor_char_id.is_empty() {
        return Err(NichePreconditionError::EmptyCharId);
    }
    if position_is_within_own_active_spirit_niche(actor_char_id, pos, registry) {
        Ok(())
    } else {
        Err(NichePreconditionError::NotInOwnNiche)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::social::components::SpiritNiche;

    fn install_niche(
        registry: &mut SpiritNicheRegistry,
        owner: &str,
        pos: [i32; 3],
        revealed: bool,
    ) {
        registry.upsert(SpiritNiche {
            owner: owner.to_string(),
            pos,
            placed_at_tick: 1,
            revealed,
            revealed_by: None,
            guardians: Vec::new(),
        });
    }

    #[test]
    fn empty_char_id_is_rejected() {
        let registry = SpiritNicheRegistry::default();
        assert_eq!(
            check_within_own_niche("", DVec3::ZERO, &registry),
            Err(NichePreconditionError::EmptyCharId)
        );
        assert!(!within_own_niche("", DVec3::ZERO, &registry));
    }

    #[test]
    fn no_niche_is_rejected() {
        let registry = SpiritNicheRegistry::default();
        assert_eq!(
            check_within_own_niche("offline:kiz", DVec3::ZERO, &registry),
            Err(NichePreconditionError::NotInOwnNiche)
        );
        assert!(!within_own_niche("offline:kiz", DVec3::ZERO, &registry));
    }

    #[test]
    fn within_own_niche_passes_at_center() {
        let mut registry = SpiritNicheRegistry::default();
        install_niche(&mut registry, "offline:kiz", [10, 64, 10], false);
        let pos = DVec3::new(10.5, 64.5, 10.5);
        assert!(check_within_own_niche("offline:kiz", pos, &registry).is_ok());
        assert!(within_own_niche("offline:kiz", pos, &registry));
    }

    #[test]
    fn within_own_niche_passes_at_edge_within_5_blocks() {
        let mut registry = SpiritNicheRegistry::default();
        install_niche(&mut registry, "offline:kiz", [10, 64, 10], false);
        // 距离恰好 ~4.99 格（半径 5.0）
        let pos = DVec3::new(15.0, 64.5, 10.5);
        assert!(check_within_own_niche("offline:kiz", pos, &registry).is_ok());
    }

    #[test]
    fn within_own_niche_rejects_outside_radius() {
        let mut registry = SpiritNicheRegistry::default();
        install_niche(&mut registry, "offline:kiz", [10, 64, 10], false);
        // 距离 > 5.5 格，明显在半径外
        let pos = DVec3::new(20.0, 64.5, 10.5);
        assert_eq!(
            check_within_own_niche("offline:kiz", pos, &registry),
            Err(NichePreconditionError::NotInOwnNiche)
        );
    }

    #[test]
    fn niche_owned_by_other_does_not_grant_safety() {
        let mut registry = SpiritNicheRegistry::default();
        install_niche(&mut registry, "offline:other", [10, 64, 10], false);
        let pos = DVec3::new(10.5, 64.5, 10.5);
        assert_eq!(
            check_within_own_niche("offline:kiz", pos, &registry),
            Err(NichePreconditionError::NotInOwnNiche)
        );
    }

    #[test]
    fn revealed_own_niche_no_longer_safe() {
        let mut registry = SpiritNicheRegistry::default();
        install_niche(&mut registry, "offline:kiz", [10, 64, 10], true);
        let pos = DVec3::new(10.5, 64.5, 10.5);
        assert_eq!(
            check_within_own_niche("offline:kiz", pos, &registry),
            Err(NichePreconditionError::NotInOwnNiche)
        );
    }

    #[test]
    fn niche_precondition_error_messages_are_chinese() {
        assert_eq!(
            NichePreconditionError::NotInOwnNiche.message(),
            "非灵龛内不可操心身份"
        );
        assert_eq!(
            NichePreconditionError::EmptyCharId.message(),
            "身份未稳，命令异常"
        );
    }
}
