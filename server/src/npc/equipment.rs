//! NPC 装备模型（plan-npc-combat-gear-v1 §P0）。
//!
//! 与玩家装备系统（`PlayerInventory` + `Weapon` component）解耦：NPC 不走
//! 背包/容器/重量系统，仅存储 6 个 slot 的静态装备状态。spawn 时由
//! `assign_npc_equipment()` 一次性生成，战斗系统直接读 `main_hand` 做
//! 武器结算、读 armor slots 做减伤。
//!
//! NPC 战斗中不消耗耐久——`durability_ratio` 仅影响掉落品初始耐久和
//! 战斗加成系数 `npc_weapon_damage_multiplier()`。

use valence::prelude::{bevy_ecs, Component, ItemKind};

use crate::combat::components::WoundKind;
use crate::combat::events::{AttackReach, FIST_REACH, SPEAR_REACH, SWORD_REACH};
use crate::combat::weapon::WeaponKind;
use crate::cultivation::components::Realm;
use crate::npc::lifecycle::NpcArchetype;

// ─── splitmix64 ────────────────────────────────────────────────────────────

/// Deterministic PRNG 基于 entity_id hash，确保相同 entity 的装备分配结果一致。
fn splitmix64(seed: u64) -> u64 {
    let mut x = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

/// 从 seed 生成 [0.0, 1.0) 的 f32。
fn splitmix64_unit(seed: u64) -> f32 {
    let bits = ((splitmix64(seed) >> 40) & 0x00FF_FFFF) as u32;
    bits as f32 / (1u32 << 24) as f32
}

/// 从 seed 生成 [0, n) 的整数。
fn splitmix64_range(seed: u64, n: u32) -> u32 {
    (splitmix64_unit(seed) * n as f32) as u32 % n
}

// ─── realm helpers ─────────────────────────────────────────────────────────

fn realm_rank(realm: Realm) -> u8 {
    match realm {
        Realm::Awaken => 0,
        Realm::Induce => 1,
        Realm::Condense => 2,
        Realm::Solidify => 3,
        Realm::Spirit => 4,
        Realm::Void => 5,
    }
}

// ─── NpcEquipSlot ──────────────────────────────────────────────────────────

/// 单个装备 slot 的属性。
#[derive(Clone, Debug, PartialEq)]
pub struct NpcEquipSlot {
    pub template_id: String,
    pub display_name: String,
    pub item_kind: ItemKind,
    /// 0=凡铁 1=灵器 2=法宝（对齐 plan-weapon-v1 §2 四档；3=仙器保留但不分配）。
    pub quality_tier: u8,
    /// 基础攻击力（用于 attack_multiplier = max(1.0, base_attack / 10.0)）。
    pub base_attack: f32,
    pub weapon_kind: Option<WeaponKind>,
    pub armor_profile_id: Option<String>,
    /// 0.0-1.0 耐久比例。NPC 不消耗耐久，仅影响战斗加成和掉落品初始耐久。
    pub durability_ratio: f32,
}

// ─── NpcEquipment ──────────────────────────────────────────────────────────

/// NPC 装备 component。6 slot，spawn 时一次性生成，战斗系统直接读取。
#[derive(Clone, Debug, Default, Component, PartialEq)]
pub struct NpcEquipment {
    pub main_hand: Option<NpcEquipSlot>,
    pub off_hand: Option<NpcEquipSlot>,
    pub head: Option<NpcEquipSlot>,
    pub chest: Option<NpcEquipSlot>,
    pub legs: Option<NpcEquipSlot>,
    pub feet: Option<NpcEquipSlot>,
}

impl NpcEquipment {
    /// 迭代所有非空 slot 及其名称。
    pub fn iter_slots(&self) -> impl Iterator<Item = (&str, &NpcEquipSlot)> {
        [
            ("main_hand", &self.main_hand),
            ("off_hand", &self.off_hand),
            ("head", &self.head),
            ("chest", &self.chest),
            ("legs", &self.legs),
            ("feet", &self.feet),
        ]
        .into_iter()
        .filter_map(|(name, slot)| slot.as_ref().map(|s| (name, s)))
    }

    /// 所有非空 armor slot（head/chest/legs/feet）。
    pub fn armor_slots(&self) -> impl Iterator<Item = &NpcEquipSlot> {
        [&self.head, &self.chest, &self.legs, &self.feet]
            .into_iter()
            .filter_map(|s| s.as_ref())
    }

    pub fn total_quality_score(&self) -> f32 {
        self.iter_slots()
            .map(|(_, slot)| {
                let tier_score = (slot.quality_tier as f32 + 1.0) * 3.0;
                let durability_bonus = slot.durability_ratio.clamp(0.0, 1.0) * 2.0;
                tier_score + durability_bonus
            })
            .sum()
    }
}

// ─── Damage multiplier ────────────────────────────────────────────────────

/// plan §8.1 #1: quality_multiplier 表，对齐 `Weapon::quality_multiplier()`。
fn quality_multiplier(quality_tier: u8) -> f32 {
    match quality_tier {
        0 => 1.00,
        1 => 1.15,
        2 => 1.35,
        _ => 1.60,
    }
}

/// 从 NpcEquipSlot 计算 NPC 武器伤害乘数。
///
/// 公式与 `Weapon::damage_multiplier()` 对齐：
/// `max(1.0, base_attack / 10.0) x quality_multiplier(quality_tier) x (0.5 + 0.5 * durability_ratio)`
pub fn npc_weapon_damage_multiplier(slot: &NpcEquipSlot) -> f32 {
    let attack_mul = (slot.base_attack / 10.0).max(1.0);
    let quality_mul = quality_multiplier(slot.quality_tier);
    let durability_combat_mul = 0.5 + 0.5 * slot.durability_ratio.clamp(0.0, 1.0);
    attack_mul * quality_mul * durability_combat_mul
}

/// 从 WeaponKind 派生 AttackReach。
pub fn reach_for_weapon_kind(kind: WeaponKind) -> AttackReach {
    match kind {
        WeaponKind::Sword | WeaponKind::Saber => SWORD_REACH,
        WeaponKind::Staff => SPEAR_REACH,
        WeaponKind::Spear => SPEAR_REACH,
        WeaponKind::Fist => FIST_REACH,
        WeaponKind::Dagger => AttackReach::new(1.2, 0.4),
        WeaponKind::Bow => SWORD_REACH, // 近战退化为剑距
    }
}

/// 从 WeaponKind 派生 WoundKind。
pub fn wound_kind_for_weapon(kind: WeaponKind) -> WoundKind {
    match kind {
        WeaponKind::Sword | WeaponKind::Saber | WeaponKind::Dagger => WoundKind::Cut,
        WeaponKind::Staff | WeaponKind::Fist => WoundKind::Blunt,
        WeaponKind::Spear | WeaponKind::Bow => WoundKind::Pierce,
    }
}

// ─── ItemKind mapping ──────────────────────────────────────────────────────

/// quality_tier → MC 主手武器 ItemKind 映射。
pub fn weapon_item_kind_for_tier(quality_tier: u8) -> ItemKind {
    match quality_tier {
        0 => ItemKind::IronSword,
        1 => ItemKind::DiamondSword,
        2 => ItemKind::GoldenSword,
        _ => ItemKind::GoldenSword,
    }
}

/// quality_tier → MC 护甲 ItemKind 映射（按 slot）。
pub fn armor_item_kind_for_tier(slot_name: &str, quality_tier: u8) -> ItemKind {
    match (slot_name, quality_tier) {
        ("head", 0) => ItemKind::LeatherHelmet,
        ("head", 1) => ItemKind::ChainmailHelmet,
        ("head", _) => ItemKind::IronHelmet,
        ("chest", 0) => ItemKind::LeatherChestplate,
        ("chest", 1) => ItemKind::ChainmailChestplate,
        ("chest", _) => ItemKind::IronChestplate,
        ("legs", 0) => ItemKind::LeatherLeggings,
        ("legs", 1) => ItemKind::ChainmailLeggings,
        ("legs", _) => ItemKind::IronLeggings,
        ("feet", 0) => ItemKind::LeatherBoots,
        ("feet", 1) => ItemKind::ChainmailBoots,
        ("feet", _) => ItemKind::IronBoots,
        _ => ItemKind::LeatherChestplate,
    }
}

// ─── Equipment drop ────────────────────────────────────────────────────────

use crate::npc::loot::RolledLoot;

/// NPC 死亡时按装备生成掉落。每个非空 slot 独立 30% 概率掉落。
///
/// 返回的 `Vec<RolledLoot>` 由调用方追加到现有 loot table roll 结果中。
pub fn roll_equipment_drops(equipment: &NpcEquipment, seed: u64) -> Vec<RolledLoot> {
    const DROP_CHANCE: f32 = 0.30;
    let mut out = Vec::new();
    for (idx, (_name, slot)) in equipment.iter_slots().enumerate() {
        let roll_seed = seed
            .wrapping_mul(0x9E37_79B9_7F4A_7C15)
            .wrapping_add((idx as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9));
        let roll = splitmix64_unit(roll_seed);
        if roll < DROP_CHANCE {
            out.push(RolledLoot {
                template_id: slot.template_id.clone(),
                stack: 1,
            });
        }
    }
    out
}

/// 武器掉落品的耐久 = slot.durability_ratio * 0.8（战损 20%）。
pub fn weapon_drop_durability(slot: &NpcEquipSlot) -> f32 {
    (slot.durability_ratio * 0.8).clamp(0.0, 1.0)
}

/// 护甲掉落品的耐久 = slot.durability_ratio * 0.7（战损 30%）。
pub fn armor_drop_durability(slot: &NpcEquipSlot) -> f32 {
    (slot.durability_ratio * 0.7).clamp(0.0, 1.0)
}

// ─── Spawn-time assignment ─────────────────────────────────────────────────

/// 简易 slot 构造 helper。
fn make_weapon_slot(
    template_id: &str,
    display_name: &str,
    item_kind: ItemKind,
    weapon_kind: WeaponKind,
    quality_tier: u8,
    base_attack: f32,
    durability_ratio: f32,
) -> NpcEquipSlot {
    NpcEquipSlot {
        template_id: template_id.to_string(),
        display_name: display_name.to_string(),
        item_kind,
        quality_tier,
        base_attack,
        weapon_kind: Some(weapon_kind),
        armor_profile_id: None,
        durability_ratio,
    }
}

fn make_armor_slot(
    template_id: &str,
    display_name: &str,
    item_kind: ItemKind,
    armor_profile_id: &str,
    quality_tier: u8,
    durability_ratio: f32,
) -> NpcEquipSlot {
    NpcEquipSlot {
        template_id: template_id.to_string(),
        display_name: display_name.to_string(),
        item_kind,
        quality_tier,
        base_attack: 0.0,
        weapon_kind: None,
        armor_profile_id: Some(armor_profile_id.to_string()),
        durability_ratio,
    }
}

/// 根据 archetype / realm / faction 分配 NPC 装备（spawn 时调用）。
///
/// `entity_seed` 由 `splitmix64(entity_id_hash)` 生成，确保 deterministic。
/// 穷尽所有 `NpcArchetype` 变体。
pub fn assign_npc_equipment(
    archetype: NpcArchetype,
    realm: Realm,
    _faction: Option<crate::npc::faction::FactionId>,
    entity_seed: u64,
) -> NpcEquipment {
    let rank = realm_rank(realm);
    // realm >= Condense(2) 时 quality_tier +1，上限 2（法宝）
    let tier_bonus: u8 = if rank >= 2 { 1 } else { 0 };
    // realm >= Solidify(3) 时护甲件数 +1
    let armor_bonus: u32 = if rank >= 3 { 1 } else { 0 };

    match archetype {
        NpcArchetype::Rogue => assign_rogue(entity_seed, tier_bonus, armor_bonus),
        NpcArchetype::Commoner => assign_commoner(entity_seed),
        NpcArchetype::Disciple => assign_disciple(entity_seed, _faction, tier_bonus, armor_bonus),
        NpcArchetype::GuardianRelic => assign_guardian_relic(entity_seed, armor_bonus),
        NpcArchetype::Daoxiang => assign_daoxiang(entity_seed),
        NpcArchetype::Zhinian => assign_zhinian(entity_seed, tier_bonus),
        // 天生武器 / 无装备的 archetypes
        NpcArchetype::Beast
        | NpcArchetype::Fuya
        | NpcArchetype::SkullFiend
        | NpcArchetype::Zombie
        // plan-dying-elder-v1：垂死大能无装备（修士本体，无武器 / 甲）
        | NpcArchetype::DyingElder => NpcEquipment::default(),
    }
}

#[allow(clippy::field_reassign_with_default)]
fn assign_rogue(seed: u64, tier_bonus: u8, armor_bonus: u32) -> NpcEquipment {
    let mut eq = NpcEquipment::default();
    let roll = splitmix64_unit(seed);
    let tier = tier_bonus.min(2);
    if roll < 0.50 {
        // 50% 铁剑
        eq.main_hand = Some(make_weapon_slot(
            "iron_sword",
            "铁剑",
            weapon_item_kind_for_tier(tier),
            WeaponKind::Sword,
            tier,
            8.0,
            0.8 + splitmix64_unit(seed.wrapping_add(1)) * 0.2,
        ));
    } else if roll < 0.80 {
        // 30% 木杖
        eq.main_hand = Some(make_weapon_slot(
            "wooden_staff",
            "木杖",
            ItemKind::Stick,
            WeaponKind::Staff,
            tier,
            5.0,
            0.9,
        ));
    }
    // else 20% 空手

    // 0-2 件随机护甲，件数 += armor_bonus
    let base_armor_count = splitmix64_range(seed.wrapping_add(10), 3); // 0,1,2
    let armor_count = (base_armor_count + armor_bonus).min(4);
    assign_random_armor(&mut eq, seed.wrapping_add(100), armor_count, tier);

    eq
}

#[allow(clippy::field_reassign_with_default)]
fn assign_commoner(seed: u64) -> NpcEquipment {
    let mut eq = NpcEquipment::default();
    let roll = splitmix64_unit(seed);
    if roll < 0.20 {
        // 20% 锄头（凡器）
        eq.main_hand = Some(make_weapon_slot(
            "wooden_hoe",
            "锄头",
            ItemKind::WoodenHoe,
            WeaponKind::Staff, // 长柄走 staff 动作
            0,
            3.0,
            0.7,
        ));
    }
    // 80% 空手

    // 空或草衣 (30% 有草衣)
    if splitmix64_unit(seed.wrapping_add(20)) < 0.30 {
        eq.chest = Some(make_armor_slot(
            "straw_vest",
            "草衣",
            ItemKind::LeatherChestplate,
            "armor_straw_vest",
            0,
            0.5,
        ));
    }

    eq
}

#[allow(clippy::field_reassign_with_default)]
fn assign_disciple(
    seed: u64,
    faction: Option<crate::npc::faction::FactionId>,
    tier_bonus: u8,
    armor_bonus: u32,
) -> NpcEquipment {
    use crate::npc::faction::FactionId;
    let mut eq = NpcEquipment::default();
    let tier = tier_bonus.min(2);

    match faction {
        Some(FactionId::Defend) => {
            // 铁盾 + 木杖
            eq.main_hand = Some(make_weapon_slot(
                "wooden_staff",
                "木杖",
                ItemKind::Stick,
                WeaponKind::Staff,
                tier,
                6.0,
                0.9,
            ));
            eq.off_hand = Some(make_weapon_slot(
                "iron_shield",
                "铁盾",
                ItemKind::Shield,
                WeaponKind::Fist, // 盾牌无独立 WeaponKind，用 Fist 占位
                0,
                0.0,
                0.9,
            ));
        }
        _ => {
            // Attack / Neutral → 铁剑
            eq.main_hand = Some(make_weapon_slot(
                "iron_sword",
                "铁剑",
                weapon_item_kind_for_tier(tier),
                WeaponKind::Sword,
                tier,
                10.0,
                0.85 + splitmix64_unit(seed.wrapping_add(1)) * 0.15,
            ));
        }
    }

    // 1-3 件护甲 + bonus
    let base_armor_count = 1 + splitmix64_range(seed.wrapping_add(10), 3); // 1,2,3
    let armor_count = (base_armor_count + armor_bonus).min(4);
    assign_random_armor(&mut eq, seed.wrapping_add(100), armor_count, tier);

    eq
}

#[allow(clippy::field_reassign_with_default)]
fn assign_guardian_relic(seed: u64, armor_bonus: u32) -> NpcEquipment {
    let mut eq = NpcEquipment::default();
    // 古剑 (quality_tier=2, 法宝)
    eq.main_hand = Some(make_weapon_slot(
        "ancient_sword",
        "古剑",
        ItemKind::GoldenSword,
        WeaponKind::Sword,
        2,
        16.0,
        0.9,
    ));

    // 2-4 件高品质护甲 + bonus
    let base_armor_count = 2 + splitmix64_range(seed.wrapping_add(10), 3).min(2); // 2,3,4
    let armor_count = (base_armor_count + armor_bonus).min(4);
    assign_random_armor(&mut eq, seed.wrapping_add(100), armor_count, 2);

    eq
}

#[allow(clippy::field_reassign_with_default)]
fn assign_daoxiang(seed: u64) -> NpcEquipment {
    let mut eq = NpcEquipment::default();
    // 锈剑 (durability_ratio=0.3)
    eq.main_hand = Some(make_weapon_slot(
        "rusted_sword",
        "锈剑",
        ItemKind::IronSword,
        WeaponKind::Sword,
        0,
        7.0,
        0.3,
    ));

    // 0-1 件残甲
    if splitmix64_unit(seed.wrapping_add(10)) < 0.5 {
        eq.chest = Some(make_armor_slot(
            "tattered_armor",
            "残甲",
            ItemKind::LeatherChestplate,
            "armor_tattered",
            0,
            0.2,
        ));
    }

    eq
}

#[allow(clippy::field_reassign_with_default)]
fn assign_zhinian(seed: u64, tier_bonus: u8) -> NpcEquipment {
    let mut eq = NpcEquipment::default();
    let tier = tier_bonus.min(2);

    // 按生前流派 —— 这里简化为随机武器种类
    let weapon_roll = splitmix64_range(seed, 3);
    let (template_id, display_name, weapon_kind, base_attack) = match weapon_roll {
        0 => ("zhinian_sword", "执念残剑", WeaponKind::Sword, 10.0),
        1 => ("zhinian_saber", "执念刀", WeaponKind::Saber, 11.0),
        _ => ("zhinian_spear", "执念枪", WeaponKind::Spear, 9.0),
    };
    eq.main_hand = Some(make_weapon_slot(
        template_id,
        display_name,
        weapon_item_kind_for_tier(tier),
        weapon_kind,
        tier,
        base_attack,
        0.5, // 半损
    ));

    // 1-2 件护甲，半损
    let armor_count = 1 + splitmix64_range(seed.wrapping_add(10), 2); // 1 或 2
    assign_random_armor_with_durability(&mut eq, seed.wrapping_add(100), armor_count, tier, 0.5);

    eq
}

/// 随机填充 N 件护甲到空 slot（head/chest/legs/feet 按 shuffle 顺序）。
fn assign_random_armor(eq: &mut NpcEquipment, seed: u64, count: u32, quality_tier: u8) {
    assign_random_armor_with_durability(eq, seed, count, quality_tier, 0.8);
}

fn assign_random_armor_with_durability(
    eq: &mut NpcEquipment,
    seed: u64,
    count: u32,
    quality_tier: u8,
    durability: f32,
) {
    // 4 个 armor slot：(slot_name, profile_id, display_name)
    let armor_defs: [(&str, &str, &str); 4] = [
        ("head", "armor_iron_helmet", "铁盔"),
        ("chest", "armor_iron_chestplate", "铁甲"),
        ("legs", "armor_iron_leggings", "铁裤"),
        ("feet", "armor_iron_boots", "铁靴"),
    ];

    // Fisher-Yates shuffle（deterministic）选前 count 个
    let mut order: Vec<usize> = (0..4).collect();
    for i in (1..4).rev() {
        let j = splitmix64_range(seed.wrapping_add(i as u64 * 17), (i + 1) as u32) as usize;
        order.swap(i, j);
    }

    for &idx in order.iter().take((count as usize).min(4)) {
        let (slot_name, profile_id, display_name) = armor_defs[idx];
        let item_kind = armor_item_kind_for_tier(slot_name, quality_tier);
        let slot = make_armor_slot(
            profile_id,
            display_name,
            item_kind,
            profile_id,
            quality_tier,
            durability,
        );
        match slot_name {
            "head" => eq.head = Some(slot),
            "chest" => eq.chest = Some(slot),
            "legs" => eq.legs = Some(slot),
            "feet" => eq.feet = Some(slot),
            _ => {}
        }
    }
}

// ─── Merge for Valence Equipment sync ──────────────────────────────────────

use crate::skin::npc_skin_selector::NpcVisualProfile;
use valence::prelude::{Equipment, ItemStack};

/// 合并 NpcEquipment + NpcVisualProfile 到 Valence Equipment。
///
/// NpcEquipment 提供 chest/legs/feet/main_hand 的实际装备外观；
/// NpcVisualProfile 继续驱动 rank aura / head marker 等纯视觉效果。
/// 冲突时 NpcEquipment 优先。
pub fn merge_equipment(npc_eq: Option<&NpcEquipment>, profile: &NpcVisualProfile) -> Equipment {
    use super::super::skin::faction_tint::apply_visual_equipment;

    let mut equipment = Equipment::default();
    // 先应用视觉层（rank marker / faction tint / age marker 等）
    apply_visual_equipment(&mut equipment, profile);

    // 叠加 NpcEquipment（如有），冲突时 NpcEquipment 优先
    if let Some(npc_eq) = npc_eq {
        if let Some(slot) = &npc_eq.main_hand {
            equipment.set_main_hand(ItemStack::new(slot.item_kind, 1, None));
        }
        if let Some(slot) = &npc_eq.off_hand {
            equipment.set_off_hand(ItemStack::new(slot.item_kind, 1, None));
        }
        if let Some(slot) = &npc_eq.head {
            let kind = armor_item_kind_for_tier("head", slot.quality_tier);
            equipment.set_head(ItemStack::new(kind, 1, None));
        }
        if let Some(slot) = &npc_eq.chest {
            let kind = armor_item_kind_for_tier("chest", slot.quality_tier);
            equipment.set_chest(ItemStack::new(kind, 1, None));
        }
        if let Some(slot) = &npc_eq.legs {
            let kind = armor_item_kind_for_tier("legs", slot.quality_tier);
            equipment.set_legs(ItemStack::new(kind, 1, None));
        }
        if let Some(slot) = &npc_eq.feet {
            let kind = armor_item_kind_for_tier("feet", slot.quality_tier);
            equipment.set_feet(ItemStack::new(kind, 1, None));
        }
    }

    equipment
}

// ─── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::npc::faction::FactionId;

    // === P0.1 NpcEquipment + NpcEquipSlot basic ===

    #[test]
    fn npc_equipment_default_all_none() {
        let eq = NpcEquipment::default();
        assert!(eq.main_hand.is_none());
        assert!(eq.off_hand.is_none());
        assert!(eq.head.is_none());
        assert!(eq.chest.is_none());
        assert!(eq.legs.is_none());
        assert!(eq.feet.is_none());
    }

    #[test]
    fn npc_equipment_iter_slots_returns_non_empty() {
        let empty = NpcEquipment::default();
        assert_eq!(
            empty.iter_slots().count(),
            0,
            "empty eq should yield 0 slots"
        );
        let eq = NpcEquipment {
            main_hand: Some(make_weapon_slot(
                "test_sword",
                "测试剑",
                ItemKind::IronSword,
                WeaponKind::Sword,
                0,
                8.0,
                1.0,
            )),
            chest: Some(make_armor_slot(
                "test_armor",
                "测试甲",
                ItemKind::LeatherChestplate,
                "test_profile",
                0,
                1.0,
            )),
            ..Default::default()
        };
        let names: Vec<&str> = eq.iter_slots().map(|(name, _)| name).collect();
        assert_eq!(names, vec!["main_hand", "chest"]);
    }

    #[test]
    fn npc_equipment_armor_slots_only_returns_armor() {
        let eq = NpcEquipment {
            main_hand: Some(make_weapon_slot(
                "sword",
                "剑",
                ItemKind::IronSword,
                WeaponKind::Sword,
                0,
                8.0,
                1.0,
            )),
            head: Some(make_armor_slot(
                "helmet",
                "盔",
                ItemKind::LeatherHelmet,
                "h",
                0,
                1.0,
            )),
            feet: Some(make_armor_slot(
                "boots",
                "靴",
                ItemKind::LeatherBoots,
                "b",
                0,
                1.0,
            )),
            ..Default::default()
        };
        let armor_count = eq.armor_slots().count();
        assert_eq!(
            armor_count, 2,
            "armor_slots should only return head+feet, not main_hand"
        );
    }

    // === P0.1 npc_weapon_damage_multiplier ===

    #[test]
    fn damage_multiplier_tier0_full_durability() {
        let slot = make_weapon_slot(
            "s",
            "s",
            ItemKind::IronSword,
            WeaponKind::Sword,
            0,
            8.0,
            1.0,
        );
        let mul = npc_weapon_damage_multiplier(&slot);
        // attack_mul = max(1.0, 8/10) = 1.0; quality = 1.0; durability = 0.5 + 0.5*1 = 1.0
        let expected = 1.0 * 1.0 * 1.0;
        assert!(
            (mul - expected).abs() < 1e-5,
            "tier0 full dur expected {expected}, got {mul}"
        );
    }

    #[test]
    fn damage_multiplier_tier1_half_durability() {
        let slot = make_weapon_slot(
            "s",
            "s",
            ItemKind::DiamondSword,
            WeaponKind::Sword,
            1,
            14.0,
            0.5,
        );
        let mul = npc_weapon_damage_multiplier(&slot);
        // attack_mul = max(1.0, 14/10) = 1.4; quality = 1.15; durability = 0.5 + 0.5*0.5 = 0.75
        let expected = 1.4 * 1.15 * 0.75;
        assert!(
            (mul - expected).abs() < 1e-4,
            "tier1 half dur expected {expected}, got {mul}"
        );
    }

    #[test]
    fn damage_multiplier_tier2_zero_durability() {
        let slot = make_weapon_slot(
            "s",
            "s",
            ItemKind::GoldenSword,
            WeaponKind::Sword,
            2,
            20.0,
            0.0,
        );
        let mul = npc_weapon_damage_multiplier(&slot);
        // attack_mul = 2.0; quality = 1.35; durability = 0.5 + 0 = 0.5
        let expected = 2.0 * 1.35 * 0.5;
        assert!(
            (mul - expected).abs() < 1e-4,
            "tier2 zero dur expected {expected}, got {mul}"
        );
    }

    #[test]
    fn damage_multiplier_low_base_attack_floors_at_1() {
        let slot = make_weapon_slot(
            "s",
            "s",
            ItemKind::IronSword,
            WeaponKind::Sword,
            0,
            3.0,
            1.0,
        );
        let mul = npc_weapon_damage_multiplier(&slot);
        // attack_mul = max(1.0, 3/10) = 1.0
        assert!(
            (mul - 1.0).abs() < 1e-5,
            "low base_attack should floor to 1.0, got {mul}"
        );
    }

    #[test]
    fn damage_multiplier_clamped_durability_above_1() {
        let slot = make_weapon_slot(
            "s",
            "s",
            ItemKind::IronSword,
            WeaponKind::Sword,
            0,
            10.0,
            1.5,
        );
        let mul = npc_weapon_damage_multiplier(&slot);
        // durability should clamp to 1.0
        let expected = 1.0 * 1.0 * 1.0;
        assert!(
            (mul - expected).abs() < 1e-5,
            "clamped durability expected {expected}, got {mul}"
        );
    }

    #[test]
    fn damage_multiplier_clamped_durability_below_0() {
        let slot = make_weapon_slot(
            "s",
            "s",
            ItemKind::IronSword,
            WeaponKind::Sword,
            0,
            10.0,
            -0.5,
        );
        let mul = npc_weapon_damage_multiplier(&slot);
        let expected = 1.0 * 1.0 * 0.5;
        assert!(
            (mul - expected).abs() < 1e-5,
            "negative durability should clamp to 0.0, got {mul}"
        );
    }

    // === P0.1 quality_multiplier all tiers ===

    #[test]
    fn quality_multiplier_all_tiers() {
        assert!((quality_multiplier(0) - 1.00).abs() < 1e-5);
        assert!((quality_multiplier(1) - 1.15).abs() < 1e-5);
        assert!((quality_multiplier(2) - 1.35).abs() < 1e-5);
        assert!((quality_multiplier(3) - 1.60).abs() < 1e-5);
        assert!((quality_multiplier(255) - 1.60).abs() < 1e-5);
    }

    // === P0.1 reach + wound mapping ===

    #[test]
    fn reach_for_all_weapon_kinds() {
        assert_eq!(reach_for_weapon_kind(WeaponKind::Sword), SWORD_REACH);
        assert_eq!(reach_for_weapon_kind(WeaponKind::Saber), SWORD_REACH);
        assert_eq!(reach_for_weapon_kind(WeaponKind::Spear), SPEAR_REACH);
        assert_eq!(reach_for_weapon_kind(WeaponKind::Staff), SPEAR_REACH);
        assert_eq!(reach_for_weapon_kind(WeaponKind::Fist), FIST_REACH);
        // Dagger has custom reach
        let dagger_reach = reach_for_weapon_kind(WeaponKind::Dagger);
        assert!((dagger_reach.base - 1.2).abs() < 1e-5);
    }

    #[test]
    fn wound_kind_for_all_weapon_kinds() {
        assert_eq!(wound_kind_for_weapon(WeaponKind::Sword), WoundKind::Cut);
        assert_eq!(wound_kind_for_weapon(WeaponKind::Saber), WoundKind::Cut);
        assert_eq!(wound_kind_for_weapon(WeaponKind::Dagger), WoundKind::Cut);
        assert_eq!(wound_kind_for_weapon(WeaponKind::Staff), WoundKind::Blunt);
        assert_eq!(wound_kind_for_weapon(WeaponKind::Fist), WoundKind::Blunt);
        assert_eq!(wound_kind_for_weapon(WeaponKind::Spear), WoundKind::Pierce);
        assert_eq!(wound_kind_for_weapon(WeaponKind::Bow), WoundKind::Pierce);
    }

    // === P0.1 ItemKind mapping ===

    #[test]
    fn weapon_item_kind_tiers() {
        assert_eq!(weapon_item_kind_for_tier(0), ItemKind::IronSword);
        assert_eq!(weapon_item_kind_for_tier(1), ItemKind::DiamondSword);
        assert_eq!(weapon_item_kind_for_tier(2), ItemKind::GoldenSword);
        assert_eq!(weapon_item_kind_for_tier(3), ItemKind::GoldenSword);
    }

    #[test]
    fn armor_item_kind_tiers_per_slot() {
        assert_eq!(armor_item_kind_for_tier("head", 0), ItemKind::LeatherHelmet);
        assert_eq!(
            armor_item_kind_for_tier("head", 1),
            ItemKind::ChainmailHelmet
        );
        assert_eq!(armor_item_kind_for_tier("head", 2), ItemKind::IronHelmet);
        assert_eq!(
            armor_item_kind_for_tier("chest", 0),
            ItemKind::LeatherChestplate
        );
        assert_eq!(
            armor_item_kind_for_tier("legs", 1),
            ItemKind::ChainmailLeggings
        );
        assert_eq!(armor_item_kind_for_tier("feet", 2), ItemKind::IronBoots);
    }

    // === P0.2 assign_npc_equipment — archetype coverage ===

    #[test]
    fn assign_zombie_returns_empty() {
        let eq = assign_npc_equipment(NpcArchetype::Zombie, Realm::Awaken, None, 42);
        assert_eq!(eq, NpcEquipment::default());
    }

    #[test]
    fn assign_beast_returns_empty() {
        let eq = assign_npc_equipment(NpcArchetype::Beast, Realm::Condense, None, 42);
        assert_eq!(eq, NpcEquipment::default());
    }

    #[test]
    fn assign_fuya_returns_empty() {
        let eq = assign_npc_equipment(NpcArchetype::Fuya, Realm::Spirit, None, 42);
        assert_eq!(eq, NpcEquipment::default());
    }

    #[test]
    fn assign_skull_fiend_returns_empty() {
        let eq = assign_npc_equipment(NpcArchetype::SkullFiend, Realm::Void, None, 42);
        assert_eq!(eq, NpcEquipment::default());
    }

    #[test]
    fn assign_rogue_has_weapon_or_empty_deterministic() {
        let eq1 = assign_npc_equipment(NpcArchetype::Rogue, Realm::Awaken, None, 12345);
        let eq2 = assign_npc_equipment(NpcArchetype::Rogue, Realm::Awaken, None, 12345);
        assert_eq!(eq1, eq2, "same seed should produce identical equipment");
    }

    #[test]
    fn assign_rogue_quality_tier_never_exceeds_2() {
        for seed in 0..200u64 {
            let eq = assign_npc_equipment(NpcArchetype::Rogue, Realm::Void, None, seed * 7919);
            for (_name, slot) in eq.iter_slots() {
                assert!(
                    slot.quality_tier <= 2,
                    "seed {} slot quality_tier {} exceeds max 2 (法宝)",
                    seed,
                    slot.quality_tier
                );
            }
        }
    }

    #[test]
    fn assign_rogue_condense_upgrades_tier() {
        // Scan seeds until we find iron_sword and wooden_staff to assert tier upgrade on both
        let mut found_sword = false;
        let mut found_staff = false;
        for seed in 0..500u64 {
            let eq = assign_npc_equipment(NpcArchetype::Rogue, Realm::Condense, None, seed);
            if let Some(main) = &eq.main_hand {
                if main.template_id == "iron_sword" && !found_sword {
                    assert!(
                        main.quality_tier >= 1,
                        "Condense rogue iron_sword should have tier >= 1, got {} (seed {})",
                        main.quality_tier,
                        seed
                    );
                    found_sword = true;
                }
                if main.template_id == "wooden_staff" && !found_staff {
                    assert!(
                        main.quality_tier >= 1,
                        "Condense rogue wooden_staff should have tier >= 1, got {} (seed {})",
                        main.quality_tier,
                        seed
                    );
                    found_staff = true;
                }
            }
            if found_sword && found_staff {
                break;
            }
        }
        assert!(
            found_sword,
            "failed to find iron_sword seed in 500 iterations"
        );
        assert!(
            found_staff,
            "failed to find wooden_staff seed in 500 iterations"
        );
    }

    #[test]
    fn assign_commoner_mostly_empty_handed() {
        let mut empty_count = 0;
        for seed in 0..100u64 {
            let eq = assign_npc_equipment(NpcArchetype::Commoner, Realm::Awaken, None, seed * 31);
            if eq.main_hand.is_none() {
                empty_count += 1;
            }
        }
        // Should be roughly 80% empty-handed
        assert!(
            empty_count >= 60,
            "commoner should be ~80% empty-handed, got {empty_count}/100"
        );
    }

    #[test]
    fn assign_disciple_always_has_weapon() {
        for seed in 0..50u64 {
            for faction in [
                Some(FactionId::Attack),
                Some(FactionId::Defend),
                Some(FactionId::Neutral),
            ] {
                let eq =
                    assign_npc_equipment(NpcArchetype::Disciple, Realm::Awaken, faction, seed * 7);
                assert!(
                    eq.main_hand.is_some(),
                    "disciple should always have main_hand weapon (seed={seed}, faction={faction:?})"
                );
            }
        }
    }

    #[test]
    fn assign_disciple_defend_has_staff_and_shield() {
        let eq = assign_npc_equipment(
            NpcArchetype::Disciple,
            Realm::Awaken,
            Some(FactionId::Defend),
            42,
        );
        assert!(eq.main_hand.is_some(), "defend disciple should have staff");
        assert!(eq.off_hand.is_some(), "defend disciple should have shield");
        let staff = eq.main_hand.unwrap();
        assert_eq!(
            staff.weapon_kind,
            Some(WeaponKind::Staff),
            "defend disciple main_hand should be staff"
        );
    }

    #[test]
    fn assign_guardian_relic_ancient_sword_tier2() {
        let eq = assign_npc_equipment(NpcArchetype::GuardianRelic, Realm::Spirit, None, 42);
        let main = eq.main_hand.expect("guardian relic must have weapon");
        assert_eq!(main.template_id, "ancient_sword");
        assert_eq!(main.quality_tier, 2);
        assert_eq!(main.item_kind, ItemKind::GoldenSword);
    }

    #[test]
    fn assign_guardian_relic_has_at_least_2_armor() {
        let eq = assign_npc_equipment(NpcArchetype::GuardianRelic, Realm::Awaken, None, 42);
        let armor_count = eq.armor_slots().count();
        assert!(
            armor_count >= 2,
            "guardian relic should have >= 2 armor pieces, got {armor_count}"
        );
    }

    #[test]
    fn assign_daoxiang_rusty_sword_low_durability() {
        let eq = assign_npc_equipment(NpcArchetype::Daoxiang, Realm::Awaken, None, 42);
        let main = eq.main_hand.expect("daoxiang must have rusty sword");
        assert_eq!(main.template_id, "rusted_sword");
        assert!(
            (main.durability_ratio - 0.3).abs() < 1e-5,
            "daoxiang sword durability should be 0.3"
        );
    }

    #[test]
    fn assign_zhinian_always_has_weapon_half_damaged() {
        for seed in 0..50u64 {
            let eq = assign_npc_equipment(NpcArchetype::Zhinian, Realm::Induce, None, seed * 13);
            let main = eq.main_hand.as_ref().expect("zhinian must have weapon");
            assert!(
                (main.durability_ratio - 0.5).abs() < 1e-5,
                "zhinian weapon should have durability 0.5"
            );
        }
    }

    // === P0.2 determinism ===

    #[test]
    fn assign_deterministic_all_archetypes() {
        for archetype in [
            NpcArchetype::Zombie,
            NpcArchetype::Commoner,
            NpcArchetype::Rogue,
            NpcArchetype::Beast,
            NpcArchetype::Disciple,
            NpcArchetype::GuardianRelic,
            NpcArchetype::Daoxiang,
            NpcArchetype::Zhinian,
            NpcArchetype::Fuya,
            NpcArchetype::SkullFiend,
        ] {
            for realm in [
                Realm::Awaken,
                Realm::Induce,
                Realm::Condense,
                Realm::Solidify,
                Realm::Spirit,
                Realm::Void,
            ] {
                let a = assign_npc_equipment(archetype, realm, None, 42);
                let b = assign_npc_equipment(archetype, realm, None, 42);
                assert_eq!(a, b, "determinism failed for {archetype:?} x {realm:?}");
            }
        }
    }

    // === P0.3 merge_equipment ===

    #[test]
    fn merge_equipment_no_npc_eq_uses_visual_profile() {
        use crate::skin::npc_skin_selector::select_npc_visual_profile;
        let profile =
            select_npc_visual_profile(NpcArchetype::Disciple, Realm::Awaken, None, None, 0.5);
        let merged = merge_equipment(None, &profile);
        let visual_only = super::super::super::skin::faction_tint::visual_equipment(&profile);
        // Without NpcEquipment, should match visual profile output
        assert_eq!(merged.head().item, visual_only.head().item);
        assert_eq!(merged.chest().item, visual_only.chest().item);
    }

    #[test]
    fn merge_equipment_npc_eq_overrides_visual_profile() {
        use crate::skin::npc_skin_selector::select_npc_visual_profile;
        let profile = select_npc_visual_profile(
            NpcArchetype::Disciple,
            Realm::Awaken,
            Some(FactionId::Attack),
            Some(crate::npc::faction::FactionRank::Disciple),
            0.5,
        );
        let npc_eq = NpcEquipment {
            chest: Some(make_armor_slot(
                "iron_chest",
                "铁甲",
                ItemKind::IronChestplate,
                "armor_iron_chestplate",
                1,
                0.9,
            )),
            ..Default::default()
        };
        let merged = merge_equipment(Some(&npc_eq), &profile);
        // NpcEquipment chest should override faction tint chest
        assert_eq!(
            merged.chest().item,
            ItemKind::ChainmailChestplate, // tier 1 maps to chainmail
            "NpcEquipment should override visual profile chest slot"
        );
    }

    #[test]
    fn merge_equipment_empty_npc_eq_preserves_visual() {
        use crate::skin::npc_skin_selector::select_npc_visual_profile;
        let profile = select_npc_visual_profile(
            NpcArchetype::Disciple,
            Realm::Awaken,
            Some(FactionId::Attack),
            Some(crate::npc::faction::FactionRank::Disciple),
            0.5,
        );
        let npc_eq = NpcEquipment::default(); // all None
        let merged = merge_equipment(Some(&npc_eq), &profile);
        let visual_only = super::super::super::skin::faction_tint::visual_equipment(&profile);
        // Empty NpcEquipment should not clear visual profile equipment
        assert_eq!(merged.chest().item, visual_only.chest().item);
    }

    #[test]
    fn merge_equipment_main_hand_weapon() {
        use crate::skin::npc_skin_selector::select_npc_visual_profile;
        let profile =
            select_npc_visual_profile(NpcArchetype::Rogue, Realm::Awaken, None, None, 0.5);
        let npc_eq = NpcEquipment {
            main_hand: Some(make_weapon_slot(
                "iron_sword",
                "铁剑",
                ItemKind::IronSword,
                WeaponKind::Sword,
                0,
                8.0,
                1.0,
            )),
            ..Default::default()
        };
        let merged = merge_equipment(Some(&npc_eq), &profile);
        assert_eq!(merged.main_hand().item, ItemKind::IronSword);
    }

    // === P0.4 战斗结算（纯函数测试） ===

    #[test]
    fn equipped_npc_has_higher_damage_than_unarmed() {
        let armed_slot = make_weapon_slot(
            "iron_sword",
            "铁剑",
            ItemKind::IronSword,
            WeaponKind::Sword,
            1,
            12.0,
            0.9,
        );
        let armed_mul = npc_weapon_damage_multiplier(&armed_slot);
        let unarmed_mul = 1.0_f32; // 无装备走默认 1.0 乘数
        assert!(
            armed_mul > unarmed_mul,
            "armed NPC damage multiplier ({armed_mul}) should be > unarmed ({unarmed_mul})"
        );
    }

    #[test]
    fn higher_tier_weapon_does_more_damage() {
        let tier0 = make_weapon_slot(
            "s",
            "s",
            ItemKind::IronSword,
            WeaponKind::Sword,
            0,
            10.0,
            1.0,
        );
        let tier1 = make_weapon_slot(
            "s",
            "s",
            ItemKind::DiamondSword,
            WeaponKind::Sword,
            1,
            10.0,
            1.0,
        );
        let tier2 = make_weapon_slot(
            "s",
            "s",
            ItemKind::GoldenSword,
            WeaponKind::Sword,
            2,
            10.0,
            1.0,
        );
        let m0 = npc_weapon_damage_multiplier(&tier0);
        let m1 = npc_weapon_damage_multiplier(&tier1);
        let m2 = npc_weapon_damage_multiplier(&tier2);
        assert!(m1 > m0, "tier1 ({m1}) should > tier0 ({m0})");
        assert!(m2 > m1, "tier2 ({m2}) should > tier1 ({m1})");
    }

    // === P0.5 death drops ===

    #[test]
    fn roll_equipment_drops_deterministic() {
        let eq = assign_npc_equipment(NpcArchetype::Rogue, Realm::Awaken, None, 42);
        let drops1 = roll_equipment_drops(&eq, 999);
        let drops2 = roll_equipment_drops(&eq, 999);
        assert_eq!(drops1, drops2, "same seed should produce identical drops");
    }

    #[test]
    fn roll_equipment_drops_empty_equipment_no_drops() {
        let eq = NpcEquipment::default();
        let drops = roll_equipment_drops(&eq, 42);
        assert!(drops.is_empty(), "empty equipment should produce no drops");
    }

    #[test]
    fn roll_equipment_drops_rate_roughly_30_percent() {
        // 用 Guardian Relic（通常 3-5 slot 有装备）大量 roll 统计命中率
        let eq = assign_npc_equipment(NpcArchetype::GuardianRelic, Realm::Spirit, None, 42);
        let slot_count = eq.iter_slots().count();
        assert!(
            slot_count >= 3,
            "guardian relic should have >= 3 slots filled"
        );

        let mut total_possible = 0usize;
        let mut total_dropped = 0usize;
        for seed in 0..1000u64 {
            let drops = roll_equipment_drops(&eq, seed * 7919);
            total_possible += slot_count;
            total_dropped += drops.len();
        }
        let rate = total_dropped as f32 / total_possible as f32;
        assert!(
            (0.25..=0.35).contains(&rate),
            "equipment drop rate should be ~30%, got {rate:.3} ({total_dropped}/{total_possible})"
        );
    }

    #[test]
    fn weapon_drop_durability_formula() {
        let slot = make_weapon_slot(
            "s",
            "s",
            ItemKind::IronSword,
            WeaponKind::Sword,
            0,
            8.0,
            0.8,
        );
        let dur = weapon_drop_durability(&slot);
        assert!(
            (dur - 0.64).abs() < 1e-5,
            "weapon drop dur should be 0.8*0.8=0.64, got {dur}"
        );
    }

    #[test]
    fn armor_drop_durability_formula() {
        let slot = make_armor_slot("a", "a", ItemKind::LeatherChestplate, "p", 0, 0.7);
        let dur = armor_drop_durability(&slot);
        assert!(
            (dur - 0.49).abs() < 1e-5,
            "armor drop dur should be 0.7*0.7=0.49, got {dur}"
        );
    }

    #[test]
    fn weapon_drop_durability_clamped() {
        let slot = make_weapon_slot(
            "s",
            "s",
            ItemKind::IronSword,
            WeaponKind::Sword,
            0,
            8.0,
            1.5,
        );
        let dur = weapon_drop_durability(&slot);
        assert!(dur <= 1.0, "durability should be clamped to 1.0, got {dur}");
    }

    #[test]
    fn armor_drop_durability_zero() {
        let slot = make_armor_slot("a", "a", ItemKind::LeatherChestplate, "p", 0, 0.0);
        let dur = armor_drop_durability(&slot);
        assert!((dur - 0.0).abs() < 1e-5, "zero durability should yield 0.0");
    }

    // === P0.2 每个 archetype x 每个 realm 至少 1 case ===

    #[test]
    fn assign_all_archetypes_all_realms_produces_valid_equipment() {
        let all_archetypes = [
            NpcArchetype::Zombie,
            NpcArchetype::Commoner,
            NpcArchetype::Rogue,
            NpcArchetype::Beast,
            NpcArchetype::Disciple,
            NpcArchetype::GuardianRelic,
            NpcArchetype::Daoxiang,
            NpcArchetype::Zhinian,
            NpcArchetype::Fuya,
            NpcArchetype::SkullFiend,
        ];
        let all_realms = [
            Realm::Awaken,
            Realm::Induce,
            Realm::Condense,
            Realm::Solidify,
            Realm::Spirit,
            Realm::Void,
        ];
        for archetype in all_archetypes {
            for realm in all_realms {
                let eq = assign_npc_equipment(archetype, realm, None, 42);
                for (_name, slot) in eq.iter_slots() {
                    assert!(
                        slot.quality_tier <= 2,
                        "{archetype:?} x {realm:?}: quality_tier {} exceeds max 2",
                        slot.quality_tier
                    );
                    assert!(
                        slot.durability_ratio >= 0.0 && slot.durability_ratio <= 1.5,
                        "{archetype:?} x {realm:?}: durability_ratio {} out of range",
                        slot.durability_ratio
                    );
                }
            }
        }
    }
}
