//! 熔炉（plan-alchemy-v1 §1.2）。
//!
//! MVP：Component + 炉阶（tier）+ integrity（炸炉会扣）+ session 句柄。
//! 方块实体持久化留待 plan-persistence-v1 对接（本模块先在内存中表现）。

use serde::{Deserialize, Serialize};
use valence::prelude::{bevy_ecs, BlockPos, Component, Entity};

use super::session::AlchemySession;

/// 炉体组件。
/// - `tier` 决定可开火候精度 + 最高配方
/// - `owner` 只影响启动权限；None = 公共/无主
/// - `integrity` 炸炉会扣，0 时炉体损毁
/// - `session` 当前会话（None = 空闲）
/// - `pos` 世界坐标：`Some` = plan §1.2 放置在世界的方块炉；
///   `None` = 纯内存/测试构造（后续炼器共用抽象时也可能为 None）
#[derive(Debug, Clone, Component, Serialize, Deserialize)]
pub struct AlchemyFurnace {
    pub tier: u8,
    #[serde(default)]
    pub owner: Option<String>,
    pub integrity: f64,
    #[serde(default)]
    pub session: Option<AlchemySession>,
    /// 世界中关联的方块实体（BlockEntity），plan §1.3 离线持续性用。
    #[serde(default)]
    pub bound_entity: Option<u64>,
    /// plan §1.2 — 放置炉的世界坐标；`(x,y,z)` 元组避免引入 `BlockPos` 的 serde 依赖。
    #[serde(default)]
    pub pos: Option<(i32, i32, i32)>,
}

impl Default for AlchemyFurnace {
    fn default() -> Self {
        Self {
            tier: 1,
            owner: None,
            integrity: 1.0,
            session: None,
            bound_entity: None,
            pos: None,
        }
    }
}

impl AlchemyFurnace {
    pub fn new(tier: u8) -> Self {
        Self {
            tier,
            ..Default::default()
        }
    }

    /// plan §1.2 — 世界放置炉。`owner` 由调用方写入（玩家 username）。
    pub fn placed(pos: BlockPos, tier: u8) -> Self {
        Self {
            tier,
            pos: Some((pos.x, pos.y, pos.z)),
            ..Default::default()
        }
    }

    /// 还原 `pos` 为 `BlockPos`，若未放置返回 `None`。
    pub fn block_pos(&self) -> Option<BlockPos> {
        self.pos.map(|(x, y, z)| BlockPos { x, y, z })
    }

    pub fn can_run(&self, recipe_tier_min: u8) -> bool {
        self.integrity > 0.0 && self.tier >= recipe_tier_min
    }

    pub fn is_busy(&self) -> bool {
        self.session.as_ref().is_some_and(|s| !s.finished)
    }

    pub fn start_session(&mut self, session: AlchemySession) -> Result<(), String> {
        if self.is_busy() {
            return Err("furnace is busy with an ongoing session".into());
        }
        self.session = Some(session);
        Ok(())
    }

    pub fn end_session(&mut self) -> Option<AlchemySession> {
        let s = self.session.take()?;
        Some(s)
    }

    /// plan §1.3 炸炉 — 扣 integrity；返回是否炉体损毁。
    pub fn apply_explode(&mut self, integrity_damage: f64) -> bool {
        self.integrity = (self.integrity - integrity_damage).max(0.0);
        self.integrity <= 0.0
    }
}

/// 一个便捷的 Resource 形式，用来跟踪 entity→furnace 映射（若游戏层没挂 ECS 组件）。
/// MVP 里我们用 ECS Component，这里仅作兼容导出用。
#[derive(Debug, Clone, Copy)]
pub struct FurnaceRef(pub Entity);

/// plan §1.2 — 从 item `template_id` 映射到炉阶。
///
/// 更高阶炉（灵铁炉 tier 2 / 仙铁炉 tier 3）见 reminder.md，等 forge-v1 的品阶
/// 系统落地后批量补齐；配方 JSON 的 `furnace_tier_min` 会自动生效。
pub fn furnace_tier_from_item_id(template_id: &str) -> Option<u8> {
    match template_id {
        "furnace_fantie" => Some(1),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alchemy::session::AlchemySession;

    #[test]
    fn can_run_requires_tier_and_integrity() {
        let f = AlchemyFurnace::new(2);
        assert!(f.can_run(1));
        assert!(f.can_run(2));
        assert!(!f.can_run(3));
        let mut broken = AlchemyFurnace::new(2);
        broken.integrity = 0.0;
        assert!(!broken.can_run(1));
    }

    #[test]
    fn start_and_end_session() {
        let mut f = AlchemyFurnace::new(1);
        let session = AlchemySession::new("r".into(), "alice".into());
        f.start_session(session).unwrap();
        assert!(f.is_busy());
        let ended = f.end_session();
        assert!(ended.is_some());
        assert!(!f.is_busy());
    }

    #[test]
    fn cannot_start_when_busy() {
        let mut f = AlchemyFurnace::new(1);
        f.start_session(AlchemySession::new("r".into(), "a".into()))
            .unwrap();
        let again = f.start_session(AlchemySession::new("r".into(), "a".into()));
        assert!(again.is_err());
    }

    #[test]
    fn apply_explode_clamps_at_zero() {
        let mut f = AlchemyFurnace::new(1);
        assert!(!f.apply_explode(0.5));
        assert!(f.apply_explode(0.8)); // 毁
        assert_eq!(f.integrity, 0.0);
    }

    #[test]
    fn placed_carries_pos_and_tier() {
        let pos = BlockPos {
            x: -12,
            y: 64,
            z: 38,
        };
        let f = AlchemyFurnace::placed(pos, 2);
        assert_eq!(f.tier, 2);
        assert_eq!(f.pos, Some((-12, 64, 38)));
        assert_eq!(f.block_pos(), Some(pos));
    }

    #[test]
    fn new_has_no_pos() {
        let f = AlchemyFurnace::new(1);
        assert!(f.pos.is_none());
        assert!(f.block_pos().is_none());
    }

    #[test]
    fn furnace_tier_from_item_id_covers_fantie() {
        assert_eq!(furnace_tier_from_item_id("furnace_fantie"), Some(1));
        assert_eq!(furnace_tier_from_item_id("hoe_iron"), None);
        assert_eq!(furnace_tier_from_item_id(""), None);
    }
}
