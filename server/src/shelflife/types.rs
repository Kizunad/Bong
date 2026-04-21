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
        /// 峰值 `Peaking` 窗口比例：`[peak_at_ticks * (1 - r), peak_at_ticks * (1 + r)]`。
        /// 例 0.1 = ±10% 峰值窗口；陈年灵茶可宽 0.2，老坛丹可窄 0.05。
        peak_window_ratio: f32,
        /// 过峰后的指数衰减半衰期。
        post_peak_half_life_ticks: u64,
        /// 过峰后跌至此值时路径迁移 Age → Spoil（仅在 `effective_dt > peak_at_ticks` 后生效）。
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

    /// plan §10 开放问题 — registry 加载时校验配置合法性。
    /// 非全量校验（跨 profile 引用由 registry 层保证），仅保护单 profile 可算性。
    pub fn validate(&self) -> Result<(), String> {
        match self {
            DecayProfile::Decay {
                formula, floor_qi, ..
            } => {
                validate_formula(formula)?;
                if *floor_qi < 0.0 || !floor_qi.is_finite() {
                    return Err(format!(
                        "DecayProfile::Decay.floor_qi must be finite and ≥ 0, got {floor_qi}"
                    ));
                }
                Ok(())
            }
            DecayProfile::Spoil {
                formula,
                spoil_threshold,
                ..
            } => {
                validate_formula(formula)?;
                if *spoil_threshold < 0.0 || !spoil_threshold.is_finite() {
                    return Err(format!(
                        "DecayProfile::Spoil.spoil_threshold must be finite and ≥ 0, got {spoil_threshold}"
                    ));
                }
                Ok(())
            }
            DecayProfile::Age {
                peak_at_ticks,
                peak_bonus,
                peak_window_ratio,
                post_peak_spoil_threshold,
                ..
            } => {
                if *peak_at_ticks == 0 {
                    return Err("DecayProfile::Age.peak_at_ticks must be > 0".into());
                }
                if !peak_bonus.is_finite() || *peak_bonus < 0.0 {
                    return Err(format!(
                        "DecayProfile::Age.peak_bonus must be finite and ≥ 0, got {peak_bonus}"
                    ));
                }
                if !peak_window_ratio.is_finite()
                    || *peak_window_ratio < 0.0
                    || *peak_window_ratio > 1.0
                {
                    return Err(format!(
                        "DecayProfile::Age.peak_window_ratio must be in [0.0, 1.0], got {peak_window_ratio}"
                    ));
                }
                if *post_peak_spoil_threshold < 0.0 || !post_peak_spoil_threshold.is_finite() {
                    return Err(format!(
                        "DecayProfile::Age.post_peak_spoil_threshold must be finite and ≥ 0, got {post_peak_spoil_threshold}"
                    ));
                }
                Ok(())
            }
        }
    }
}

fn validate_formula(formula: &DecayFormula) -> Result<(), String> {
    match formula {
        DecayFormula::Exponential { .. } => Ok(()),
        DecayFormula::Linear { decay_per_tick } => {
            if !decay_per_tick.is_finite() || *decay_per_tick < 0.0 {
                return Err(format!(
                    "DecayFormula::Linear.decay_per_tick must be finite and ≥ 0, got {decay_per_tick}"
                ));
            }
            Ok(())
        }
        DecayFormula::Stepwise => Ok(()),
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

/// plan §8 `compute_track_state` 返回的**内部路径机态**。
///
/// **注意**：此枚举是 **机态**（path state），不是 UI **显示档位**。plan §4 要求 tooltip
/// 显示 5 档（鲜品/微损/半衰/残留/死物 等），5 档细分应由 M3（tooltip 实装）阶段从
/// `current_qi / initial_qi` 比率 + TrackState 共同衍生，不在本枚举范围。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackState {
    /// Decay/Spoil: headroom 剩余 > 50% · Age: 峰前。
    Fresh,
    /// Decay: 已低于 half headroom 但未 Dead · Spoil: 同理但未 Spoiled。
    Declining,
    /// Decay: current ≤ floor_qi，item ID 应切"死 X"。
    Dead,
    /// Spoil: current ≤ spoil_threshold，消费触发 contam 警告。
    Spoiled,
    /// Age: 处于 `peak_at_ticks ± peak_window_ratio` 窗口，消费触发 bonus。
    Peaking,
    /// Age: 已过 `peak_hi`，但 current 仍 > post_peak_spoil_threshold。
    PastPeak,
    /// Age: 已过 `peak_at_ticks` 且 current ≤ post_peak_spoil_threshold。
    /// 调用方应在存储层把 `Freshness.track / profile` 更新为 Spoil 配置。
    AgePostPeakSpoiled,
}

/// plan §3 / §8 — 容器对物品保质期的 rate 影响行为。
///
/// 不同行为对不同 track / formula 的物品作用不同 —
/// 由 `container_storage_multiplier(behavior, profile)` 统一解析（见 container 模块）。
///
/// 实装映射：
/// - 凡俗箱子 → `Normal`
/// - 玉盒 / 灵匣 → `Halve`
/// - 阵法护匣 → `Freeze`
/// - 阴干架 / 干燥架 → `DryingRack`（仅 Stepwise 公式生效）
/// - 冰窖 → `SpoilOnly`（仅 Spoil 路径生效）
/// - 陈化窖 → `AgeAccelerate`（仅 Age 路径生效）
#[derive(Debug, Clone, PartialEq)]
pub enum ContainerFreshnessBehavior {
    /// 凡俗箱（基准）— rate ×1.0，全 track / formula 通行。
    Normal,
    /// 玉盒 / 灵匣 — Exp/Linear/Age 路径 rate ×0.5；对 Stepwise 公式不适用（退 Normal）。
    Halve,
    /// 阵法护匣 — 全 track 冻结；进入时记 `frozen_since_tick`，离开时 frozen_accumulated 增加。
    Freeze,
    /// 阴干架 / 干燥架 — 仅 Stepwise 公式生效，直接作用 multiplier；
    /// 非 Stepwise item 在此容器退 Normal。
    DryingRack { multiplier: f32 },
    /// 冰窖 — 仅 Spoil 路径生效，rate 缩放；非 Spoil item 在此退 Normal。
    SpoilOnly { rate: f32 },
    /// 陈化窖 — 仅 Age 路径生效，加速 peak_at_ticks（实装为 storage_multiplier > 1.0）；
    /// `factor: f32` 表 "peak_at 时间被压缩到 1/factor"。
    AgeAccelerate { factor: f32 },
}

impl ContainerFreshnessBehavior {
    /// 是否为冻结类容器（决定 enter/exit 时是否记 frozen_since_tick）。
    pub fn is_freeze(&self) -> bool {
        matches!(self, ContainerFreshnessBehavior::Freeze)
    }
}
