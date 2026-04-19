//! plan-lingtian-v1 §1.2.2 — 地形适合性检查（开垦前置）。
//!
//! 适合：grass / dirt / swamp_mud。不适合：沙 / 石 / 冰 / 死域。
//! 真实方块判定走 valence::block::BlockKind，但本模块用一个简洁的
//! `TerrainKind` 抽象，允许 session 层免依赖 valence types 做单测。

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerrainKind {
    Grass,
    Dirt,
    SwampMud,
    Sand,
    Stone,
    Ice,
    DeadZone,
    /// 任何上面没列出的方块（树叶、水、空气等）。
    Unknown,
}

impl TerrainKind {
    /// plan §1.2.2 步骤 1 — 可开垦地形（grass / dirt / swamp_mud）。
    pub fn is_tillable(self) -> bool {
        matches!(self, Self::Grass | Self::Dirt | Self::SwampMud)
    }

    /// plan §1.2.2 步骤 4 — 明确不适合（用于 UI 灰掉"开始"）。
    /// 注意 `Unknown` 也不适合，但与"明确禁止"分开，方便日志区分。
    pub fn is_explicitly_blocked(self) -> bool {
        matches!(self, Self::Sand | Self::Stone | Self::Ice | Self::DeadZone)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TillRejectReason {
    /// 地形明确不适合（沙 / 石 / 冰 / 死域）。
    BlockedTerrain,
    /// 既非可开垦也非明确禁止 —— 比如树叶、水方块。UI 也应灰掉，但语义不同。
    UnsupportedTerrain,
}

pub fn classify_for_till(terrain: TerrainKind) -> Result<(), TillRejectReason> {
    if terrain.is_tillable() {
        Ok(())
    } else if terrain.is_explicitly_blocked() {
        Err(TillRejectReason::BlockedTerrain)
    } else {
        Err(TillRejectReason::UnsupportedTerrain)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tillable_terrains() {
        assert!(classify_for_till(TerrainKind::Grass).is_ok());
        assert!(classify_for_till(TerrainKind::Dirt).is_ok());
        assert!(classify_for_till(TerrainKind::SwampMud).is_ok());
    }

    #[test]
    fn explicitly_blocked_terrains() {
        for t in [
            TerrainKind::Sand,
            TerrainKind::Stone,
            TerrainKind::Ice,
            TerrainKind::DeadZone,
        ] {
            assert_eq!(classify_for_till(t), Err(TillRejectReason::BlockedTerrain));
        }
    }

    #[test]
    fn unknown_terrain_is_unsupported_not_blocked() {
        assert_eq!(
            classify_for_till(TerrainKind::Unknown),
            Err(TillRejectReason::UnsupportedTerrain),
        );
    }
}
