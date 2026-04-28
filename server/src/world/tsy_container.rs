//! plan-tsy-container-v1 §1 — TSY 容器与搜刮的核心数据模型。
//!
//! 容器（`LootContainer`）为 Entity-scoped Component，挂在世界中的实体上；
//! 玩家搜刮中状态（`SearchProgress`）挂在玩家 Entity 上，互斥语义靠
//! `LootContainer.searched_by` 和玩家 `SearchProgress` 双向锁定。
//!
//! 本模块只定义数据结构 + 纯函数 helper（搜刮时长 / 钥匙映射），
//! system / event 在 `tsy_container_search.rs`。
//!
//! 字段级 `#[allow(dead_code)]`：`depth` / `locked` / `spawned_at_tick` /
//! `started_at_tick` 等是 IPC schema bridge / 未来 system（client HUD bridge /
//! lifecycle inspect / debug 命令）消费的元数据，运行时 system 暂不读，
//! 但已写入 ECS 状态。

#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use valence::prelude::{bevy_ecs, Component, Entity};

use crate::inventory::ItemInstance;
use crate::world::zone::TsyDepth;

/// 5 档容器类型（plan §0.2 / §十六.三）。决定 base_search_ticks / required_key /
/// is_skeleton 等运行时行为。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContainerKind {
    /// 普通：干尸
    DryCorpse,
    /// 普通：骨架
    Skeleton,
    /// 罕见：储物袋残骸
    StoragePouch,
    /// 史诗：石匣 / 玉棺（locked）
    StoneCasket,
    /// 传说：法阵核心（locked，= 骨架）
    RelicCore,
}

impl ContainerKind {
    /// 基础搜刮时长（tick，20 tps；见 plan §1.1 表）。
    pub const fn base_search_ticks(self) -> u32 {
        match self {
            Self::DryCorpse | Self::Skeleton => 80,
            Self::StoragePouch => 200,
            Self::StoneCasket => 400,
            Self::RelicCore => 600,
        }
    }

    /// 需要的钥匙类型（None = 不需要锁；见 plan §1.1 / §3）。
    pub const fn required_key(self) -> Option<KeyKind> {
        match self {
            Self::StoneCasket => Some(KeyKind::StoneCasketKey),
            Self::RelicCore => Some(KeyKind::ArrayCoreSigil),
            _ => None,
        }
    }

    /// 是否为骨架（搜空 → 发 RelicExtracted；见 plan §0.6）。
    pub const fn is_skeleton(self) -> bool {
        matches!(self, Self::RelicCore)
    }

    /// snake_case 字符串名（用于 schema / 日志 / loot_pool 配置 key 等）。
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DryCorpse => "dry_corpse",
            Self::Skeleton => "skeleton",
            Self::StoragePouch => "storage_pouch",
            Self::StoneCasket => "stone_casket",
            Self::RelicCore => "relic_core",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "dry_corpse" => Some(Self::DryCorpse),
            "skeleton" => Some(Self::Skeleton),
            "storage_pouch" => Some(Self::StoragePouch),
            "stone_casket" => Some(Self::StoneCasket),
            "relic_core" => Some(Self::RelicCore),
            _ => None,
        }
    }
}

/// 钥匙类型（plan §1.1 / §3.1）。每种钥匙对应一个 template_id；
/// 见 [`ItemInstance::as_container_key`]（在 `inventory/mod.rs` 内 impl）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeyKind {
    StoneCasketKey,
    JadeCoffinSeal,
    ArrayCoreSigil,
}

impl KeyKind {
    /// 对应钥匙物品的 `template_id`。
    pub const fn template_id(self) -> &'static str {
        match self {
            Self::StoneCasketKey => "key_stone_casket",
            Self::JadeCoffinSeal => "key_jade_coffin",
            Self::ArrayCoreSigil => "key_array_core",
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::StoneCasketKey => "stone_casket_key",
            Self::JadeCoffinSeal => "jade_coffin_seal",
            Self::ArrayCoreSigil => "array_core_sigil",
        }
    }
}

/// TSY 容器的运行时状态。挂在 Entity 上，Entity `Position` = 容器世界坐标。
#[derive(Component, Debug, Clone)]
pub struct LootContainer {
    pub kind: ContainerKind,
    /// TSY 家族 id，e.g. "tsy_lingxu_01"（不带 layer 后缀）。
    pub family_id: String,
    /// 所在 TSY 层深。
    pub depth: TsyDepth,
    /// 指向 `LootPoolRegistry` 的 key，搜刮完成时滚 loot 用。
    pub loot_pool_id: String,
    /// `Some` = 锁着，需要持有对应钥匙才能开搜。
    pub locked: Option<KeyKind>,
    /// 当前正在搜的玩家 Entity（互斥语义）。
    pub searched_by: Option<Entity>,
    /// 已被搜空（`depleted = true` 后不再可搜）。
    pub depleted: bool,
    pub spawned_at_tick: u64,
}

impl LootContainer {
    /// 构造一个初始未锁定的容器（locked 由 `kind.required_key()` 推导）。
    pub fn new(
        kind: ContainerKind,
        family_id: String,
        depth: TsyDepth,
        loot_pool_id: String,
        spawned_at_tick: u64,
    ) -> Self {
        Self {
            kind,
            family_id,
            depth,
            loot_pool_id,
            locked: kind.required_key(),
            searched_by: None,
            depleted: false,
            spawned_at_tick,
        }
    }
}

/// 玩家正在搜刮的状态，挂在**玩家 Entity** 上（不在容器上）。
/// 同时存在 `SearchProgress` 与该容器 `searched_by = Some(player)` 互锁。
#[derive(Component, Debug, Clone)]
pub struct SearchProgress {
    pub container: Entity,
    pub required_ticks: u32,
    pub elapsed_ticks: u32,
    pub started_at_tick: u64,
    /// 用于位置中断检测；偏移 > [`SEARCH_MOVE_INTERRUPT_THRESHOLD_M`] 视为中断。
    pub started_pos: [f64; 3],
    /// 已锁定要消耗的钥匙 instance_id（None = 不需要钥匙；完成时扣 1 stack）。
    pub key_item_instance_id: Option<u64>,
}

/// 搜刮中断的位移阈值（米）。plan §2.2 — 玩家位置偏移超过此值即中断。
pub const SEARCH_MOVE_INTERRUPT_THRESHOLD_M: f64 = 0.5;

/// 玩家与容器交互的最大距离（米）。plan §2.1 — 距离 > 3 block 拒绝开搜。
pub const SEARCH_INTERACT_RANGE_M: f64 = 3.0;

/// 真元抽取在搜刮期间的乘数。plan §0.3 / §2.3。
pub const SEARCH_DRAIN_MULTIPLIER: f64 = 1.5;

/// 给 `ItemInstance` 加 `as_container_key` 识别 helper（仅 server 内部；
/// 不上 inventory schema，因为钥匙是 template_id 约定，不是新字段）。
pub fn item_as_container_key(item: &ItemInstance) -> Option<KeyKind> {
    match item.template_id.as_str() {
        "key_stone_casket" => Some(KeyKind::StoneCasketKey),
        "key_jade_coffin" => Some(KeyKind::JadeCoffinSeal),
        "key_array_core" => Some(KeyKind::ArrayCoreSigil),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::ItemRarity;

    fn dummy_item(template_id: &str) -> ItemInstance {
        ItemInstance {
            instance_id: 1,
            template_id: template_id.to_string(),
            display_name: "test".to_string(),
            grid_w: 1,
            grid_h: 1,
            weight: 0.1,
            rarity: ItemRarity::Common,
            description: String::new(),
            stack_count: 1,
            spirit_quality: 0.0,
            durability: 1.0,
            freshness: None,
            mineral_id: None,
            charges: None,
            forge_quality: None,
            forge_color: None,
            forge_side_effects: Vec::new(),
            forge_achieved_tier: None,
        }
    }

    #[test]
    fn base_search_ticks_table() {
        // plan §1.1 表：4 / 4 / 10 / 20 / 30 秒（@20 tps）。
        assert_eq!(ContainerKind::DryCorpse.base_search_ticks(), 80);
        assert_eq!(ContainerKind::Skeleton.base_search_ticks(), 80);
        assert_eq!(ContainerKind::StoragePouch.base_search_ticks(), 200);
        assert_eq!(ContainerKind::StoneCasket.base_search_ticks(), 400);
        assert_eq!(ContainerKind::RelicCore.base_search_ticks(), 600);
    }

    #[test]
    fn required_key_mapping() {
        assert_eq!(ContainerKind::DryCorpse.required_key(), None);
        assert_eq!(ContainerKind::Skeleton.required_key(), None);
        assert_eq!(ContainerKind::StoragePouch.required_key(), None);
        assert_eq!(
            ContainerKind::StoneCasket.required_key(),
            Some(KeyKind::StoneCasketKey)
        );
        assert_eq!(
            ContainerKind::RelicCore.required_key(),
            Some(KeyKind::ArrayCoreSigil)
        );
    }

    #[test]
    fn is_skeleton_only_relic_core() {
        assert!(!ContainerKind::DryCorpse.is_skeleton());
        assert!(!ContainerKind::Skeleton.is_skeleton());
        assert!(!ContainerKind::StoragePouch.is_skeleton());
        assert!(!ContainerKind::StoneCasket.is_skeleton());
        assert!(ContainerKind::RelicCore.is_skeleton());
    }

    #[test]
    fn kind_str_roundtrip() {
        for kind in [
            ContainerKind::DryCorpse,
            ContainerKind::Skeleton,
            ContainerKind::StoragePouch,
            ContainerKind::StoneCasket,
            ContainerKind::RelicCore,
        ] {
            assert_eq!(ContainerKind::from_str(kind.as_str()), Some(kind));
        }
        assert!(ContainerKind::from_str("invalid").is_none());
    }

    #[test]
    fn key_kind_template_id_table() {
        assert_eq!(KeyKind::StoneCasketKey.template_id(), "key_stone_casket");
        assert_eq!(KeyKind::JadeCoffinSeal.template_id(), "key_jade_coffin");
        assert_eq!(KeyKind::ArrayCoreSigil.template_id(), "key_array_core");
    }

    #[test]
    fn item_as_container_key_recognises_three_kinds() {
        assert_eq!(
            item_as_container_key(&dummy_item("key_stone_casket")),
            Some(KeyKind::StoneCasketKey)
        );
        assert_eq!(
            item_as_container_key(&dummy_item("key_jade_coffin")),
            Some(KeyKind::JadeCoffinSeal)
        );
        assert_eq!(
            item_as_container_key(&dummy_item("key_array_core")),
            Some(KeyKind::ArrayCoreSigil)
        );
        assert_eq!(item_as_container_key(&dummy_item("iron_sword")), None);
    }

    #[test]
    fn loot_container_new_locks_per_kind() {
        let lc = LootContainer::new(
            ContainerKind::DryCorpse,
            "tsy_lingxu_01".to_string(),
            TsyDepth::Shallow,
            "dry_corpse_shallow_common".to_string(),
            42,
        );
        assert!(lc.locked.is_none());
        assert!(!lc.depleted);
        assert!(lc.searched_by.is_none());

        let stone = LootContainer::new(
            ContainerKind::StoneCasket,
            "tsy_lingxu_01".to_string(),
            TsyDepth::Mid,
            "stone_casket_mid".to_string(),
            42,
        );
        assert_eq!(stone.locked, Some(KeyKind::StoneCasketKey));

        let core = LootContainer::new(
            ContainerKind::RelicCore,
            "tsy_lingxu_01".to_string(),
            TsyDepth::Deep,
            "relic_core_deep".to_string(),
            42,
        );
        assert_eq!(core.locked, Some(KeyKind::ArrayCoreSigil));
    }

    #[test]
    fn key_kind_round_trip_via_template() {
        for kk in [
            KeyKind::StoneCasketKey,
            KeyKind::JadeCoffinSeal,
            KeyKind::ArrayCoreSigil,
        ] {
            assert_eq!(
                item_as_container_key(&dummy_item(kk.template_id())),
                Some(kk)
            );
        }
    }
}
