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
        | NpcArchetype::DyingElder
        // plan-mundane-fauna-v1 P0：凡兽无灵无装备（原生 MC 动物天生武器/无甲）
        | NpcArchetype::Mundane => NpcEquipment::default(),
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
#[path = "equipment_tests.rs"]
mod tests;
