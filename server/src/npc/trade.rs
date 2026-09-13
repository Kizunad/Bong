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

use serde::{Deserialize, Serialize};
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
#[derive(Component, Default, Clone, Debug, PartialEq, Serialize, Deserialize)]
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

    /// Returns true when this component still only represents implicit neutral defaults.
    pub fn is_empty(&self) -> bool {
        self.scores.is_empty()
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

/// 交易物品候选库——单一真相来源，与 `npc_trade_catalog_entry`（买路）完全对齐。
///
/// 规则：
/// - template_id / price_bone_coins 必须与 `npc_trade_catalog_entry` 保持一致，两处同步修改。
/// - 仅列入 `server/assets/items/*.toml` 中存在对应条目的物品，避免 GUI 展示但买路失败的孤岛。
/// - 旧版条目（lingcao/bone_meal/rough_bandage/fragment_scroll/qi_condensing_powder/
///   spirit_stone_shard/bone_reinforcing_pill/spirit_jade）已移除——无 asset 支撑
///   或与买路 template_id / 价格不对齐，留在展示层会导致"能看不能买"或价格不一致。
const TRADE_CATALOGUE: &[CatalogueEntry] = &[
    // Awaken tier — Commoner + Rogue
    CatalogueEntry {
        template_id: "spirit_grass",
        display_name: "灵草",
        realm_min: 0,
        price_bone_coins: 10,
        count_min: 1,
        count_max: 5,
    },
    CatalogueEntry {
        template_id: "ling_xi_wan_flawed",
        display_name: "灵息丸（次品）",
        realm_min: 0,
        price_bone_coins: 8,
        count_min: 1,
        count_max: 3,
    },
    CatalogueEntry {
        template_id: "ju_ling_dan_flawed",
        display_name: "聚灵丹（次品）",
        realm_min: 0,
        price_bone_coins: 15,
        count_min: 1,
        count_max: 3,
    },
    // Induce tier — Rogue only
    CatalogueEntry {
        template_id: "skill_scroll_herbalism_baicao_can",
        display_name: "《百草图考·残》",
        realm_min: 1,
        price_bone_coins: 30,
        count_min: 1,
        count_max: 2,
    },
    CatalogueEntry {
        template_id: "broken_artifact_scroll",
        display_name: "残卷",
        realm_min: 1,
        price_bone_coins: 40,
        count_min: 1,
        count_max: 2,
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
        // plan-dying-elder-v1：垂死大能走专属 GiveDan 交互（P1），不走通用 trade
        NpcArchetype::GuardianRelic
        | NpcArchetype::Daoxiang
        | NpcArchetype::Zhinian
        | NpcArchetype::Beast
        | NpcArchetype::SkullFiend
        | NpcArchetype::Fuya
        | NpcArchetype::Zombie
        | NpcArchetype::DyingElder
        // plan-mundane-fauna-v1 P0：凡兽不可交易（动物无交易行为）。
        | NpcArchetype::Mundane => {
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
#[path = "trade_tests.rs"]
mod tests;
