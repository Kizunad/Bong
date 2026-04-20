//! plan-lingtian-v1 §1.2.1 — 锄头三档。
//!
//! 与 inventory 解耦：`HoeKind` 仅描述 lingtian 视角下的耐久 / 用途。
//! 实际 item 在 `assets/items/lingtian.toml` 注册为 `Misc` 类。

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HoeKind {
    Iron,
    Lingtie,
    Xuantie,
}

impl HoeKind {
    /// 由 inventory item id 反查锄档。非锄返回 None。
    pub fn from_item_id(id: &str) -> Option<Self> {
        match id {
            "hoe_iron" => Some(Self::Iron),
            "hoe_lingtie" => Some(Self::Lingtie),
            "hoe_xuantie" => Some(Self::Xuantie),
            _ => None,
        }
    }

    pub fn item_id(self) -> &'static str {
        match self {
            Self::Iron => "hoe_iron",
            Self::Lingtie => "hoe_lingtie",
            Self::Xuantie => "hoe_xuantie",
        }
    }

    /// plan §1.2.1 — 总可用次数。
    pub fn uses_max(self) -> u32 {
        match self {
            Self::Iron => 20,
            Self::Lingtie => 50,
            Self::Xuantie => 100,
        }
    }

    /// 单次操作（开垦 / 翻新）耗费的归一化耐久量。
    pub fn use_durability_cost(self) -> f64 {
        1.0 / self.uses_max() as f64
    }
}

impl fmt::Display for HoeKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.item_id())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_known_ids() {
        for kind in [HoeKind::Iron, HoeKind::Lingtie, HoeKind::Xuantie] {
            let id = kind.item_id();
            assert_eq!(HoeKind::from_item_id(id), Some(kind));
        }
    }

    #[test]
    fn unknown_id_is_none() {
        assert!(HoeKind::from_item_id("rusted_blade").is_none());
        assert!(HoeKind::from_item_id("").is_none());
    }

    #[test]
    fn cost_matches_uses_max() {
        assert!((HoeKind::Iron.use_durability_cost() - 0.05).abs() < 1e-9);
        assert!((HoeKind::Lingtie.use_durability_cost() - 0.02).abs() < 1e-9);
        assert!((HoeKind::Xuantie.use_durability_cost() - 0.01).abs() < 1e-9);
    }
}
