//! 妖兽死亡掉落链路（plan-fauna-v1 §3 / §7 P0 + P4）。

use valence::prelude::{
    Commands, Entity, EventReader, EventWriter, Position, Query, Res, ResMut, With,
};

use crate::combat::events::{
    ApplyStatusEffectIntent, DeathEvent, StatusEffectKind, HALLUCINATION_DURATION_TICKS,
};
use crate::inventory::{
    DroppedLootEntry, DroppedLootRegistry, InventoryInstanceIdAllocator, ItemInstance, ItemRegistry,
};
use crate::npc::lifecycle::NpcArchetype;
use crate::npc::spawn::NpcMarker;
use crate::shelflife::{DecayProfileRegistry, Freshness};
use crate::world::dimension::{CurrentDimension, DimensionKind};

use super::components::{BeastKind, FaunaDropIssued, FaunaTag};

type FaunaDropNpcQuery<'w, 's> = Query<
    'w,
    's,
    (
        Option<&'static FaunaTag>,
        Option<&'static NpcArchetype>,
        &'static Position,
        Option<&'static CurrentDimension>,
        Option<&'static FaunaDropIssued>,
    ),
    With<NpcMarker>,
>;

pub const SHU_GU: &str = "shu_gu";
pub const ZHU_GU: &str = "zhu_gu";
pub const FENG_HE_GU: &str = "feng_he_gu";
pub const YI_SHOU_GU: &str = "yi_shou_gu";
pub const BIAN_YI_HEXIN: &str = "bian_yi_hexin";
pub const FU_YA_HESUI: &str = "fu_ya_hesui";
pub const ZHEN_SHI_CHU: &str = "zhen_shi_chu";
// 飞鲸（神兽级）专属掉落 ID —— 用于 WHALE_DROPS 表 + ItemRegistry 查询
pub const JING_GU: &str = "jing_gu";
pub const JING_SUI: &str = "jing_sui";
pub const JING_HUN_YU: &str = "jing_hun_yu";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuantityRange {
    pub min: u32,
    pub max: u32,
}

impl QuantityRange {
    pub const fn fixed(count: u32) -> Self {
        Self {
            min: count,
            max: count,
        }
    }

    pub const fn between(min: u32, max: u32) -> Self {
        Self { min, max }
    }

    fn roll(self, seed: u64) -> u32 {
        let min = self.min.max(1);
        let max = self.max.max(min);
        let span = max - min + 1;
        min + (splitmix64_u32(seed) % span)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DropEntry {
    pub item_id: &'static str,
    pub quantity: QuantityRange,
    pub probability: f32,
}

impl DropEntry {
    pub const fn guaranteed(item_id: &'static str, quantity: QuantityRange) -> Self {
        Self {
            item_id,
            quantity,
            probability: 1.0,
        }
    }

    pub const fn rare(item_id: &'static str, quantity: QuantityRange, probability: f32) -> Self {
        Self {
            item_id,
            quantity,
            probability,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RolledFaunaDrop {
    pub item_id: &'static str,
    pub quantity: u32,
}

const RAT_DROPS: [DropEntry; 2] = [
    DropEntry::guaranteed(SHU_GU, QuantityRange::between(1, 3)),
    DropEntry::guaranteed(YI_SHOU_GU, QuantityRange::fixed(1)),
];

const SPIDER_DROPS: [DropEntry; 3] = [
    DropEntry::guaranteed(ZHU_GU, QuantityRange::between(1, 2)),
    DropEntry::guaranteed(YI_SHOU_GU, QuantityRange::fixed(1)),
    DropEntry::rare(ZHEN_SHI_CHU, QuantityRange::fixed(1), 0.05),
];

const HYBRID_DROPS: [DropEntry; 3] = [
    DropEntry::guaranteed(FENG_HE_GU, QuantityRange::between(2, 4)),
    DropEntry::guaranteed(YI_SHOU_GU, QuantityRange::fixed(1)),
    DropEntry::rare(BIAN_YI_HEXIN, QuantityRange::fixed(1), 0.08),
];

const VOID_DISTORTED_DROPS: [DropEntry; 4] = [
    DropEntry::guaranteed(FENG_HE_GU, QuantityRange::between(3, 5)),
    DropEntry::guaranteed(FU_YA_HESUI, QuantityRange::fixed(1)),
    DropEntry::guaranteed(YI_SHOU_GU, QuantityRange::fixed(1)),
    DropEntry::rare(BIAN_YI_HEXIN, QuantityRange::fixed(1), 0.20),
];

/// 飞行鲸：神兽级中立巨型生物，化虚境界以上才打得动（HP=800）。
/// 专属掉落池（不复用 spider/void 系材料，独立"鲸"系列）：
/// - 异兽骨 8-15：杂骨保底量大
/// - 苍鲸脊骨 2-4：鲸专属保底，炼器极品 (legendary)
/// - 鲸髓凝液 ×1 保底：髓液，破境/延寿用 (legendary)
/// - 鲸魂玉珏 30%：灵识凝玉，化虚悟性 (legendary)
/// - 变异核心 20%：异化兽核心，破境跳板（其他妖兽也掉，但鲸级稀有度 20% 高于平均）
const WHALE_DROPS: [DropEntry; 5] = [
    DropEntry::guaranteed(YI_SHOU_GU, QuantityRange::between(8, 15)),
    DropEntry::guaranteed(JING_GU, QuantityRange::between(2, 4)),
    DropEntry::guaranteed(JING_SUI, QuantityRange::fixed(1)),
    DropEntry::rare(JING_HUN_YU, QuantityRange::fixed(1), 0.30),
    DropEntry::rare(BIAN_YI_HEXIN, QuantityRange::fixed(1), 0.20),
];

pub fn drop_table_for(kind: BeastKind) -> &'static [DropEntry] {
    match kind {
        BeastKind::Rat => &RAT_DROPS,
        BeastKind::Spider => &SPIDER_DROPS,
        BeastKind::HybridBeast => &HYBRID_DROPS,
        BeastKind::VoidDistorted => &VOID_DISTORTED_DROPS,
        BeastKind::Whale => &WHALE_DROPS,
    }
}

pub fn roll_fauna_drops(tag: FaunaTag, seed: u64) -> Vec<RolledFaunaDrop> {
    let mut out = Vec::new();
    for (idx, entry) in drop_table_for(tag.beast_kind).iter().enumerate() {
        let idx_seed = seed.wrapping_add((idx as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15));
        let chance = (entry.probability * tag.variant.rare_drop_multiplier()).clamp(0.0, 1.0);
        if splitmix64_unit(idx_seed) > chance {
            continue;
        }
        out.push(RolledFaunaDrop {
            item_id: entry.item_id,
            quantity: entry.quantity.roll(idx_seed.rotate_left(17)),
        });
    }
    out
}

#[allow(clippy::too_many_arguments)]
pub fn fauna_drop_system(
    mut commands: Commands,
    mut deaths: EventReader<DeathEvent>,
    npcs: FaunaDropNpcQuery<'_, '_>,
    item_registry: Option<Res<ItemRegistry>>,
    decay_profiles: Option<Res<DecayProfileRegistry>>,
    mut allocator: Option<ResMut<InventoryInstanceIdAllocator>>,
    mut loot_registry: Option<ResMut<DroppedLootRegistry>>,
    mut status_effects: EventWriter<ApplyStatusEffectIntent>,
) {
    let (Some(item_registry), Some(allocator), Some(loot_registry)) = (
        item_registry.as_deref(),
        allocator.as_deref_mut(),
        loot_registry.as_deref_mut(),
    ) else {
        return;
    };
    let decay_profiles = decay_profiles.as_deref();

    for event in deaths.read() {
        let Ok((tag, archetype, pos, dimension, issued)) = npcs.get(event.target) else {
            continue;
        };
        if issued.is_some() {
            continue;
        }
        let Some(tag) = tag.copied().or_else(|| fallback_tag(archetype.copied())) else {
            continue;
        };

        let seed = fauna_drop_seed(event.target, event.at_tick);
        let drops = roll_fauna_drops(tag, seed);
        let mut dropped_core = false;
        for (idx, drop) in drops.into_iter().enumerate() {
            let Ok(item) = build_fauna_item_instance(
                drop.item_id,
                drop.quantity,
                event.at_tick,
                item_registry,
                decay_profiles,
                allocator,
            ) else {
                tracing::warn!(
                    "[bong][fauna] drop `{}` skipped because item template/profile is missing",
                    drop.item_id
                );
                continue;
            };
            dropped_core |= item.template_id == BIAN_YI_HEXIN;
            let world_pos = jittered_drop_pos(pos.get(), seed, idx as u64);
            loot_registry.entries.insert(
                item.instance_id,
                DroppedLootEntry {
                    instance_id: item.instance_id,
                    source_container_id: format!("fauna_drop:{}", tag.beast_kind.as_str()),
                    source_row: 0,
                    source_col: 0,
                    world_pos,
                    dimension: dimension
                        .map(|dim| dim.0)
                        .unwrap_or(DimensionKind::Overworld),
                    item,
                },
            );
        }

        if dropped_core {
            if let Some(attacker) = event.attacker {
                status_effects.send(ApplyStatusEffectIntent {
                    target: attacker,
                    kind: StatusEffectKind::InsightHallucination,
                    magnitude: 0.35,
                    duration_ticks: HALLUCINATION_DURATION_TICKS,
                    issued_at_tick: event.at_tick,
                });
            }
        }

        commands.entity(event.target).insert(FaunaDropIssued);
    }
}

pub fn build_fauna_item_instance(
    template_id: &str,
    stack_count: u32,
    created_at_tick: u64,
    item_registry: &ItemRegistry,
    decay_profiles: Option<&DecayProfileRegistry>,
    allocator: &mut InventoryInstanceIdAllocator,
) -> Result<ItemInstance, String> {
    let template = item_registry
        .get(template_id)
        .ok_or_else(|| format!("unknown fauna item template `{template_id}`"))?;
    let freshness = freshness_for_template(template_id, created_at_tick, decay_profiles);
    Ok(ItemInstance {
        instance_id: allocator.next_id()?,
        template_id: template.id.clone(),
        display_name: template.display_name.clone(),
        grid_w: template.grid_w,
        grid_h: template.grid_h,
        weight: template.base_weight,
        rarity: template.rarity,
        description: template.description.clone(),
        stack_count: stack_count.max(1),
        spirit_quality: template.spirit_quality_initial,
        durability: 1.0,
        freshness,
        mineral_id: None,
        charges: None,
        forge_quality: None,
        forge_color: None,
        forge_side_effects: Vec::new(),
        forge_achieved_tier: None,
        alchemy: None,
        lingering_owner_qi: None,
    })
}

pub fn freshness_for_template(
    template_id: &str,
    created_at_tick: u64,
    decay_profiles: Option<&DecayProfileRegistry>,
) -> Option<Freshness> {
    let (profile_id, initial_qi) = freshness_profile_for_template(template_id)?;
    let profile = decay_profiles?.get(&crate::shelflife::DecayProfileId::new(profile_id))?;
    Some(Freshness::new(created_at_tick, initial_qi, profile))
}

pub fn freshness_profile_for_template(template_id: &str) -> Option<(&'static str, f32)> {
    match template_id {
        SHU_GU => Some(("fauna_bone_shu_gu_v1", 5.0)),
        ZHU_GU => Some(("fauna_bone_zhu_gu_v1", 15.0)),
        FENG_HE_GU => Some(("fauna_bone_feng_he_gu_v1", 40.0)),
        YI_SHOU_GU => Some(("fauna_bone_yi_shou_gu_v1", 20.0)),
        "bone_coin_5" => Some(("bone_coin_5_v1", 5.0)),
        "bone_coin_15" => Some(("bone_coin_15_v1", 15.0)),
        "bone_coin_40" => Some(("bone_coin_40_v1", 40.0)),
        "fengling_bone_coin" => Some(("bone_coin_v1", 10.0)),
        _ => None,
    }
}

fn fallback_tag(archetype: Option<NpcArchetype>) -> Option<FaunaTag> {
    match archetype? {
        NpcArchetype::Beast => Some(FaunaTag::new(BeastKind::Rat)),
        NpcArchetype::Fuya => Some(FaunaTag::new(BeastKind::VoidDistorted)),
        _ => None,
    }
}

pub fn fauna_drop_seed(entity: Entity, tick: u64) -> u64 {
    entity
        .to_bits()
        .rotate_left(23)
        .wrapping_add(tick.wrapping_mul(0x9E37_79B9_7F4A_7C15))
}

fn jittered_drop_pos(base: valence::prelude::DVec3, seed: u64, idx: u64) -> [f64; 3] {
    let x = splitmix64_unit(seed.wrapping_add(idx)) as f64 - 0.5;
    let z = splitmix64_unit(seed.wrapping_add(idx.rotate_left(11))) as f64 - 0.5;
    [base.x + x * 0.7, base.y, base.z + z * 0.7]
}

fn splitmix64_u32(seed: u64) -> u32 {
    (splitmix64(seed) >> 32) as u32
}

fn splitmix64_unit(seed: u64) -> f32 {
    let bits = ((splitmix64(seed) >> 40) & 0x00FF_FFFF) as u32;
    bits as f32 / (1u32 << 24) as f32
}

fn splitmix64(seed: u64) -> u64 {
    let mut x = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use valence::prelude::{App, Events, Update};

    use super::super::components::BeastVariant;
    use super::*;
    use crate::inventory::{
        dropped_loot_snapshot, pickup_dropped_loot_instance, ContainerState, InventoryRevision,
        ItemCategory, ItemRarity, ItemTemplate, PlayerInventory, MAIN_PACK_CONTAINER_ID,
    };
    use crate::npc::spawn::NpcMarker;

    fn template(id: &str) -> ItemTemplate {
        ItemTemplate {
            id: id.to_string(),
            display_name: id.to_string(),
            category: if id.starts_with("bone_coin") {
                ItemCategory::BoneCoin
            } else {
                ItemCategory::Misc
            },
            max_stack_count: if id.starts_with("bone_coin") {
                u32::MAX
            } else {
                16
            },
            grid_w: 1,
            grid_h: 1,
            base_weight: 0.1,
            rarity: ItemRarity::Common,
            spirit_quality_initial: 1.0,
            description: id.to_string(),
            effect: None,
            cast_duration_ms: crate::inventory::DEFAULT_CAST_DURATION_MS,
            cooldown_ms: crate::inventory::DEFAULT_COOLDOWN_MS,
            weapon_spec: None,
            forge_station_spec: None,
            blueprint_scroll_spec: None,
            inscription_scroll_spec: None,
        }
    }

    fn fauna_registry() -> ItemRegistry {
        let ids = [
            SHU_GU,
            ZHU_GU,
            FENG_HE_GU,
            YI_SHOU_GU,
            BIAN_YI_HEXIN,
            FU_YA_HESUI,
            ZHEN_SHI_CHU,
            JING_GU,
            JING_SUI,
            JING_HUN_YU,
            "bone_coin_5",
            "bone_coin_15",
            "bone_coin_40",
        ];
        ItemRegistry::from_map(
            ids.into_iter()
                .map(|id| (id.to_string(), template(id)))
                .collect::<HashMap<_, _>>(),
        )
    }

    fn empty_player_inventory() -> PlayerInventory {
        PlayerInventory {
            revision: InventoryRevision(1),
            containers: vec![ContainerState {
                id: MAIN_PACK_CONTAINER_ID.to_string(),
                name: "主背包".to_string(),
                rows: 5,
                cols: 7,
                items: Vec::new(),
            }],
            equipped: HashMap::new(),
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 50.0,
        }
    }

    #[test]
    fn each_beast_kind_rolls_guaranteed_bone_material() {
        for kind in [
            BeastKind::Rat,
            BeastKind::Spider,
            BeastKind::HybridBeast,
            BeastKind::VoidDistorted,
            BeastKind::Whale,
        ] {
            for seed in [1, 42, 99] {
                let drops = roll_fauna_drops(FaunaTag::new(kind), seed);
                assert!(
                    drops.iter().any(|drop| drop.item_id == YI_SHOU_GU),
                    "{kind:?} must emit canonical yi_shou_gu for seed {seed}"
                );
                assert!(
                    drops.iter().any(|drop| drop.quantity >= 1),
                    "{kind:?} must emit at least one positive stack"
                );
            }
        }
    }

    #[test]
    fn whale_drops_match_neutral_giant_design_table() {
        // 神兽级数值锁（饱和）：
        // - yi_shou_gu 量 [8, 15] 保底
        // - jing_gu 量 [2, 4] 保底（鲸专属脊骨）
        // - jing_sui ×1 保底
        // - jing_hun_yu 30% (rare)
        // - bian_yi_hexin 20% (rare)
        // - 不掉 fu_ya_hesui / zhen_shi_chu（鲸专属池，与其他妖兽稀有项错开）
        let mut yi_min = u32::MAX;
        let mut yi_max = 0u32;
        let mut jing_gu_min = u32::MAX;
        let mut jing_gu_max = 0u32;
        let mut jing_sui_hits = 0;
        let mut jing_hun_yu_hits = 0;
        let mut bian_yi_hits = 0;
        let mut fu_ya_hits = 0;
        let mut zhen_shi_hits = 0;
        const SAMPLES: u64 = 2000;
        for seed in 0..SAMPLES {
            let drops = roll_fauna_drops(FaunaTag::new(BeastKind::Whale), seed.wrapping_mul(31));
            let yi = drops
                .iter()
                .find(|d| d.item_id == YI_SHOU_GU)
                .expect("whale must always drop yi_shou_gu (guaranteed)");
            yi_min = yi_min.min(yi.quantity);
            yi_max = yi_max.max(yi.quantity);

            let jg = drops
                .iter()
                .find(|d| d.item_id == JING_GU)
                .expect("whale must always drop jing_gu (鲸专属脊骨保底)");
            jing_gu_min = jing_gu_min.min(jg.quantity);
            jing_gu_max = jing_gu_max.max(jg.quantity);

            if drops.iter().any(|d| d.item_id == JING_SUI) {
                jing_sui_hits += 1;
            }
            if drops.iter().any(|d| d.item_id == JING_HUN_YU) {
                jing_hun_yu_hits += 1;
            }
            if drops.iter().any(|d| d.item_id == BIAN_YI_HEXIN) {
                bian_yi_hits += 1;
            }
            if drops.iter().any(|d| d.item_id == FU_YA_HESUI) {
                fu_ya_hits += 1;
            }
            if drops.iter().any(|d| d.item_id == ZHEN_SHI_CHU) {
                zhen_shi_hits += 1;
            }
        }
        // yi_shou_gu 数量恰好 [8, 15]
        assert!(
            yi_min >= 8 && yi_max <= 15,
            "yi_shou_gu range observed [{yi_min}, {yi_max}], spec [8, 15]"
        );
        assert!(yi_max > yi_min, "rolls must span >1 unique value");
        // jing_gu 数量恰好 [2, 4]
        assert!(
            jing_gu_min >= 2 && jing_gu_max <= 4,
            "jing_gu range observed [{jing_gu_min}, {jing_gu_max}], spec [2, 4]"
        );
        // jing_sui 100% 出
        assert_eq!(
            jing_sui_hits, SAMPLES,
            "jing_sui must drop on every whale kill (guaranteed)"
        );
        // jing_hun_yu ~30%
        let jhy_rate = jing_hun_yu_hits as f64 / SAMPLES as f64;
        assert!(
            (0.25..=0.35).contains(&jhy_rate),
            "jing_hun_yu rate {jhy_rate:.3} should be ~0.30 (±0.05)"
        );
        // bian_yi_hexin ~20%
        let bian_yi_rate = bian_yi_hits as f64 / SAMPLES as f64;
        assert!(
            (0.16..=0.24).contains(&bian_yi_rate),
            "bian_yi_hexin rate {bian_yi_rate:.3} should be ~0.20 (±0.04)"
        );
        // 鲸专属池：绝不掉 fu_ya_hesui / zhen_shi_chu
        assert_eq!(fu_ya_hits, 0, "whale must NOT drop fu_ya_hesui (鲸专属池)");
        assert_eq!(
            zhen_shi_hits, 0,
            "whale must NOT drop zhen_shi_chu (鲸专属池)"
        );
    }

    #[test]
    fn variant_increases_rare_drop_rate_without_changing_guaranteed() {
        let normal = (0..500)
            .flat_map(|seed| roll_fauna_drops(FaunaTag::new(BeastKind::VoidDistorted), seed * 17))
            .filter(|drop| drop.item_id == BIAN_YI_HEXIN)
            .count();
        let tainted = (0..500)
            .flat_map(|seed| {
                roll_fauna_drops(
                    FaunaTag::with_variant(BeastKind::VoidDistorted, BeastVariant::Tainted),
                    seed * 17,
                )
            })
            .filter(|drop| drop.item_id == BIAN_YI_HEXIN)
            .count();
        assert!(tainted > normal, "tainted={tainted} normal={normal}");
    }

    #[test]
    fn fauna_item_instance_attaches_bone_freshness_when_profile_exists() {
        let registry = fauna_registry();
        let profiles = crate::shelflife::build_default_registry();
        let mut allocator = InventoryInstanceIdAllocator::new(100);
        let item = build_fauna_item_instance(
            FENG_HE_GU,
            2,
            77,
            &registry,
            Some(&profiles),
            &mut allocator,
        )
        .expect("template and profile should exist");

        assert_eq!(item.template_id, FENG_HE_GU);
        assert_eq!(item.stack_count, 2);
        let freshness = item.freshness.expect("bone drop should carry freshness");
        assert_eq!(freshness.profile.as_str(), "fauna_bone_feng_he_gu_v1");
        assert_eq!(freshness.initial_qi, 40.0);
        assert_eq!(freshness.created_at_tick, 77);
    }

    #[test]
    fn death_event_creates_dropped_loot_and_marks_target() {
        let mut app = App::new();
        app.add_event::<DeathEvent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.insert_resource(fauna_registry());
        app.insert_resource(crate::shelflife::build_default_registry());
        app.insert_resource(InventoryInstanceIdAllocator::new(10));
        app.insert_resource(DroppedLootRegistry::default());
        app.add_systems(Update, fauna_drop_system);

        let beast = app
            .world_mut()
            .spawn((
                NpcMarker,
                FaunaTag::new(BeastKind::Spider),
                Position::new([1.0, 64.0, 2.0]),
            ))
            .id();
        app.world_mut().send_event(DeathEvent {
            target: beast,
            cause: "test".to_string(),
            attacker: None,
            attacker_player_id: None,
            at_tick: 55,
        });

        app.update();

        let drops = app.world().resource::<DroppedLootRegistry>();
        assert!(
            drops
                .entries
                .values()
                .any(|entry| entry.item.template_id == ZHU_GU),
            "spider death should drop zhu_gu"
        );
        assert!(
            app.world().get::<FaunaDropIssued>(beast).is_some(),
            "processed beast should be marked to prevent duplicate drops"
        );
    }

    #[test]
    fn rat_kill_to_g_pickup_round_trip_creates_inventory_shu_gu() {
        let mut app = App::new();
        app.add_event::<DeathEvent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.insert_resource(fauna_registry());
        app.insert_resource(crate::shelflife::build_default_registry());
        app.insert_resource(InventoryInstanceIdAllocator::new(10));
        app.insert_resource(DroppedLootRegistry::default());
        app.add_systems(Update, fauna_drop_system);

        let rat = app
            .world_mut()
            .spawn((
                NpcMarker,
                FaunaTag::new(BeastKind::Rat),
                Position::new([0.0, 64.0, 0.0]),
            ))
            .id();
        app.world_mut().send_event(DeathEvent {
            target: rat,
            cause: "player_kill".to_string(),
            attacker: None,
            attacker_player_id: Some("offline:test-player".to_string()),
            at_tick: 55,
        });

        app.update();

        let (shu_gu_id, pickup_pos) = {
            let drops = app.world().resource::<DroppedLootRegistry>();
            let entry = drops
                .entries
                .values()
                .find(|entry| entry.item.template_id == SHU_GU)
                .expect("rat death should create a dropped shu_gu entry");
            (entry.instance_id, entry.world_pos)
        };
        let mut inventory = empty_player_inventory();
        {
            let mut registry = app.world_mut().resource_mut::<DroppedLootRegistry>();
            pickup_dropped_loot_instance(&mut inventory, &mut registry, pickup_pos, shu_gu_id)
                .expect("G pickup should move dropped shu_gu into inventory");
        }

        assert!(
            inventory
                .containers
                .iter()
                .flat_map(|container| container.items.iter())
                .any(|placed| {
                    placed.instance.template_id == SHU_GU && placed.instance.stack_count >= 1
                }),
            "picked-up player inventory should contain shu_gu"
        );
        let drops = app.world().resource::<DroppedLootRegistry>();
        assert!(
            !drops.entries.contains_key(&shu_gu_id),
            "G pickup should remove the shu_gu drop from DroppedLootRegistry"
        );
        assert!(
            dropped_loot_snapshot(drops)
                .iter()
                .all(|entry| entry.instance_id != shu_gu_id),
            "post-pickup dropped loot snapshot should no longer contain shu_gu"
        );
    }

    #[test]
    fn untagged_legacy_beast_falls_back_to_rat_table() {
        let mut app = App::new();
        app.add_event::<DeathEvent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.insert_resource(fauna_registry());
        app.insert_resource(crate::shelflife::build_default_registry());
        app.insert_resource(InventoryInstanceIdAllocator::new(10));
        app.insert_resource(DroppedLootRegistry::default());
        app.add_systems(Update, fauna_drop_system);

        let beast = app
            .world_mut()
            .spawn((
                NpcMarker,
                NpcArchetype::Beast,
                Position::new([1.0, 64.0, 2.0]),
            ))
            .id();
        app.world_mut().send_event(DeathEvent {
            target: beast,
            cause: "test".to_string(),
            attacker: None,
            attacker_player_id: None,
            at_tick: 55,
        });

        app.update();

        let dropped_templates = app
            .world()
            .resource::<DroppedLootRegistry>()
            .entries
            .values()
            .map(|entry| entry.item.template_id.clone())
            .collect::<Vec<_>>();
        assert!(
            dropped_templates
                .iter()
                .any(|template_id| template_id == SHU_GU),
            "legacy Beast fallback should use low-tier rat table"
        );
        assert!(
            !dropped_templates
                .iter()
                .any(|template_id| template_id == FENG_HE_GU),
            "legacy Beast fallback must not mint hybrid-tier bones"
        );
    }

    #[test]
    fn core_drop_applies_hallucination_to_attacker() {
        let mut app = App::new();
        app.add_event::<DeathEvent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.insert_resource(fauna_registry());
        app.insert_resource(crate::shelflife::build_default_registry());
        app.insert_resource(InventoryInstanceIdAllocator::new(10));
        app.insert_resource(DroppedLootRegistry::default());
        app.add_systems(Update, fauna_drop_system);

        let attacker = app.world_mut().spawn_empty().id();
        let beast = app
            .world_mut()
            .spawn((
                NpcMarker,
                FaunaTag::with_variant(BeastKind::VoidDistorted, BeastVariant::Tainted),
                Position::new([0.0, 64.0, 0.0]),
            ))
            .id();
        app.world_mut().send_event(DeathEvent {
            target: beast,
            cause: "test".to_string(),
            attacker: Some(attacker),
            attacker_player_id: None,
            at_tick: 159,
        });

        app.update();

        let effects = app.world().resource::<Events<ApplyStatusEffectIntent>>();
        let mut reader = effects.get_reader();
        let collected = reader.read(effects).collect::<Vec<_>>();
        if app
            .world()
            .resource::<DroppedLootRegistry>()
            .entries
            .values()
            .any(|entry| entry.item.template_id == BIAN_YI_HEXIN)
        {
            assert!(collected.iter().any(|event| {
                event.target == attacker && event.kind == StatusEffectKind::InsightHallucination
            }));
        }
    }
}
