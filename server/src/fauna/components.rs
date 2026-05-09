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
    /// 飞行中立巨型生物（feat/flying-whale 开发中）。客户端用 GeckoLib 自定义
    /// 模型 `bong:whale` 渲染，server 走自定义 EntityKind（详见
    /// `npc::spawn_whale::WHALE_ENTITY_KIND`）。
    Whale,
}

impl BeastKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Rat => "rat",
            Self::Spider => "spider",
            Self::HybridBeast => "hybrid_beast",
            Self::VoidDistorted => "void_distorted",
            Self::Whale => "whale",
        }
    }
}

pub fn fauna_tag_for_beast_spawn(home_zone: &str, seed: u64) -> FaunaTag {
    FaunaTag::new(beast_kind_for_spawn_context(home_zone, seed))
}

pub fn fauna_spawn_seed(home_zone: &str, x: f64, z: f64) -> u64 {
    let mut seed = 0xFA17_A5EED_u64;
    for byte in home_zone.bytes() {
        seed = seed.wrapping_mul(0x100_0000_01B3).wrapping_add(byte as u64);
    }
    seed ^ x.to_bits().rotate_left(17) ^ z.to_bits().rotate_right(11)
}

pub fn beast_kind_for_spawn_context(home_zone: &str, seed: u64) -> BeastKind {
    let zone = home_zone.to_ascii_lowercase();
    if zone.contains("spider") || zone.contains("zhu") || zone.contains("蛛") {
        return BeastKind::Spider;
    }
    if zone.contains("hybrid")
        || zone.contains("feng_he")
        || zone.contains("fenghe")
        || zone.contains("缝合")
        || zone.contains("异变")
    {
        return BeastKind::HybridBeast;
    }
    if zone.contains("rat") || zone.contains("shu") || zone.contains("鼠") {
        return BeastKind::Rat;
    }

    match seed % 20 {
        0 => BeastKind::HybridBeast,
        1..=5 => BeastKind::Spider,
        _ => BeastKind::Rat,
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
