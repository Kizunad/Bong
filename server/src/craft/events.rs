//! plan-craft-v1 §3 数据契约 — Craft 事件层。
//!
//! 4 类事件 + 顿悟触发枚举：
//!   * `CraftStartedEvent` — `start_craft` 成功后广播；UI/agent 接 narration
//!   * `CraftCompletedEvent` — tick 推进到 0，产出已写 inventory
//!   * `CraftFailedEvent` — 取消 / 材料丢失 / 玩家死亡等终止路径
//!   * `RecipeUnlockedEvent` — 三渠道解锁后写 RecipeUnlockState 同时广播
//!   * `InsightTrigger` — §3 plan 顿悟解锁路径触发源（§六:658）
//!
//! 守恒律：`CraftStartedEvent.qi_paid` 与 `qi_physics::ledger::QiTransfer`
//! 的 `amount` 必须相等且同源（同 caster），调用方在 `start_craft` 内
//! 一次性扣 ledger，事件只是观察通知。

use valence::prelude::{bevy_ecs, Entity, Event};

use super::recipe::RecipeId;

/// 顿悟触发源（worldview §六:658 关键时刻人生选择）。
///
/// `unlock_via_insight` 收到此 trigger → 弹选项 → 玩家选定后写 `RecipeUnlockedEvent`。
/// 三种典型来源（plan §0 设计轴心）：
///   * `Breakthrough` — 首次境界突破（cultivation::BreakthroughEvent 钩入）
///   * `NearDeath` — 濒死生还（HP < 15% 后救回）
///   * `DefeatStronger` — 击杀比自己境界高的对手（combat 接入）
///
/// 后续可扩；序列化 / 比较只看 variant，不挂额外 payload（payload 由 source plan 填）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InsightTrigger {
    Breakthrough,
    NearDeath,
    DefeatStronger,
}

impl InsightTrigger {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Breakthrough => "breakthrough",
            Self::NearDeath => "near_death",
            Self::DefeatStronger => "defeat_stronger",
        }
    }
}

/// `start_craft` 成功后立即广播。`qi_paid` 与 ledger 中实际扣除的 amount
/// 一致（守恒律观察记录）；caster 是发起手搓的玩家 entity。
#[derive(Debug, Clone, Event, PartialEq)]
pub struct CraftStartedEvent {
    pub caster: Entity,
    pub recipe_id: RecipeId,
    pub started_at_tick: u64,
    pub total_ticks: u64,
    pub qi_paid: f64,
}

/// tick 推进到 0 后 `finalize_craft` 写 inventory 并广播。`output_template` /
/// `output_count` 直接来自配方定义，UI 收到后刷右详情面板的"产出"行。
#[derive(Debug, Clone, Event, PartialEq)]
pub struct CraftCompletedEvent {
    pub caster: Entity,
    pub recipe_id: RecipeId,
    pub completed_at_tick: u64,
    pub output_template: String,
    pub output_count: u32,
}

/// 失败路径汇总：玩家取消 / 死亡清空 / 材料异常移除等。
/// `material_returned` 是按 §5 决策门 #3 默认 70% 返还后实际写回 inventory 的份数；
/// `qi_refunded` 当前固定 0（plan §5 决策门 #3 "qi 不退"）。
#[derive(Debug, Clone, Event, PartialEq)]
pub struct CraftFailedEvent {
    pub caster: Entity,
    pub recipe_id: RecipeId,
    pub reason: CraftFailureReason,
    pub material_returned: u32,
    pub qi_refunded: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CraftFailureReason {
    /// 玩家主动按"取消任务"按钮。返还策略 §5 决策门 #3 = B（70%）。
    PlayerCancelled,
    /// 玩家死亡（§5 决策门 #4 = A）。返还策略与 PlayerCancelled 一致，
    /// 但调用方负责清 session component。
    PlayerDied,
    /// 内部错误（recipe id 不存在 / 数据损坏等），属 fail-safe 路径。
    InternalError,
}

/// 三渠道解锁通用事件。`source` 区分残卷 / 师承 / 顿悟。
#[derive(Debug, Clone, Event, PartialEq)]
pub struct RecipeUnlockedEvent {
    pub caster: Entity,
    pub recipe_id: RecipeId,
    pub source: UnlockEventSource,
    pub unlocked_at_tick: u64,
}

/// 仅事件层用的解锁来源枚举（与 `recipe::UnlockSource` 区分：后者描述
/// 配方支持哪些来源，这个描述本次实际是从哪条路径触发的）。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum UnlockEventSource {
    Scroll { item_template: String },
    Mentor { npc_archetype: String },
    Insight { trigger: InsightTrigger },
}

/// plan-craft-v1 P2 — client → server 起手搓 intent。
/// `client_request_handler` 收到 `ClientRequestV1::CraftStart` 时 emit；
/// craft 模块的 `apply_craft_intents` 系统读后跑 `start_craft`，
/// 成功则 emit `CraftStartedEvent`，失败 emit `CraftFailedEvent`。
#[derive(Debug, Clone, Event, PartialEq, Eq)]
pub struct CraftStartIntent {
    pub caster: Entity,
    pub recipe_id: RecipeId,
    pub quantity: u32,
}

/// plan-craft-v1 P2 — client → server 取消 craft session intent。
/// 70% 材料返还 + qi 不退（§5 决策门 #3）。
#[derive(Debug, Clone, Copy, Event, PartialEq, Eq)]
pub struct CraftCancelIntent {
    pub caster: Entity,
}

/// plan-craft-v1 P3 §0 设计轴心 —— 三渠道解锁通用 intent。
///
/// 各 source plan（inventory ItemUse / social NPC dialog / cultivation
/// breakthrough/insight）按自身条件触发时 emit 一条 `CraftUnlockIntent`，
/// craft 模块的 `apply_unlock_intents` 系统统一处理：查 recipe → 跑
/// 对应 `unlock_via_*` → 写 `RecipeUnlockState` → emit `RecipeUnlockedEvent`。
///
/// 这种"intent 入口集中化"避免每个 source plan 各自写一份 unlock 流程，
/// 也让 worldview §九 信息差精神有单一可观察点（trace `apply_unlock_intents`
/// 即可看到全服解锁）。
#[derive(Debug, Clone, Event, PartialEq, Eq)]
pub struct CraftUnlockIntent {
    pub caster: Entity,
    pub recipe_id: RecipeId,
    pub source: UnlockEventSource,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insight_trigger_str_stable() {
        assert_eq!(InsightTrigger::Breakthrough.as_str(), "breakthrough");
        assert_eq!(InsightTrigger::NearDeath.as_str(), "near_death");
        assert_eq!(InsightTrigger::DefeatStronger.as_str(), "defeat_stronger");
    }

    #[test]
    fn craft_failure_reason_variants_distinct() {
        // 三个 variant 必须互不相等，否则上层无法判定路径
        assert_ne!(
            CraftFailureReason::PlayerCancelled,
            CraftFailureReason::PlayerDied
        );
        assert_ne!(
            CraftFailureReason::PlayerCancelled,
            CraftFailureReason::InternalError
        );
        assert_ne!(
            CraftFailureReason::PlayerDied,
            CraftFailureReason::InternalError
        );
    }

    #[test]
    fn unlock_event_source_carries_payload() {
        let s = UnlockEventSource::Scroll {
            item_template: "scroll_eclipse_needle".into(),
        };
        let m = UnlockEventSource::Mentor {
            npc_archetype: "poison_master".into(),
        };
        let i = UnlockEventSource::Insight {
            trigger: InsightTrigger::Breakthrough,
        };
        // 互相不等
        assert_ne!(s, m);
        assert_ne!(s, i);
        assert_ne!(m, i);
        // 但同 variant 同 payload 必须相等（事件去重 / 比较语义）
        let s2 = UnlockEventSource::Scroll {
            item_template: "scroll_eclipse_needle".into(),
        };
        assert_eq!(s, s2);
    }
}
