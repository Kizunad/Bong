//! 妖兽域 ECS component 与正典枚举（plan-fauna-v1 §6）。

use serde::{Deserialize, Serialize};
use valence::prelude::{bevy_ecs, Component};

/// 妖兽种类。命名保留 gameplay 语义，不绑定具体 Minecraft entity skin。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BeastKind {
    // 醒灵级
    Rat,
    // 引气级
    Spider,
    GreenSpider,
    JungleScorpion,
    CockadeSnake,
    // 凝脉级
    BlueSpider,
    IceScorpion,
    MandrakeSnake,
    // 固元级
    HybridBeast,
    VoidDistorted,
    DarkTiger,
    // 通灵+ Boss
    LivingPillar,
    // 化虚级
    PoisonDragon,
    BoneDragon,
    Whale,
}

impl BeastKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Rat => "rat",
            Self::Spider => "spider",
            Self::GreenSpider => "green_spider",
            Self::JungleScorpion => "jungle_scorpion",
            Self::CockadeSnake => "cockade_snake",
            Self::BlueSpider => "blue_spider",
            Self::IceScorpion => "ice_scorpion",
            Self::MandrakeSnake => "mandrake_snake",
            Self::HybridBeast => "hybrid_beast",
            Self::VoidDistorted => "void_distorted",
            Self::DarkTiger => "dark_tiger",
            Self::LivingPillar => "living_pillar",
            Self::PoisonDragon => "poison_dragon",
            Self::BoneDragon => "bone_dragon",
            Self::Whale => "whale",
        }
    }

    pub const fn health_max(self) -> f32 {
        match self {
            Self::Rat => 8.0,
            Self::Spider => 25.0,
            Self::GreenSpider => 30.0,
            Self::JungleScorpion => 35.0,
            Self::CockadeSnake => 22.0,
            Self::BlueSpider => 55.0,
            Self::IceScorpion => 70.0,
            Self::MandrakeSnake => 50.0,
            Self::HybridBeast => 400.0,
            Self::VoidDistorted => 400.0,
            Self::DarkTiger => 500.0,
            Self::LivingPillar => 2500.0,
            Self::PoisonDragon => 8000.0,
            Self::BoneDragon => 9000.0,
            Self::Whale => 10000.0,
        }
    }

    pub const fn realm_tier(self) -> u8 {
        match self {
            Self::Rat => 0,
            Self::Spider | Self::GreenSpider | Self::JungleScorpion | Self::CockadeSnake => 1,
            Self::BlueSpider | Self::IceScorpion | Self::MandrakeSnake => 2,
            Self::HybridBeast | Self::VoidDistorted | Self::DarkTiger => 3,
            Self::LivingPillar => 4,
            Self::PoisonDragon | Self::BoneDragon | Self::Whale => 5,
        }
    }

    pub const fn is_terrestrial(self) -> bool {
        matches!(
            self,
            Self::Rat
                | Self::Spider
                | Self::GreenSpider
                | Self::JungleScorpion
                | Self::CockadeSnake
                | Self::BlueSpider
                | Self::IceScorpion
                | Self::MandrakeSnake
                | Self::HybridBeast
                | Self::VoidDistorted
                | Self::DarkTiger
        )
    }

    pub const ALL_TERRESTRIAL: &'static [BeastKind] = &[
        Self::Rat,
        Self::Spider,
        Self::GreenSpider,
        Self::JungleScorpion,
        Self::CockadeSnake,
        Self::BlueSpider,
        Self::IceScorpion,
        Self::MandrakeSnake,
        Self::HybridBeast,
        Self::VoidDistorted,
        Self::DarkTiger,
    ];
}

pub fn is_prey_of(prey: BeastKind, predator: BeastKind) -> bool {
    prey.is_terrestrial() && predator.is_terrestrial() && predator.realm_tier() > prey.realm_tier()
}

pub fn fauna_tag_for_beast_spawn(home_zone: &str, seed: u64) -> FaunaTag {
    FaunaTag::new(beast_kind_for_spawn_context(home_zone, seed, None))
}

pub fn fauna_tag_for_beast_spawn_with_qi(home_zone: &str, seed: u64, zone_qi: f64) -> FaunaTag {
    FaunaTag::new(beast_kind_for_spawn_context(home_zone, seed, Some(zone_qi)))
}

pub fn fauna_spawn_seed(home_zone: &str, x: f64, z: f64) -> u64 {
    let mut seed = 0xFA17_A5EED_u64;
    for byte in home_zone.bytes() {
        seed = seed.wrapping_mul(0x100_0000_01B3).wrapping_add(byte as u64);
    }
    seed ^ x.to_bits().rotate_left(17) ^ z.to_bits().rotate_right(11)
}

struct SpawnWeight {
    kind: BeastKind,
    weight: u32,
}

const SPAWN_POOL_DEAD_EDGE: &[SpawnWeight] = &[
    SpawnWeight { kind: BeastKind::Rat, weight: 50 },
    SpawnWeight { kind: BeastKind::Spider, weight: 30 },
    SpawnWeight { kind: BeastKind::JungleScorpion, weight: 15 },
    SpawnWeight { kind: BeastKind::CockadeSnake, weight: 5 },
];

const SPAWN_POOL_LOW_QI: &[SpawnWeight] = &[
    SpawnWeight { kind: BeastKind::Rat, weight: 40 },
    SpawnWeight { kind: BeastKind::Spider, weight: 15 },
    SpawnWeight { kind: BeastKind::JungleScorpion, weight: 15 },
    SpawnWeight { kind: BeastKind::CockadeSnake, weight: 15 },
    SpawnWeight { kind: BeastKind::GreenSpider, weight: 15 },
];

const SPAWN_POOL_MID_QI: &[SpawnWeight] = &[
    SpawnWeight { kind: BeastKind::JungleScorpion, weight: 20 },
    SpawnWeight { kind: BeastKind::CockadeSnake, weight: 18 },
    SpawnWeight { kind: BeastKind::GreenSpider, weight: 18 },
    SpawnWeight { kind: BeastKind::IceScorpion, weight: 12 },
    SpawnWeight { kind: BeastKind::MandrakeSnake, weight: 10 },
    SpawnWeight { kind: BeastKind::Rat, weight: 8 },
    SpawnWeight { kind: BeastKind::HybridBeast, weight: 8 },
    SpawnWeight { kind: BeastKind::DarkTiger, weight: 3 },
    SpawnWeight { kind: BeastKind::BlueSpider, weight: 3 },
];

const SPAWN_POOL_HIGH_QI: &[SpawnWeight] = &[
    SpawnWeight { kind: BeastKind::IceScorpion, weight: 20 },
    SpawnWeight { kind: BeastKind::MandrakeSnake, weight: 18 },
    SpawnWeight { kind: BeastKind::BlueSpider, weight: 15 },
    SpawnWeight { kind: BeastKind::DarkTiger, weight: 15 },
    SpawnWeight { kind: BeastKind::HybridBeast, weight: 15 },
    SpawnWeight { kind: BeastKind::GreenSpider, weight: 7 },
    SpawnWeight { kind: BeastKind::JungleScorpion, weight: 5 },
    SpawnWeight { kind: BeastKind::CockadeSnake, weight: 5 },
];

const SPAWN_POOL_PEAK_QI: &[SpawnWeight] = &[
    SpawnWeight { kind: BeastKind::DarkTiger, weight: 30 },
    SpawnWeight { kind: BeastKind::HybridBeast, weight: 25 },
    SpawnWeight { kind: BeastKind::IceScorpion, weight: 18 },
    SpawnWeight { kind: BeastKind::MandrakeSnake, weight: 15 },
    SpawnWeight { kind: BeastKind::BlueSpider, weight: 12 },
];

fn pick_from_pool(pool: &[SpawnWeight], seed: u64) -> BeastKind {
    let total: u64 = pool.iter().map(|w| w.weight as u64).sum();
    if total == 0 {
        return BeastKind::Rat;
    }
    let roll = seed % total;
    let mut acc = 0u64;
    for entry in pool {
        acc += entry.weight as u64;
        if roll < acc {
            return entry.kind;
        }
    }
    pool.last().map(|e| e.kind).unwrap_or(BeastKind::Rat)
}

pub fn beast_kind_for_spawn_context(home_zone: &str, seed: u64, zone_qi: Option<f64>) -> BeastKind {
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
    if zone.contains("tiger") || zone.contains("hu") || zone.contains("虎") {
        return BeastKind::DarkTiger;
    }
    if zone.contains("scorpion") || zone.contains("xie") || zone.contains("蝎") {
        return BeastKind::JungleScorpion;
    }
    if zone.contains("snake") || zone.contains("she") || zone.contains("蛇") {
        return BeastKind::CockadeSnake;
    }

    let qi = zone_qi.unwrap_or(0.3);
    let pool = if qi < 0.15 {
        SPAWN_POOL_DEAD_EDGE
    } else if qi < 0.35 {
        SPAWN_POOL_LOW_QI
    } else if qi < 0.55 {
        SPAWN_POOL_MID_QI
    } else if qi < 0.80 {
        SPAWN_POOL_HIGH_QI
    } else {
        SPAWN_POOL_PEAK_QI
    };
    pick_from_pool(pool, seed)
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
