//! NPC 交易库存 + 动态定价 + 信誉系统（plan-npc-overhaul-v1 §P3）。
//!
//! `NpcTradeInventory` component 在 spawn 时由 `assign_npc_trade_inventory()` 一次性生成。
//! 交易物品与装备独立——散修可能卖灵草但自己用剑战斗，两者不矛盾。
//!
//! 定价走骨币枚数（整数），server 端按 shelflife 模块的骨币真元含量做折算——
//! GUI 层不暴露半衰期细节。
//!
//! P3 新增：
//! - `NpcPlayerReputation`：per-NPC per-player 信誉度 (0.0-1.0)
//! - `DynamicPricing`：zone_qi × reputation × scarcity × urgency 动态定价
//! - `TradeEligibility`：信誉门控交易资格
//! - `InformationOffer`：NPC 出售信息商品
//! - `ReputationGossipEvent`：NPC 间传话系统
//! - `TradeAbortedByNpc`：翻脸掠夺事件

use std::collections::HashMap;

use valence::prelude::{bevy_ecs, App, Component, Entity, Resource, Update};

use crate::cultivation::components::Realm;
use crate::cultivation::technique_scroll::realm_rank;
use crate::npc::lifecycle::NpcArchetype;

// ─── splitmix64 helpers (deterministic RNG) ──────────────────────────────────

fn splitmix64(seed: u64) -> u64 {
    let mut x = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

fn splitmix64_range(seed: u64, n: u32) -> u32 {
    if n == 0 {
        return 0;
    }
    let bits = ((splitmix64(seed) >> 40) & 0x00FF_FFFF) as u32;
    let unit = bits as f32 / (1u32 << 24) as f32;
    (unit * n as f32) as u32 % n
}

// ─── TradeOffer ─────────────────────────────────────────────────────────────

/// 单条交易报价。
#[derive(Clone, Debug, PartialEq)]
pub struct TradeOffer {
    /// 物品模板 ID（对应 `ItemTemplateRegistry` 的 key）。
    pub template_id: String,
    /// 展示用中文名称（下发到 client HUD）。
    pub display_name: String,
    /// 可供购买的数量。
    pub count: u32,
    /// 骨币定价（整数枚）。
    pub price_bone_coins: u32,
}

// ─── NpcTradeInventory ──────────────────────────────────────────────────────

/// NPC 可售卖物品列表（spawn 时一次性生成，§8.1 #3 方案 B）。
///
/// 通过 `bong:npc_metadata` S2C packet 下发给 client。
#[derive(Clone, Debug, Default, Component)]
pub struct NpcTradeInventory {
    pub offers: Vec<TradeOffer>,
}

// ─── NpcPlayerReputation (P3.1) ─────────────────────────────────────────────

/// Per-NPC per-player 信誉度。默认 0.5（中立），范围 [0.0, 1.0]。
///
/// 信誉来源：
/// - 交易成功: +0.05
/// - 被攻击: -0.3
/// - 传话（gossip）: -0.05
/// - 帮助: +0.1
#[derive(Component, Default, Clone, Debug)]
pub struct NpcPlayerReputation {
    scores: HashMap<String, f32>,
}

impl NpcPlayerReputation {
    /// 获取某玩家的信誉度，未记录玩家默认 0.5。
    pub fn get(&self, player_id: &str) -> f32 {
        *self.scores.get(player_id).unwrap_or(&0.5)
    }

    /// 调整某玩家的信誉度，结果 clamp 到 [0.0, 1.0]。
    pub fn adjust(&mut self, player_id: &str, delta: f32) {
        let score = self.scores.entry(player_id.to_string()).or_insert(0.5);
        *score = (*score + delta).clamp(0.0, 1.0);
    }

    /// 返回某玩家对应的信誉等级。
    pub fn tier(&self, player_id: &str) -> RepTier {
        RepTier::from_score(self.get(player_id))
    }
}

/// 信誉等级——决定交易资格和价格修正。
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum RepTier {
    /// score > 0.7: 折扣交易
    High,
    /// score > 0.3: 正常交易
    Mid,
    /// score > 0.1: 加价 + 拒绝稀有品
    Low,
    /// score <= 0.1: 拒绝一切交易
    Hostile,
}

impl RepTier {
    pub fn from_score(score: f32) -> Self {
        if score > 0.7 {
            Self::High
        } else if score > 0.3 {
            Self::Mid
        } else if score > 0.1 {
            Self::Low
        } else {
            Self::Hostile
        }
    }
}

// ─── DynamicPricing (P3.2) ─────────────────────────────────────────────────

/// 动态定价配置参数。
#[derive(Clone, Debug, Resource)]
pub struct TradePricingConfig {
    /// 区域灵气低阈值——低于此值涨价。
    pub zone_qi_low_threshold: f32,
    /// 区域灵气高阈值——高于此值降价。
    pub zone_qi_high_threshold: f32,
    /// 低灵气区域价格乘数。
    pub zone_qi_low_multiplier: f32,
    /// 高灵气区域价格乘数。
    pub zone_qi_high_multiplier: f32,
    /// 高信誉折扣率。
    pub rep_high_discount: f32,
    /// 低信誉加价率。
    pub rep_low_markup: f32,
    /// 价格下限比例（final >= base * floor_ratio）。
    pub floor_ratio: f32,
}

impl Default for TradePricingConfig {
    fn default() -> Self {
        Self {
            zone_qi_low_threshold: 0.3,
            zone_qi_high_threshold: 0.6,
            zone_qi_low_multiplier: 1.5,
            zone_qi_high_multiplier: 0.8,
            rep_high_discount: 0.85,
            rep_low_markup: 1.3,
            floor_ratio: 0.3,
        }
    }
}

/// 动态定价计算器。
pub struct DynamicPricing;

impl DynamicPricing {
    /// 根据区域灵气、信誉、稀缺度、NPC 紧迫度计算最终价格。
    ///
    /// 公式：
    /// ```text
    /// zone_mod = qi < low_threshold → low_multiplier | qi > high_threshold → high_multiplier | else → 1.0
    /// rep_mod  = rep > 0.7 → rep_high_discount | rep < 0.3 → rep_low_markup | else → 1.0
    /// scarcity_mod = max(1.0, 1.0 + scarcity * 0.5)  // 稀缺品只涨不降
    /// urgency_mod  = if npc_qi < 0.2 → 0.7 | else → 1.0
    /// raw = base × zone_mod × rep_mod × scarcity_mod × urgency_mod
    /// final = max(base × floor_ratio, raw).round(), min 1
    /// ```
    pub fn compute_price(
        base_price: u32,
        zone_qi: f32,
        npc_qi_need: f32,
        reputation: f32,
        supply_scarcity: f32,
        config: &TradePricingConfig,
    ) -> u32 {
        let zone_mod = if zone_qi < config.zone_qi_low_threshold {
            config.zone_qi_low_multiplier
        } else if zone_qi > config.zone_qi_high_threshold {
            config.zone_qi_high_multiplier
        } else {
            1.0
        };

        let rep_mod = if reputation > 0.7 {
            config.rep_high_discount
        } else if reputation < 0.3 {
            config.rep_low_markup
        } else {
            1.0
        };

        let scarcity_mod = (1.0 + supply_scarcity * 0.5).max(1.0);

        let urgency_mod = if npc_qi_need < 0.2 { 0.7 } else { 1.0 };

        let raw = base_price as f32 * zone_mod * rep_mod * scarcity_mod * urgency_mod;
        let floor = base_price as f32 * config.floor_ratio;
        let final_price = raw.max(floor).round() as u32;
        final_price.max(1)
    }
}

// ─── TradeEligibility (P3.3) ───────────────────────────────────────────────

/// 交易资格——基于信誉等级决定是否允许交易及价格修正。
#[derive(Clone, Debug, PartialEq)]
pub enum TradeEligibility {
    /// 允许交易，附带价格修正系数。
    Allowed { price_modifier: f32 },
    /// 低信誉：允许普通物品，拒绝稀有品。
    RefuseRare,
    /// 敌对：拒绝一切交易。
    Refused,
}

/// 根据信誉等级判定交易资格。
pub fn check_trade_eligibility(tier: RepTier) -> TradeEligibility {
    match tier {
        RepTier::High => TradeEligibility::Allowed {
            price_modifier: 0.85,
        },
        RepTier::Mid => TradeEligibility::Allowed {
            price_modifier: 1.0,
        },
        RepTier::Low => TradeEligibility::RefuseRare,
        RepTier::Hostile => TradeEligibility::Refused,
    }
}

// ─── InformationOffer (P3.5) ───────────────────────────────────────────────

/// 信息商品——NPC 出售的情报。
#[derive(Clone, Debug)]
pub struct InformationOffer {
    /// 情报类型。
    pub info_kind: InfoKind,
    /// 骨币价格。
    pub price_bone_coins: u32,
    /// 准确度 (0.0-1.0)——受信誉等级影响。
    pub accuracy: f32,
    /// 过期时间（tick 数），过期后情报失效。
    pub expiry_ticks: u64,
}

/// 情报类型。
#[derive(Clone, Debug, PartialEq)]
pub enum InfoKind {
    /// 区域灵气浓度情报。
    ZoneQiLevel { zone_name: String, qi_value: f32 },
    /// 危险警告。
    DangerWarning {
        zone_name: String,
        threat_desc: String,
    },
    /// 资源位置。
    ResourceLocation { zone_name: String, resource: String },
    /// NPC 行踪。
    NpcSighting {
        target_desc: String,
        last_zone: String,
    },
}

/// 生成 0-2 条信息商品。
///
/// accuracy 按信誉等级分级：
/// - High: 0.8-1.0
/// - Mid:  0.5-0.8
/// - Low:  0.3-0.6
pub fn generate_info_offers(
    home_zone: &str,
    npc_rep_tier: RepTier,
    seed: u64,
) -> Vec<InformationOffer> {
    // 决定生成数量（0-2）
    let count = splitmix64_range(seed, 3) as usize; // 0, 1, or 2
    if count == 0 {
        return Vec::new();
    }

    let (accuracy_min, accuracy_max) = match npc_rep_tier {
        RepTier::High => (0.8_f32, 1.0_f32),
        RepTier::Mid => (0.5, 0.8),
        RepTier::Low | RepTier::Hostile => (0.3, 0.6),
    };

    let mut offers = Vec::with_capacity(count);
    for i in 0..count {
        let item_seed = splitmix64(seed.wrapping_add(i as u64 * 0x9E37_79B9));
        let kind_idx = splitmix64_range(item_seed, 4);
        let accuracy_seed = splitmix64(item_seed.wrapping_add(1));
        let accuracy_unit = ((accuracy_seed >> 40) & 0x00FF_FFFF) as f32 / (1u32 << 24) as f32;
        let accuracy = accuracy_min + accuracy_unit * (accuracy_max - accuracy_min);

        let info_kind = match kind_idx {
            0 => InfoKind::ZoneQiLevel {
                zone_name: home_zone.to_string(),
                qi_value: 0.5,
            },
            1 => InfoKind::DangerWarning {
                zone_name: home_zone.to_string(),
                threat_desc: "异兽出没".to_string(),
            },
            2 => InfoKind::ResourceLocation {
                zone_name: home_zone.to_string(),
                resource: "灵草".to_string(),
            },
            _ => InfoKind::NpcSighting {
                target_desc: "散修".to_string(),
                last_zone: home_zone.to_string(),
            },
        };

        let price_seed = splitmix64(item_seed.wrapping_add(2));
        let price = 5 + splitmix64_range(price_seed, 16); // 5-20 骨币

        offers.push(InformationOffer {
            info_kind,
            price_bone_coins: price,
            accuracy,
            expiry_ticks: 24_000, // 约 20 分钟
        });
    }

    offers
}

// ─── ReputationGossipEvent (P3.4) ──────────────────────────────────────────

/// NPC 间传话事件——当 NPC 与 Hostile 信誉玩家交互时发出。
#[derive(Clone, Debug, bevy_ecs::event::Event)]
pub struct ReputationGossipEvent {
    /// 发起传话的 NPC。
    pub source_npc: Entity,
    /// 目标玩家 ID（character_id）。
    pub target_player_id: String,
    /// 信誉变化量（通常为负值）。
    pub delta: f32,
    /// 剩余跳数——初始为 3，每跳减 1。
    pub hops_remaining: u8,
}

/// 待处理的传话条目（带延迟）。
#[derive(Clone, Debug)]
pub struct PendingGossipEntry {
    /// 目标玩家 ID。
    pub target_player_id: String,
    /// 信誉变化量。
    pub delta: f32,
    /// 剩余跳数。
    pub hops_remaining: u8,
    /// 来源 NPC（用于位置查询）。
    pub source_npc: Entity,
    /// 剩余延迟 tick 数。
    pub remaining_ticks: u32,
}

/// 排队中的传话存储。
#[derive(Debug, Default, Resource)]
pub struct PendingGossip {
    pub entries: Vec<PendingGossipEntry>,
}

/// 传话传播范围（格）。
pub const GOSSIP_PROPAGATION_RADIUS: f64 = 48.0;
/// 传话延迟（tick）。
pub const GOSSIP_DELAY_TICKS: u32 = 120;
/// 传话初始跳数。
pub const GOSSIP_INITIAL_HOPS: u8 = 3;

/// 系统：tick down pending gossips，到期时发出 `ReputationGossipEvent`。
pub fn tick_pending_gossips(
    mut pending: valence::prelude::ResMut<PendingGossip>,
    mut gossip_events: valence::prelude::EventWriter<ReputationGossipEvent>,
) {
    let mut i = 0;
    while i < pending.entries.len() {
        pending.entries[i].remaining_ticks = pending.entries[i].remaining_ticks.saturating_sub(1);
        if pending.entries[i].remaining_ticks == 0 {
            let entry = pending.entries.swap_remove(i);
            gossip_events.send(ReputationGossipEvent {
                source_npc: entry.source_npc,
                target_player_id: entry.target_player_id,
                delta: entry.delta,
                hops_remaining: entry.hops_remaining,
            });
        } else {
            i += 1;
        }
    }
}

/// 系统：处理收到的 gossip 事件，对范围内 NPC 施加信誉影响并转发。
pub fn process_gossip_events(
    mut events: valence::prelude::EventReader<ReputationGossipEvent>,
    mut npcs: valence::prelude::Query<
        (
            Entity,
            &valence::prelude::Position,
            &mut NpcPlayerReputation,
        ),
        valence::prelude::With<crate::npc::spawn::NpcMarker>,
    >,
    mut pending: valence::prelude::ResMut<PendingGossip>,
) {
    for event in events.read() {
        let source_pos = npcs.get(event.source_npc).ok().map(|(_, pos, _)| pos.get());

        let Some(source_pos) = source_pos else {
            continue;
        };

        // 衰减系数: delta * 0.5^(GOSSIP_INITIAL_HOPS - hops_remaining)
        let decay_exponent = GOSSIP_INITIAL_HOPS.saturating_sub(event.hops_remaining) as f32;
        let effective_delta = event.delta * 0.5_f32.powf(decay_exponent);

        let mut affected: Vec<Entity> = Vec::new();

        for (npc_entity, npc_pos, mut rep) in &mut npcs {
            if npc_entity == event.source_npc {
                continue;
            }
            let npc_pos = npc_pos.get();
            let dist_sq = (npc_pos.x - source_pos.x).powi(2)
                + (npc_pos.y - source_pos.y).powi(2)
                + (npc_pos.z - source_pos.z).powi(2);
            if dist_sq <= GOSSIP_PROPAGATION_RADIUS * GOSSIP_PROPAGATION_RADIUS {
                rep.adjust(&event.target_player_id, effective_delta);
                affected.push(npc_entity);
            }
        }

        // 转发：hops > 0 时，受影响的 NPC 继续传播
        if event.hops_remaining > 0 {
            for npc_entity in affected {
                pending.entries.push(PendingGossipEntry {
                    target_player_id: event.target_player_id.clone(),
                    delta: event.delta,
                    hops_remaining: event.hops_remaining - 1,
                    source_npc: npc_entity,
                    remaining_ticks: GOSSIP_DELAY_TICKS,
                });
            }
        }
    }
}

// ─── TradeAbortedByNpc (P3.6) ──────────────────────────────────────────────

/// NPC 中止交易事件（翻脸掠夺/敌意拒绝）。
#[derive(Clone, Debug, bevy_ecs::event::Event)]
pub struct TradeAbortedByNpc {
    /// 发起中止的 NPC。
    pub npc: Entity,
    /// 目标玩家。
    pub player: Entity,
    /// 中止原因。
    pub reason: TradeAbortReason,
}

/// 交易中止原因。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TradeAbortReason {
    /// 翻脸掠夺。
    Ambush,
    /// 敌对信誉。
    HostileReputation,
}

// ─── register ──────────────────────────────────────────────────────────────

/// 注册交易系统到 Bevy App。
pub fn register(app: &mut App) {
    app.insert_resource(TradePricingConfig::default())
        .insert_resource(PendingGossip::default())
        .add_event::<ReputationGossipEvent>()
        .add_event::<TradeAbortedByNpc>()
        .add_systems(Update, (tick_pending_gossips, process_gossip_events));
}

// ─── Static trade catalogue ─────────────────────────────────────────────────

/// 候选交易物品条目（静态定义，按 realm_min 过滤）。
struct CatalogueEntry {
    template_id: &'static str,
    display_name: &'static str,
    realm_min: u8,
    price_bone_coins: u32,
    count_min: u32,
    count_max: u32,
}

/// 交易物品候选库——覆盖灵草/丹药/残卷/工具/材料等类目。
/// 各 realm tier 逐步解锁高阶物品。
const TRADE_CATALOGUE: &[CatalogueEntry] = &[
    // Awaken tier
    CatalogueEntry {
        template_id: "lingcao",
        display_name: "灵草",
        realm_min: 0,
        price_bone_coins: 12,
        count_min: 1,
        count_max: 5,
    },
    CatalogueEntry {
        template_id: "bone_meal",
        display_name: "骨粉",
        realm_min: 0,
        price_bone_coins: 5,
        count_min: 2,
        count_max: 8,
    },
    CatalogueEntry {
        template_id: "rough_bandage",
        display_name: "粗布绷带",
        realm_min: 0,
        price_bone_coins: 8,
        count_min: 1,
        count_max: 3,
    },
    // Induce tier
    CatalogueEntry {
        template_id: "fragment_scroll",
        display_name: "残卷",
        realm_min: 1,
        price_bone_coins: 45,
        count_min: 1,
        count_max: 2,
    },
    CatalogueEntry {
        template_id: "qi_condensing_powder",
        display_name: "凝气散",
        realm_min: 1,
        price_bone_coins: 30,
        count_min: 1,
        count_max: 3,
    },
    // Condense tier
    CatalogueEntry {
        template_id: "meridian_salve",
        display_name: "通脉膏",
        realm_min: 2,
        price_bone_coins: 60,
        count_min: 1,
        count_max: 2,
    },
    CatalogueEntry {
        template_id: "spirit_stone_shard",
        display_name: "灵石碎片",
        realm_min: 2,
        price_bone_coins: 80,
        count_min: 1,
        count_max: 3,
    },
    // Solidify tier
    CatalogueEntry {
        template_id: "bone_reinforcing_pill",
        display_name: "固骨丹",
        realm_min: 3,
        price_bone_coins: 120,
        count_min: 1,
        count_max: 2,
    },
    // Spirit tier
    CatalogueEntry {
        template_id: "spirit_jade",
        display_name: "灵玉",
        realm_min: 4,
        price_bone_coins: 200,
        count_min: 1,
        count_max: 1,
    },
];

// ─── assign_npc_trade_inventory ─────────────────────────────────────────────

/// 根据 archetype / realm 生成 NPC 交易库存。
///
/// 分配规则（§8.1 #3）：
/// - `Commoner`: 1-2 件 Awaken 物品
/// - `Rogue`: 1-3 件，按 realm 解锁
/// - `Disciple`: 2-4 件，按 realm 解锁
/// - `GuardianRelic` / `Daoxiang` / `Zhinian` / `Beast` / `SkullFiend` / `Fuya` / `Zombie`: 不可交易
pub fn assign_npc_trade_inventory(
    archetype: NpcArchetype,
    realm: Realm,
    entity_seed: u64,
) -> NpcTradeInventory {
    let npc_rank = realm_rank(realm);

    let (offer_min, offer_max) = match archetype {
        NpcArchetype::Commoner => (1, 2),
        NpcArchetype::Rogue => (1, 3),
        NpcArchetype::Disciple => (2, 4),
        // 非交易 archetype
        NpcArchetype::GuardianRelic
        | NpcArchetype::Daoxiang
        | NpcArchetype::Zhinian
        | NpcArchetype::Beast
        | NpcArchetype::SkullFiend
        | NpcArchetype::Fuya
        | NpcArchetype::Zombie => {
            return NpcTradeInventory { offers: Vec::new() };
        }
    };

    // 过滤可用物品（Commoner 固定只卖 Awaken 级别物品，不随 realm 解锁）
    let effective_rank = match archetype {
        NpcArchetype::Commoner => 0,
        _ => npc_rank,
    };
    let available: Vec<&CatalogueEntry> = TRADE_CATALOGUE
        .iter()
        .filter(|entry| entry.realm_min <= effective_rank)
        .collect();

    if available.is_empty() {
        return NpcTradeInventory { offers: Vec::new() };
    }

    // 决定物品数量
    let range = (offer_max - offer_min + 1) as u32;
    let count = offer_min + splitmix64_range(entity_seed, range) as usize;
    let count = count.min(available.len());

    // Fisher-Yates shuffle 选前 count 个
    let mut indices: Vec<usize> = (0..available.len()).collect();
    for i in (1..indices.len()).rev() {
        let j = splitmix64_range(
            entity_seed.wrapping_add(i as u64 * 0x9E37_79B9),
            (i + 1) as u32,
        ) as usize;
        indices.swap(i, j);
    }

    let offers: Vec<TradeOffer> = indices
        .iter()
        .take(count)
        .enumerate()
        .map(|(idx, &orig_idx)| {
            let entry = available[orig_idx];
            let count_range = entry.count_max - entry.count_min + 1;
            let count_seed = entity_seed.wrapping_add(idx as u64 * 0xBF58_476D);
            let item_count = entry.count_min + splitmix64_range(count_seed, count_range);
            TradeOffer {
                template_id: entry.template_id.to_string(),
                display_name: entry.display_name.to_string(),
                count: item_count,
                price_bone_coins: entry.price_bone_coins,
            }
        })
        .collect();

    NpcTradeInventory { offers }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // === Non-trading archetypes return empty ===

    #[test]
    fn guardian_relic_returns_empty() {
        let inv = assign_npc_trade_inventory(NpcArchetype::GuardianRelic, Realm::Spirit, 42);
        assert!(
            inv.offers.is_empty(),
            "GuardianRelic should have no trade inventory"
        );
    }

    #[test]
    fn daoxiang_returns_empty() {
        let inv = assign_npc_trade_inventory(NpcArchetype::Daoxiang, Realm::Condense, 42);
        assert!(
            inv.offers.is_empty(),
            "Daoxiang should have no trade inventory"
        );
    }

    #[test]
    fn zhinian_returns_empty() {
        let inv = assign_npc_trade_inventory(NpcArchetype::Zhinian, Realm::Condense, 42);
        assert!(
            inv.offers.is_empty(),
            "Zhinian should have no trade inventory"
        );
    }

    #[test]
    fn beast_returns_empty() {
        let inv = assign_npc_trade_inventory(NpcArchetype::Beast, Realm::Awaken, 42);
        assert!(
            inv.offers.is_empty(),
            "Beast should have no trade inventory"
        );
    }

    #[test]
    fn skull_fiend_returns_empty() {
        let inv = assign_npc_trade_inventory(NpcArchetype::SkullFiend, Realm::Void, 42);
        assert!(
            inv.offers.is_empty(),
            "SkullFiend should have no trade inventory"
        );
    }

    #[test]
    fn fuya_returns_empty() {
        let inv = assign_npc_trade_inventory(NpcArchetype::Fuya, Realm::Spirit, 42);
        assert!(inv.offers.is_empty(), "Fuya should have no trade inventory");
    }

    #[test]
    fn zombie_returns_empty() {
        let inv = assign_npc_trade_inventory(NpcArchetype::Zombie, Realm::Awaken, 42);
        assert!(
            inv.offers.is_empty(),
            "Zombie should have no trade inventory"
        );
    }

    // === Trading archetypes produce correct counts ===

    #[test]
    fn commoner_returns_1_to_2() {
        for seed in 0..50u64 {
            let inv = assign_npc_trade_inventory(NpcArchetype::Commoner, Realm::Awaken, seed);
            assert!(
                !inv.offers.is_empty() && inv.offers.len() <= 2,
                "commoner should have 1-2 offers, got {} (seed={})",
                inv.offers.len(),
                seed
            );
            for offer in &inv.offers {
                assert!(offer.price_bone_coins > 0, "price should be positive");
                assert!(offer.count >= 1, "count should be at least 1");
                assert!(
                    !offer.template_id.is_empty(),
                    "template_id should not be empty"
                );
                assert!(
                    !offer.display_name.is_empty(),
                    "display_name should not be empty"
                );
            }
        }
    }

    #[test]
    fn rogue_returns_1_to_3() {
        for seed in 0..50u64 {
            let inv = assign_npc_trade_inventory(NpcArchetype::Rogue, Realm::Condense, seed);
            assert!(
                !inv.offers.is_empty() && inv.offers.len() <= 3,
                "rogue should have 1-3 offers, got {} (seed={})",
                inv.offers.len(),
                seed
            );
        }
    }

    #[test]
    fn disciple_returns_2_to_4() {
        for seed in 0..50u64 {
            let inv = assign_npc_trade_inventory(NpcArchetype::Disciple, Realm::Condense, seed);
            assert!(
                inv.offers.len() >= 2 && inv.offers.len() <= 4,
                "disciple should have 2-4 offers, got {} (seed={})",
                inv.offers.len(),
                seed
            );
        }
    }

    // === Realm gating ===

    #[test]
    fn awaken_only_gets_awaken_tier_items() {
        for seed in 0..100u64 {
            let inv = assign_npc_trade_inventory(NpcArchetype::Rogue, Realm::Awaken, seed);
            for offer in &inv.offers {
                let entry = TRADE_CATALOGUE
                    .iter()
                    .find(|e| e.template_id == offer.template_id)
                    .expect("offer should reference a catalogue entry");
                assert!(
                    entry.realm_min == 0,
                    "Awaken NPC should only get realm_min=0 items, got {} with realm_min={}",
                    offer.template_id,
                    entry.realm_min
                );
            }
        }
    }

    #[test]
    fn higher_realm_unlocks_more_items() {
        // Spirit realm should have access to more items than Awaken
        let awaken_available: Vec<_> = TRADE_CATALOGUE
            .iter()
            .filter(|e| e.realm_min == 0)
            .collect();
        let spirit_available: Vec<_> = TRADE_CATALOGUE
            .iter()
            .filter(|e| e.realm_min <= 4)
            .collect();
        assert!(
            spirit_available.len() > awaken_available.len(),
            "Spirit realm should unlock more items than Awaken ({} vs {})",
            spirit_available.len(),
            awaken_available.len()
        );
    }

    // === Determinism ===

    #[test]
    fn assign_trade_inventory_deterministic() {
        for archetype in [
            NpcArchetype::Commoner,
            NpcArchetype::Rogue,
            NpcArchetype::Disciple,
        ] {
            let a = assign_npc_trade_inventory(archetype, Realm::Condense, 12345);
            let b = assign_npc_trade_inventory(archetype, Realm::Condense, 12345);
            assert_eq!(
                a.offers.len(),
                b.offers.len(),
                "same seed should produce same count for {:?}",
                archetype
            );
            for (oa, ob) in a.offers.iter().zip(b.offers.iter()) {
                assert_eq!(
                    oa.template_id, ob.template_id,
                    "same seed should produce same items for {:?}",
                    archetype
                );
                assert_eq!(
                    oa.count, ob.count,
                    "same seed should produce same counts for {:?}",
                    archetype
                );
            }
        }
    }

    // === Count bounds within catalogue spec ===

    #[test]
    fn item_counts_within_catalogue_bounds() {
        for seed in 0..200u64 {
            let inv = assign_npc_trade_inventory(NpcArchetype::Rogue, Realm::Spirit, seed);
            for offer in &inv.offers {
                let entry = TRADE_CATALOGUE
                    .iter()
                    .find(|e| e.template_id == offer.template_id)
                    .expect("offer should reference a catalogue entry");
                assert!(
                    offer.count >= entry.count_min && offer.count <= entry.count_max,
                    "count {} for {} should be in [{}, {}]",
                    offer.count,
                    offer.template_id,
                    entry.count_min,
                    entry.count_max
                );
            }
        }
    }

    // === No duplicates ===

    #[test]
    fn no_duplicate_offers() {
        for seed in 0..100u64 {
            let inv = assign_npc_trade_inventory(NpcArchetype::Disciple, Realm::Spirit, seed);
            let mut seen = std::collections::HashSet::new();
            for offer in &inv.offers {
                assert!(
                    seen.insert(&offer.template_id),
                    "duplicate template_id {} in trade inventory (seed={})",
                    offer.template_id,
                    seed
                );
            }
        }
    }

    // === Price correctness ===

    #[test]
    fn prices_match_catalogue() {
        for seed in 0..100u64 {
            let inv = assign_npc_trade_inventory(NpcArchetype::Rogue, Realm::Spirit, seed);
            for offer in &inv.offers {
                let entry = TRADE_CATALOGUE
                    .iter()
                    .find(|e| e.template_id == offer.template_id)
                    .expect("offer should reference a catalogue entry");
                assert_eq!(
                    offer.price_bone_coins, entry.price_bone_coins,
                    "price for {} should match catalogue ({} vs {})",
                    offer.template_id, offer.price_bone_coins, entry.price_bone_coins
                );
            }
        }
    }

    // =========================================================================
    // P3 — NpcPlayerReputation
    // =========================================================================

    #[test]
    fn reputation_new_npc_default_mid() {
        let rep = NpcPlayerReputation::default();
        assert!(
            (rep.get("unknown") - 0.5).abs() < f32::EPSILON,
            "unknown player should get default 0.5, got {}",
            rep.get("unknown")
        );
        assert_eq!(
            rep.tier("unknown"),
            RepTier::Mid,
            "default 0.5 should map to Mid tier"
        );
    }

    #[test]
    fn reputation_trade_increases() {
        let mut rep = NpcPlayerReputation::default();
        rep.adjust("player:a", 0.05);
        let expected = 0.55;
        assert!(
            (rep.get("player:a") - expected).abs() < f32::EPSILON,
            "after +0.05 trade adjustment from 0.5, expected {}, got {}",
            expected,
            rep.get("player:a")
        );
    }

    #[test]
    fn reputation_attack_decreases() {
        let mut rep = NpcPlayerReputation::default();
        rep.adjust("player:b", -0.3);
        let expected = 0.2;
        assert!(
            (rep.get("player:b") - expected).abs() < f32::EPSILON,
            "after -0.3 attack adjustment from 0.5, expected {}, got {}",
            expected,
            rep.get("player:b")
        );
    }

    #[test]
    fn reputation_clamp_bounds() {
        let mut rep = NpcPlayerReputation::default();

        // Clamp upper bound
        rep.adjust("player:up", 1.0);
        assert!(
            (rep.get("player:up") - 1.0).abs() < f32::EPSILON,
            "reputation should clamp at 1.0, got {}",
            rep.get("player:up")
        );

        // Further increase should stay at 1.0
        rep.adjust("player:up", 0.5);
        assert!(
            (rep.get("player:up") - 1.0).abs() < f32::EPSILON,
            "reputation should stay at 1.0 after further increase, got {}",
            rep.get("player:up")
        );

        // Clamp lower bound
        rep.adjust("player:down", -1.0);
        assert!(
            (rep.get("player:down") - 0.0).abs() < f32::EPSILON,
            "reputation should clamp at 0.0, got {}",
            rep.get("player:down")
        );

        // Further decrease should stay at 0.0
        rep.adjust("player:down", -0.5);
        assert!(
            (rep.get("player:down") - 0.0).abs() < f32::EPSILON,
            "reputation should stay at 0.0 after further decrease, got {}",
            rep.get("player:down")
        );
    }

    #[test]
    fn reputation_tier_boundaries() {
        // 0.71 → High
        assert_eq!(
            RepTier::from_score(0.71),
            RepTier::High,
            "0.71 should be High tier (boundary: > 0.7)"
        );
        // 0.70 → Mid (boundary exact)
        assert_eq!(
            RepTier::from_score(0.70),
            RepTier::Mid,
            "0.70 should be Mid tier (not > 0.7)"
        );
        // 0.31 → Mid
        assert_eq!(
            RepTier::from_score(0.31),
            RepTier::Mid,
            "0.31 should be Mid tier (boundary: > 0.3)"
        );
        // 0.30 → Low (boundary exact)
        assert_eq!(
            RepTier::from_score(0.30),
            RepTier::Low,
            "0.30 should be Low tier (not > 0.3)"
        );
        // 0.11 → Low
        assert_eq!(
            RepTier::from_score(0.11),
            RepTier::Low,
            "0.11 should be Low tier (boundary: > 0.1)"
        );
        // 0.10 → Hostile (boundary exact)
        assert_eq!(
            RepTier::from_score(0.10),
            RepTier::Hostile,
            "0.10 should be Hostile tier (not > 0.1)"
        );
        // 0.09 → Hostile
        assert_eq!(
            RepTier::from_score(0.09),
            RepTier::Hostile,
            "0.09 should be Hostile tier"
        );
        // 0.0 → Hostile
        assert_eq!(
            RepTier::from_score(0.0),
            RepTier::Hostile,
            "0.0 should be Hostile tier"
        );
        // 1.0 → High
        assert_eq!(
            RepTier::from_score(1.0),
            RepTier::High,
            "1.0 should be High tier"
        );
    }

    #[test]
    fn reputation_multiple_players_independent() {
        let mut rep = NpcPlayerReputation::default();
        rep.adjust("player:alice", 0.2);
        rep.adjust("player:bob", -0.3);

        assert!(
            (rep.get("player:alice") - 0.7).abs() < f32::EPSILON,
            "alice should be 0.7, got {}",
            rep.get("player:alice")
        );
        assert!(
            (rep.get("player:bob") - 0.2).abs() < f32::EPSILON,
            "bob should be 0.2, got {}",
            rep.get("player:bob")
        );
        // Third player unaffected
        assert!(
            (rep.get("player:charlie") - 0.5).abs() < f32::EPSILON,
            "charlie should remain 0.5, got {}",
            rep.get("player:charlie")
        );
    }

    #[test]
    fn reputation_cumulative_adjustments() {
        let mut rep = NpcPlayerReputation::default();
        // 0.5 + 0.05 + 0.05 + 0.1 = 0.7
        rep.adjust("player:a", 0.05);
        rep.adjust("player:a", 0.05);
        rep.adjust("player:a", 0.1);
        assert!(
            (rep.get("player:a") - 0.7).abs() < 1e-6,
            "cumulative adjustments should sum correctly, expected 0.7, got {}",
            rep.get("player:a")
        );
    }

    // =========================================================================
    // P3 — DynamicPricing
    // =========================================================================

    #[test]
    fn pricing_zone_qi_low_increases_price() {
        let config = TradePricingConfig::default();
        let price = DynamicPricing::compute_price(100, 0.2, 0.5, 0.5, 0.0, &config);
        // zone_mod=1.5, rep_mod=1.0, scarcity=1.0, urgency=1.0 → 100*1.5=150
        assert_eq!(
            price, 150,
            "low zone_qi (0.2) should multiply price by 1.5: expected 150, got {}",
            price
        );
    }

    #[test]
    fn pricing_zone_qi_high_decreases_price() {
        let config = TradePricingConfig::default();
        let price = DynamicPricing::compute_price(100, 0.8, 0.5, 0.5, 0.0, &config);
        // zone_mod=0.8, rep_mod=1.0, scarcity=1.0, urgency=1.0 → 100*0.8=80
        assert_eq!(
            price, 80,
            "high zone_qi (0.8) should multiply price by 0.8: expected 80, got {}",
            price
        );
    }

    #[test]
    fn pricing_zone_qi_mid_no_effect() {
        let config = TradePricingConfig::default();
        let price = DynamicPricing::compute_price(100, 0.5, 0.5, 0.5, 0.0, &config);
        assert_eq!(
            price, 100,
            "mid zone_qi (0.5) should have no effect: expected 100, got {}",
            price
        );
    }

    #[test]
    fn pricing_high_reputation_discount() {
        let config = TradePricingConfig::default();
        let price = DynamicPricing::compute_price(100, 0.5, 0.5, 0.9, 0.0, &config);
        // zone_mod=1.0, rep_mod=0.85, scarcity=1.0, urgency=1.0 → 100*0.85=85
        assert_eq!(
            price, 85,
            "high reputation (0.9) should give 15% discount: expected 85, got {}",
            price
        );
    }

    #[test]
    fn pricing_low_reputation_markup() {
        let config = TradePricingConfig::default();
        let price = DynamicPricing::compute_price(100, 0.5, 0.5, 0.2, 0.0, &config);
        // zone_mod=1.0, rep_mod=1.3, scarcity=1.0, urgency=1.0 → 100*1.3=130
        assert_eq!(
            price, 130,
            "low reputation (0.2) should markup 30%: expected 130, got {}",
            price
        );
    }

    #[test]
    fn pricing_npc_urgent_sells_cheap() {
        let config = TradePricingConfig::default();
        let price = DynamicPricing::compute_price(100, 0.5, 0.1, 0.5, 0.0, &config);
        // zone_mod=1.0, rep_mod=1.0, scarcity=1.0, urgency=0.7 → 100*0.7=70
        assert_eq!(
            price, 70,
            "urgent NPC (qi_need=0.1) should sell at 70%: expected 70, got {}",
            price
        );
    }

    #[test]
    fn pricing_npc_not_urgent_no_discount() {
        let config = TradePricingConfig::default();
        let price = DynamicPricing::compute_price(100, 0.5, 0.3, 0.5, 0.0, &config);
        // urgency_mod = 1.0 because npc_qi >= 0.2
        assert_eq!(
            price, 100,
            "non-urgent NPC (qi_need=0.3) should have no urgency discount: expected 100, got {}",
            price
        );
    }

    #[test]
    fn pricing_price_floor_enforced() {
        let config = TradePricingConfig::default();
        // High zone_qi + High rep + urgent → 0.8 * 0.85 * 0.7 = 0.476 → 47.6
        // But floor = 100 * 0.3 = 30, so result should be max(30, 47.6) = 48 (rounded)
        let price = DynamicPricing::compute_price(100, 0.8, 0.1, 0.9, 0.0, &config);
        let floor = (100.0 * config.floor_ratio).round() as u32;
        assert!(
            price >= floor,
            "price {} should be >= floor {}: base=100, zone_qi=0.8, npc_qi=0.1, rep=0.9",
            price,
            floor
        );
    }

    #[test]
    fn pricing_price_floor_boundary() {
        let config = TradePricingConfig::default();
        // base=10, floor = 10 * 0.3 = 3
        // Even with maximum discounts, price should be >= 3
        let price = DynamicPricing::compute_price(10, 0.8, 0.1, 0.9, 0.0, &config);
        assert!(
            price >= 3,
            "price for base=10 should be >= floor 3, got {}",
            price
        );
    }

    #[test]
    fn pricing_minimum_one() {
        let config = TradePricingConfig {
            floor_ratio: 0.0, // disable floor to test min=1 boundary
            ..TradePricingConfig::default()
        };
        let price = DynamicPricing::compute_price(1, 0.8, 0.1, 0.9, 0.0, &config);
        assert!(
            price >= 1,
            "minimum price should always be 1, got {}",
            price
        );
    }

    #[test]
    fn pricing_scarce_item_no_discount() {
        let config = TradePricingConfig::default();
        // scarcity=1.0 → scarcity_mod = max(1.0, 1.0 + 1.0 * 0.5) = 1.5
        let price = DynamicPricing::compute_price(100, 0.5, 0.5, 0.5, 1.0, &config);
        assert!(
            price >= 100,
            "scarce item (scarcity=1.0) should only markup, never below base: expected >=100, got {}",
            price
        );
        assert_eq!(
            price, 150,
            "scarcity=1.0 should add 50% markup: expected 150, got {}",
            price
        );
    }

    #[test]
    fn pricing_scarcity_moderate() {
        let config = TradePricingConfig::default();
        // scarcity=0.5 → scarcity_mod = max(1.0, 1.0 + 0.5 * 0.5) = 1.25
        let price = DynamicPricing::compute_price(100, 0.5, 0.5, 0.5, 0.5, &config);
        assert_eq!(
            price, 125,
            "scarcity=0.5 should add 25% markup: expected 125, got {}",
            price
        );
    }

    #[test]
    fn pricing_zero_scarcity_no_markup() {
        let config = TradePricingConfig::default();
        let price = DynamicPricing::compute_price(100, 0.5, 0.5, 0.5, 0.0, &config);
        assert_eq!(
            price, 100,
            "zero scarcity should have no markup: expected 100, got {}",
            price
        );
    }

    #[test]
    fn pricing_default_config_values() {
        let config = TradePricingConfig::default();
        assert!(
            (config.zone_qi_low_threshold - 0.3).abs() < f32::EPSILON,
            "default zone_qi_low_threshold should be 0.3"
        );
        assert!(
            (config.zone_qi_high_threshold - 0.6).abs() < f32::EPSILON,
            "default zone_qi_high_threshold should be 0.6"
        );
        assert!(
            (config.zone_qi_low_multiplier - 1.5).abs() < f32::EPSILON,
            "default zone_qi_low_multiplier should be 1.5"
        );
        assert!(
            (config.zone_qi_high_multiplier - 0.8).abs() < f32::EPSILON,
            "default zone_qi_high_multiplier should be 0.8"
        );
        assert!(
            (config.rep_high_discount - 0.85).abs() < f32::EPSILON,
            "default rep_high_discount should be 0.85"
        );
        assert!(
            (config.rep_low_markup - 1.3).abs() < f32::EPSILON,
            "default rep_low_markup should be 1.3"
        );
        assert!(
            (config.floor_ratio - 0.3).abs() < f32::EPSILON,
            "default floor_ratio should be 0.3"
        );
    }

    #[test]
    fn pricing_combined_modifiers() {
        let config = TradePricingConfig::default();
        // Low zone qi + low rep + scarce → 1.5 * 1.3 * 1.5 * 1.0 = 2.925 → 293
        let price = DynamicPricing::compute_price(100, 0.2, 0.5, 0.2, 1.0, &config);
        let expected = (100.0_f32 * 1.5 * 1.3 * 1.5 * 1.0).round() as u32;
        assert_eq!(
            price, expected,
            "combined low_qi + low_rep + scarce: expected {}, got {}",
            expected, price
        );
    }

    // =========================================================================
    // P3 — TradeEligibility
    // =========================================================================

    #[test]
    fn eligibility_high_tier_discount() {
        let e = check_trade_eligibility(RepTier::High);
        assert_eq!(
            e,
            TradeEligibility::Allowed {
                price_modifier: 0.85
            },
            "High tier should allow trade with 0.85 discount"
        );
    }

    #[test]
    fn eligibility_mid_tier_normal() {
        let e = check_trade_eligibility(RepTier::Mid);
        assert_eq!(
            e,
            TradeEligibility::Allowed {
                price_modifier: 1.0
            },
            "Mid tier should allow trade with 1.0 modifier"
        );
    }

    #[test]
    fn eligibility_low_tier_refuse_rare() {
        let e = check_trade_eligibility(RepTier::Low);
        assert_eq!(
            e,
            TradeEligibility::RefuseRare,
            "Low tier should refuse rare items"
        );
    }

    #[test]
    fn eligibility_hostile_refused() {
        let e = check_trade_eligibility(RepTier::Hostile);
        assert_eq!(
            e,
            TradeEligibility::Refused,
            "Hostile tier should refuse all trade"
        );
    }

    #[test]
    fn eligibility_extreme_low_refuses_all() {
        let mut rep = NpcPlayerReputation::default();
        rep.adjust("player:hostile", -0.5); // 0.5 - 0.5 = 0.0 → Hostile
        let tier = rep.tier("player:hostile");
        assert_eq!(tier, RepTier::Hostile, "score 0.0 should be Hostile");
        let e = check_trade_eligibility(tier);
        assert_eq!(
            e,
            TradeEligibility::Refused,
            "Hostile tier from extreme low reputation should refuse all trade"
        );
    }

    // =========================================================================
    // P3 — InformationOffer
    // =========================================================================

    #[test]
    fn info_accuracy_correlates_reputation_high() {
        // High tier: accuracy in [0.8, 1.0]
        for seed in 0..50u64 {
            let offers = generate_info_offers("test_zone", RepTier::High, seed);
            for offer in &offers {
                assert!(
                    offer.accuracy >= 0.8 && offer.accuracy <= 1.0,
                    "High tier accuracy should be in [0.8, 1.0], got {} (seed={})",
                    offer.accuracy,
                    seed
                );
            }
        }
    }

    #[test]
    fn info_accuracy_correlates_reputation_mid() {
        // Mid tier: accuracy in [0.5, 0.8]
        for seed in 0..50u64 {
            let offers = generate_info_offers("test_zone", RepTier::Mid, seed);
            for offer in &offers {
                assert!(
                    offer.accuracy >= 0.5 && offer.accuracy <= 0.8,
                    "Mid tier accuracy should be in [0.5, 0.8], got {} (seed={})",
                    offer.accuracy,
                    seed
                );
            }
        }
    }

    #[test]
    fn info_accuracy_correlates_reputation_low() {
        // Low tier: accuracy in [0.3, 0.6]
        for seed in 0..50u64 {
            let offers = generate_info_offers("test_zone", RepTier::Low, seed);
            for offer in &offers {
                assert!(
                    offer.accuracy >= 0.3 && offer.accuracy <= 0.6,
                    "Low tier accuracy should be in [0.3, 0.6], got {} (seed={})",
                    offer.accuracy,
                    seed
                );
            }
        }
    }

    #[test]
    fn info_kind_all_variants() {
        // Verify all 4 InfoKind variants are constructible
        let zone_qi = InfoKind::ZoneQiLevel {
            zone_name: "test".to_string(),
            qi_value: 0.5,
        };
        let danger = InfoKind::DangerWarning {
            zone_name: "test".to_string(),
            threat_desc: "danger".to_string(),
        };
        let resource = InfoKind::ResourceLocation {
            zone_name: "test".to_string(),
            resource: "ore".to_string(),
        };
        let sighting = InfoKind::NpcSighting {
            target_desc: "rogue".to_string(),
            last_zone: "test".to_string(),
        };

        // Verify they are distinct
        assert_ne!(
            zone_qi, danger,
            "ZoneQiLevel and DangerWarning should differ"
        );
        assert_ne!(
            resource, sighting,
            "ResourceLocation and NpcSighting should differ"
        );
        assert_ne!(
            zone_qi, resource,
            "ZoneQiLevel and ResourceLocation should differ"
        );
    }

    #[test]
    fn info_generate_produces_bounded() {
        let mut any_zero = false;
        let mut any_nonzero = false;
        for seed in 0..200u64 {
            let offers = generate_info_offers("test_zone", RepTier::Mid, seed);
            assert!(
                offers.len() <= 2,
                "generate_info_offers should produce 0-2 items, got {} (seed={})",
                offers.len(),
                seed
            );
            if offers.is_empty() {
                any_zero = true;
            } else {
                any_nonzero = true;
            }
            for offer in &offers {
                assert!(
                    offer.price_bone_coins >= 5 && offer.price_bone_coins <= 20,
                    "price should be in [5, 20], got {} (seed={})",
                    offer.price_bone_coins,
                    seed
                );
                assert!(
                    offer.expiry_ticks == 24_000,
                    "expiry should be 24000, got {} (seed={})",
                    offer.expiry_ticks,
                    seed
                );
            }
        }
        assert!(
            any_zero,
            "over 200 seeds, should see at least one empty result"
        );
        assert!(
            any_nonzero,
            "over 200 seeds, should see at least one non-empty result"
        );
    }

    #[test]
    fn info_generate_deterministic() {
        let a = generate_info_offers("zone_a", RepTier::High, 12345);
        let b = generate_info_offers("zone_a", RepTier::High, 12345);
        assert_eq!(a.len(), b.len(), "same seed should produce same count");
        for (oa, ob) in a.iter().zip(b.iter()) {
            assert_eq!(
                oa.info_kind, ob.info_kind,
                "same seed should produce same info_kind"
            );
            assert_eq!(
                oa.price_bone_coins, ob.price_bone_coins,
                "same seed should produce same price"
            );
            assert!(
                (oa.accuracy - ob.accuracy).abs() < f32::EPSILON,
                "same seed should produce same accuracy"
            );
        }
    }

    // =========================================================================
    // P3 — TradeAbortedByNpc
    // =========================================================================

    #[test]
    fn ambush_abort_event_fields() {
        let npc = Entity::from_raw(42);
        let player = Entity::from_raw(99);
        let event = TradeAbortedByNpc {
            npc,
            player,
            reason: TradeAbortReason::Ambush,
        };
        assert_eq!(
            event.reason,
            TradeAbortReason::Ambush,
            "TradeAbortedByNpc should carry Ambush reason"
        );
        assert_eq!(
            event.npc,
            Entity::from_raw(42),
            "npc entity should be preserved"
        );
        assert_eq!(
            event.player,
            Entity::from_raw(99),
            "player entity should be preserved"
        );
    }

    #[test]
    fn hostile_reputation_abort_event_fields() {
        let event = TradeAbortedByNpc {
            npc: Entity::from_raw(1),
            player: Entity::from_raw(2),
            reason: TradeAbortReason::HostileReputation,
        };
        assert_eq!(
            event.reason,
            TradeAbortReason::HostileReputation,
            "TradeAbortedByNpc should carry HostileReputation reason"
        );
    }

    // =========================================================================
    // P3 — ReputationGossipEvent
    // =========================================================================

    #[test]
    fn gossip_event_construction() {
        let event = ReputationGossipEvent {
            source_npc: Entity::from_raw(10),
            target_player_id: "offline:Azure".to_string(),
            delta: -0.05,
            hops_remaining: 3,
        };
        assert_eq!(event.hops_remaining, 3, "initial hops should be 3");
        assert!(
            (event.delta - (-0.05)).abs() < f32::EPSILON,
            "delta should be -0.05"
        );
    }

    #[test]
    fn pending_gossip_countdown() {
        let mut pending = PendingGossip::default();
        pending.entries.push(PendingGossipEntry {
            target_player_id: "player:test".to_string(),
            delta: -0.05,
            hops_remaining: 2,
            source_npc: Entity::from_raw(1),
            remaining_ticks: 3,
        });

        // Tick down manually
        pending.entries[0].remaining_ticks -= 1;
        assert_eq!(
            pending.entries[0].remaining_ticks, 2,
            "after 1 tick, remaining should be 2"
        );

        pending.entries[0].remaining_ticks -= 1;
        assert_eq!(
            pending.entries[0].remaining_ticks, 1,
            "after 2 ticks, remaining should be 1"
        );

        pending.entries[0].remaining_ticks -= 1;
        assert_eq!(
            pending.entries[0].remaining_ticks, 0,
            "after 3 ticks, remaining should be 0 (ready to emit)"
        );
    }

    #[test]
    fn gossip_decay_formula() {
        // Verify the decay formula: delta * 0.5^(GOSSIP_INITIAL_HOPS - hops_remaining)
        let base_delta: f32 = -0.1;

        // Hop 3 (first emission): 0.5^(3-3) = 0.5^0 = 1.0
        let eff_0 = base_delta * 0.5_f32.powf((GOSSIP_INITIAL_HOPS - 3) as f32);
        assert!(
            (eff_0 - base_delta).abs() < f32::EPSILON,
            "at hop 3, effective delta should equal base delta: expected {}, got {}",
            base_delta,
            eff_0
        );

        // Hop 2: 0.5^(3-2) = 0.5^1 = 0.5
        let eff_1 = base_delta * 0.5_f32.powf((GOSSIP_INITIAL_HOPS - 2) as f32);
        assert!(
            (eff_1 - base_delta * 0.5).abs() < f32::EPSILON,
            "at hop 2, effective delta should be half: expected {}, got {}",
            base_delta * 0.5,
            eff_1
        );

        // Hop 1: 0.5^(3-1) = 0.5^2 = 0.25
        let eff_2 = base_delta * 0.5_f32.powf((GOSSIP_INITIAL_HOPS - 1) as f32);
        assert!(
            (eff_2 - base_delta * 0.25).abs() < f32::EPSILON,
            "at hop 1, effective delta should be quarter: expected {}, got {}",
            base_delta * 0.25,
            eff_2
        );

        // Hop 0: 0.5^(3-0) = 0.5^3 = 0.125
        let eff_3 = base_delta * 0.5_f32.powf(GOSSIP_INITIAL_HOPS as f32);
        assert!(
            (eff_3 - base_delta * 0.125).abs() < f32::EPSILON,
            "at hop 0, effective delta should be 1/8: expected {}, got {}",
            base_delta * 0.125,
            eff_3
        );
    }

    #[test]
    fn gossip_constants() {
        assert_eq!(
            GOSSIP_PROPAGATION_RADIUS, 48.0,
            "gossip propagation radius should be 48 blocks"
        );
        assert_eq!(GOSSIP_DELAY_TICKS, 120, "gossip delay should be 120 ticks");
        assert_eq!(GOSSIP_INITIAL_HOPS, 3, "gossip initial hops should be 3");
    }

    // =========================================================================
    // P3 — RepTier enum completeness
    // =========================================================================

    #[test]
    fn rep_tier_all_variants_reachable() {
        // Ensure all 4 variants are reachable from from_score
        let variants = [
            (1.0, RepTier::High),
            (0.5, RepTier::Mid),
            (0.2, RepTier::Low),
            (0.0, RepTier::Hostile),
        ];
        for (score, expected) in variants {
            assert_eq!(
                RepTier::from_score(score),
                expected,
                "score {} should map to {:?}",
                score,
                expected
            );
        }
    }

    // =========================================================================
    // P3 — NpcPlayerReputation + DynamicPricing integration
    // =========================================================================

    #[test]
    fn reputation_to_pricing_integration() {
        let mut rep = NpcPlayerReputation::default();
        let config = TradePricingConfig::default();

        // Default (Mid) → normal price
        let price_mid =
            DynamicPricing::compute_price(100, 0.5, 0.5, rep.get("player:a"), 0.0, &config);
        assert_eq!(price_mid, 100, "mid reputation should give normal price");

        // High reputation → discount
        rep.adjust("player:a", 0.25); // 0.75 → High
        let price_high =
            DynamicPricing::compute_price(100, 0.5, 0.5, rep.get("player:a"), 0.0, &config);
        assert_eq!(price_high, 85, "high reputation should give 15% discount");

        // Low reputation → markup
        rep.adjust("player:b", -0.25); // 0.25 → Low
        let price_low =
            DynamicPricing::compute_price(100, 0.5, 0.5, rep.get("player:b"), 0.0, &config);
        assert_eq!(price_low, 130, "low reputation should give 30% markup");
    }
}
