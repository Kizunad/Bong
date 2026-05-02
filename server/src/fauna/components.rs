//! 妖兽域 ECS component 与正典枚举（plan-fauna-v1 §6）。

use serde::{Deserialize, Serialize};
use valence::prelude::{bevy_ecs, Component};

/// 妖兽种类。命名保留 gameplay 语义，不绑定具体 Minecraft entity skin。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BeastKind {
    Rat,
    Spider,
    HybridBeast,
    VoidDistorted,
}

impl BeastKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Rat => "rat",
            Self::Spider => "spider",
            Self::HybridBeast => "hybrid_beast",
            Self::VoidDistorted => "void_distorted",
        }
    }
}

/// 同种妖兽的变体。当前只修饰稀有掉落率，不改变保底骨材。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BeastVariant {
    #[default]
    Normal,
    Thunder,
    Tainted,
}

impl BeastVariant {
    pub const fn rare_drop_multiplier(self) -> f32 {
        match self {
            Self::Normal => 1.0,
            Self::Thunder => 1.25,
            Self::Tainted => 1.5,
        }
    }
}

/// 挂在 NPC / hostile entity 上的妖兽标签。死亡掉落只信这个 component；
/// 未显式标记的旧 Beast/Fuya NPC 会由 drop 系统做兼容 fallback。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Component, Serialize, Deserialize)]
pub struct FaunaTag {
    pub beast_kind: BeastKind,
    pub variant: BeastVariant,
}

impl FaunaTag {
    pub const fn new(beast_kind: BeastKind) -> Self {
        Self {
            beast_kind,
            variant: BeastVariant::Normal,
        }
    }

    pub const fn with_variant(beast_kind: BeastKind, variant: BeastVariant) -> Self {
        Self {
            beast_kind,
            variant,
        }
    }
}

/// 避免同一 DeathEvent / lifecycle 重入重复吐掉落。
#[derive(Debug, Clone, Copy, Default, Component)]
pub struct FaunaDropIssued;
