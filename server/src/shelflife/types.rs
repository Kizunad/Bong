//! plan-shelflife-v1 §2 核心类型定义。
//!
//! v1 选 **enum 分支** 承载 DecayProfile（而非 Option struct，见 plan §2.2 M0 决策）。

use serde::{Deserialize, Serialize};

/// plan §0.1 三条降级路径。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DecayTrack {
    /// 衰减 — 灵气逸散（灵石 / 骨币 / 残卷）。
    Decay,
    /// 腐败 — 消费触发 contam / 中毒（兽血 / 兽肉 / 鲜草 / 过期丹）。
    Spoil,
    /// 陈化 — 峰值超值 + 过峰 Spoil 迁移（陈酒 / 老坛丹）。
    Age,
}

/// plan §8 DecayProfile 注册表的唯一 ID。字符串包装，按正典矿物 / 药材 / 丹药命名。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DecayProfileId(pub String);

impl DecayProfileId {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// plan §1 衰减公式（Decay / Spoil 路径共用）。
///
/// Age 路径有自身的 PeakAndFall 逻辑，不走此 enum — 见 `DecayProfile::Age`。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DecayFormula {
    /// plan §1.1 — `current = initial * 0.5 ^ (dt / half_life)`。
    Exponential { half_life_ticks: u64 },
    /// plan §1.2 — `current = max(0, initial - decay_per_tick * dt)`。
    Linear { decay_per_tick: f32 },
    /// plan §1.3 — `current = initial * storage_multiplier`（不使用 dt；由 container 给 multiplier）。
    Stepwise,
}

/// plan §2.2 DecayProfile 定义（v1 选 enum 分支）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DecayProfile {
    /// Decay 路径 — 衰减至 `floor_qi` 后判死物。
    Decay {
        id: DecayProfileId,
        formula: DecayFormula,
        /// 死物残值（current_qi 跌至此值后不再衰减，item ID 变体切换触发点）。
        floor_qi: f32,
    },
    /// Spoil 路径 — 跌至 `spoil_threshold` 以下判腐败。
    Spoil {
        id: DecayProfileId,
        formula: DecayFormula,
        /// 腐败阈值（current_qi 低于此值时消费触发 `SpoilConsumeWarning`）。
        spoil_threshold: f32,
    },
    /// Age 路径 — PeakAndFall 内建 + 过峰 Spoil 迁移（plan §1.4）。
    Age {
        id: DecayProfileId,
        /// 到达峰值的 tick 数（从 `created_at_tick` 算起）。
        peak_at_ticks: u64,
        /// 峰值加成系数（0.5 = 峰值为 initial × 1.5）。
        peak_bonus: f32,
        /// 过峰后的指数衰减半衰期。
        post_peak_half_life_ticks: u64,
        /// 过峰后跌至此值时路径迁移 Age → Spoil。
        post_peak_spoil_threshold: f32,
        /// 迁移后挂的 Spoil profile。
        post_peak_spoil_profile: DecayProfileId,
    },
}

impl DecayProfile {
    pub fn id(&self) -> &DecayProfileId {
        match self {
            DecayProfile::Decay { id, .. }
            | DecayProfile::Spoil { id, .. }
            | DecayProfile::Age { id, .. } => id,
        }
    }

    pub fn track(&self) -> DecayTrack {
        match self {
            DecayProfile::Decay { .. } => DecayTrack::Decay,
            DecayProfile::Spoil { .. } => DecayTrack::Spoil,
            DecayProfile::Age { .. } => DecayTrack::Age,
        }
    }
}

/// plan §2.1 物品 NBT 扩展字段。挂在 inventory item 上，由 shelflife plan 定义，
/// `plan-inventory-v1` 负责在 InventoryItem 中实现 `freshness: Option<Freshness>` 字段（plan §0.4 边界约定）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Freshness {
    /// 物品 mined / harvested / crafted 时的 tick。
    pub created_at_tick: u64,
    /// 初始灵气 / 真元 / 药力含量。
    pub initial_qi: f32,
    /// 走哪条路径 — 冗余存储，避免每次 lookup profile 再取 track。
    pub track: DecayTrack,
    /// 指向 `DecayProfileRegistry` 的 ID。
    pub profile: DecayProfileId,
    /// 累积已冻结 ticks（进过 Freeze 容器的时长，lazy eval 时从 dt 减去）。
    #[serde(default)]
    pub frozen_accumulated: u64,
    /// 当前进入 Freeze 容器的 tick；`Some` = 正在冻结，`None` = 未冻结。
    /// 离开容器时 `frozen_accumulated += now - frozen_since_tick`，然后置 None。
    #[serde(default)]
    pub frozen_since_tick: Option<u64>,
}

impl Freshness {
    pub fn new(created_at_tick: u64, initial_qi: f32, profile: &DecayProfile) -> Self {
        Self {
            created_at_tick,
            initial_qi,
            track: profile.track(),
            profile: profile.id().clone(),
            frozen_accumulated: 0,
            frozen_since_tick: None,
        }
    }
}

/// plan §8 `compute_track_state` 返回的当下路径状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackState {
    /// Decay — 未跌至 floor_qi，或 Spoil 未跌至 spoil_threshold，或 Age 峰前。
    Fresh,
    /// Decay 路径 — 已跌破半衰期但未达 floor_qi。
    Declining,
    /// Decay 路径 — 已至 floor_qi，item ID 应切"死 X"。
    Dead,
    /// Spoil 路径 — 已跌至 spoil_threshold 以下，消费触发 contam 警告。
    Spoiled,
    /// Age 路径 — 处于 peak_at_ticks 附近 ±10% 窗口，消费触发 bonus。
    Peaking,
    /// Age 路径 — 已过峰值，尚未到 post_peak_spoil_threshold。
    PastPeak,
    /// Age 路径已跌破 post_peak_spoil_threshold，路径迁移为 Spoil。
    /// 此状态下调用方应在存储层把 `Freshness.track/profile` 更新为 Spoil 配置。
    AgePostPeakSpoiled,
}
