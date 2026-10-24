//! 熔炉（plan-alchemy-v1 §1.2）。
//!
//! MVP：Component + 炉阶（tier）+ integrity（炸炉会扣）+ session 句柄。
//! 方块实体持久化留待 plan-persistence-v1 对接（本模块先在内存中表现）。

use crate::world::dimension::DimensionKind;
use serde::{Deserialize, Serialize};
use valence::prelude::{bevy_ecs, BlockPos, Component, DVec3, Entity};

use super::{adapter::AlchemySessionAdapter, session::AlchemySession};

/// 当前世界炉只生成在主世界；坐标请求不得跨维度或隔空控制工位。
pub(crate) fn within_reach(
    position: DVec3,
    dimension: DimensionKind,
    pos: (i32, i32, i32),
) -> bool {
    let center = DVec3::new(
        f64::from(pos.0) + 0.5,
        f64::from(pos.1),
        f64::from(pos.2) + 0.5,
    );
    dimension == DimensionKind::Overworld
        && crate::reach::DistanceRule::NEARBY_INTERACT.allows(position, center)
}

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
    /// 当前炼丹会话的唯一权威存储；域状态通过 adapter 的 `Deref` 暴露给既有算法。
    #[serde(default)]
    pub session: Option<AlchemySessionAdapter>,
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
    /// 公共炉允许使用，但正在炼制或等待收取的炉次只属于原施术者。
    pub fn can_access(&self, player_id: &str) -> bool {
        let same_player = |id: &str| {
            id.strip_prefix("offline:").unwrap_or(id)
                == player_id.strip_prefix("offline:").unwrap_or(player_id)
        };
        self.owner
            .as_deref()
            .is_none_or(|owner| owner.is_empty() || same_player(owner))
            && self
                .session
                .as_ref()
                .is_none_or(|session| same_player(&session.caster_id))
    }

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

    /// 兼容纯内存测试构造；生产放置炉应使用 [`Self::start_session_at`]，把当前实体
    /// 和坐标稳定身份一起登记。无坐标的测试炉使用固定的内存身份，不把 Entity Debug
    /// 字符串写入 checkpoint。
    pub fn start_session(&mut self, session: AlchemySession) -> Result<(), String> {
        let placed_id = self
            .stable_placed_id()
            .unwrap_or_else(|| "alchemy:furnace:memory".to_string());
        let entity = self
            .bound_entity
            .and_then(|bits| Entity::try_from_bits(bits).ok())
            .unwrap_or_else(|| Entity::from_raw(0));
        self.start_session_with_identity(session, entity, placed_id)
    }

    /// 生产起炉入口：稳定工位身份来自放置坐标，运行期 Entity 只作为可替换 locator。
    pub fn start_session_at(
        &mut self,
        furnace_entity: Entity,
        session: AlchemySession,
    ) -> Result<(), String> {
        let placed_id = self
            .stable_placed_id()
            .ok_or_else(|| "炼丹炉缺少稳定放置身份".to_string())?;
        self.bound_entity = Some(furnace_entity.to_bits());
        self.start_session_with_identity(session, furnace_entity, placed_id)
    }

    fn start_session_with_identity(
        &mut self,
        session: AlchemySession,
        furnace_entity: Entity,
        placed_id: String,
    ) -> Result<(), String> {
        if self.session.is_some() {
            return Err("请先收取上一炉结果".into());
        }
        let session_key = format!("alchemy:session:{placed_id}");
        self.session = Some(
            AlchemySessionAdapter::from_furnace(
                session,
                session_key,
                placed_id.clone(),
                furnace_entity,
                placed_id,
                DimensionKind::Overworld,
            )
            .map_err(|error| format!("炼丹会话身份无效：{error}"))?,
        );
        Ok(())
    }

    pub fn end_session(&mut self) -> Option<AlchemySessionAdapter> {
        let s = self.session.take()?;
        Some(s)
    }

    /// 跨重启稳定的炉身份。坐标是放置炉的持久业务键，不使用运行期 Entity。
    pub fn stable_placed_id(&self) -> Option<String> {
        self.pos
            .map(|(x, y, z)| format!("alchemy:furnace:overworld:{x}:{y}:{z}"))
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
        "furnace_lingtie" => Some(2),
        "furnace_xitie" => Some(3),
        _ => None,
    }
}
