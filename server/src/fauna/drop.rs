//! 妖兽死亡掉落链路（plan-fauna-v1 §3 / §7 P0 + P4）。

use valence::prelude::{
    Commands, Despawned, Entity, EventReader, EventWriter, Position, Query, Res, ResMut, With,
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
use super::mundane::{MundaneFaunaKind, MundaneFaunaSpecies};

type FaunaDropNpcQuery<'w, 's> = Query<
    'w,
    's,
    (
        Option<&'static FaunaTag>,
        Option<&'static NpcArchetype>,
        // plan-mundane-fauna-v1 P1：凡兽掉落分支查表依据——`MundaneFaunaSpecies` 挂在
        // `spawn_mundane_fauna_at`（`npc/spawn/mundane.rs`），妖兽（FaunaTag）没有此组件。
        Option<&'static MundaneFaunaSpecies>,
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
pub const JING_GU: &str = "jing_gu";
pub const JING_SUI: &str = "jing_sui";
pub const JING_HUN_YU: &str = "jing_hun_yu";

// ── 新增物种掉落 ID ──
pub const ZHU_SI: &str = "zhu_si";
pub const LV_ZHU_DUNANG: &str = "lv_zhu_dunang";
pub const XIE_KE: &str = "xie_ke";
pub const XIE_WEI_ZHEN: &str = "xie_wei_zhen";
pub const XIE_DU_XIAN: &str = "xie_du_xian";
pub const SHE_LIN: &str = "she_lin";
pub const SHE_DAN: &str = "she_dan";
pub const JIGUAN_SHE_GUAN: &str = "jiguan_she_guan";
pub const BING_ZHU_SI: &str = "bing_zhu_si";
pub const SHUANG_ZHU_HE: &str = "shuang_zhu_he";
pub const BINGBI_JIAPIAN: &str = "bingbi_jiapian";
pub const BINGBI_XIE_HE: &str = "bingbi_xie_he";
pub const SHE_YA: &str = "she_ya";
pub const MANTUOLUO_SHE_TONG: &str = "mantuoluo_she_tong";
pub const HU_GU: &str = "hu_gu";
pub const HU_PI: &str = "hu_pi";
pub const XIESHEN_HU_XIN: &str = "xieshen_hu_xin";
pub const LONG_GU: &str = "long_gu";
pub const LONG_LIN: &str = "long_lin";
pub const DU_LONG_ZHU: &str = "du_long_zhu";
pub const KU_LONG_GU: &str = "ku_long_gu";
pub const LONG_YA: &str = "long_ya";
pub const GU_LONG_HUN_JING: &str = "gu_long_hun_jing";
pub const ZHU_HE_SUIPIAN: &str = "zhu_he_suipian";
pub const CHU_XU: &str = "chu_xu";
pub const SHENYUAN_ZHI_YAN: &str = "shenyuan_zhi_yan";

// ── plan-mundane-fauna-v1 P1：凡兽掉落物 ID ──
pub const RAW_BEAST_MEAT: &str = "raw_beast_meat";
pub const RAW_BEAST_HIDE: &str = "raw_beast_hide";
pub const RAW_BEAST_BLOOD: &str = "raw_beast_blood";
pub const RABBIT_PELT: &str = "rabbit_pelt";
/// 凡骨——凡俗材料档，`bone_coin::bone_grade_for_template` 天然对未知 template_id 返回
/// `None`（不加档），故凡骨永远无法喂进封灵骨币制作（§8.1 正典硬约束：骨币料仅限异变兽骨）。
pub const FAN_GU: &str = "fan_gu";

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

const GREEN_SPIDER_DROPS: [DropEntry; 3] = [
    DropEntry::guaranteed(ZHU_GU, QuantityRange::between(1, 2)),
    DropEntry::guaranteed(ZHU_SI, QuantityRange::between(2, 4)),
    DropEntry::rare(LV_ZHU_DUNANG, QuantityRange::fixed(1), 0.08),
];

const JUNGLE_SCORPION_DROPS: [DropEntry; 3] = [
    DropEntry::guaranteed(XIE_KE, QuantityRange::between(1, 3)),
    DropEntry::guaranteed(XIE_WEI_ZHEN, QuantityRange::fixed(1)),
    DropEntry::rare(XIE_DU_XIAN, QuantityRange::fixed(1), 0.08),
];

const COCKADE_SNAKE_DROPS: [DropEntry; 3] = [
    DropEntry::guaranteed(SHE_LIN, QuantityRange::between(2, 4)),
    DropEntry::guaranteed(SHE_DAN, QuantityRange::fixed(1)),
    DropEntry::rare(JIGUAN_SHE_GUAN, QuantityRange::fixed(1), 0.08),
];

const BLUE_SPIDER_DROPS: [DropEntry; 3] = [
    DropEntry::guaranteed(ZHU_GU, QuantityRange::between(2, 3)),
    DropEntry::guaranteed(BING_ZHU_SI, QuantityRange::between(3, 5)),
    DropEntry::rare(SHUANG_ZHU_HE, QuantityRange::fixed(1), 0.10),
];

const ICE_SCORPION_DROPS: [DropEntry; 3] = [
    DropEntry::guaranteed(XIE_KE, QuantityRange::between(2, 4)),
    DropEntry::guaranteed(BINGBI_JIAPIAN, QuantityRange::between(1, 2)),
    DropEntry::rare(BINGBI_XIE_HE, QuantityRange::fixed(1), 0.12),
];

const MANDRAKE_SNAKE_DROPS: [DropEntry; 3] = [
    DropEntry::guaranteed(SHE_LIN, QuantityRange::between(3, 5)),
    DropEntry::guaranteed(SHE_YA, QuantityRange::between(1, 2)),
    DropEntry::rare(MANTUOLUO_SHE_TONG, QuantityRange::fixed(1), 0.12),
];

const DARK_TIGER_DROPS: [DropEntry; 4] = [
    DropEntry::guaranteed(HU_GU, QuantityRange::between(3, 5)),
    DropEntry::guaranteed(HU_PI, QuantityRange::fixed(1)),
    DropEntry::guaranteed(YI_SHOU_GU, QuantityRange::between(2, 3)),
    DropEntry::rare(XIESHEN_HU_XIN, QuantityRange::fixed(1), 0.15),
];

const LIVING_PILLAR_DROPS: [DropEntry; 3] = [
    DropEntry::guaranteed(ZHU_HE_SUIPIAN, QuantityRange::between(3, 5)),
    DropEntry::guaranteed(CHU_XU, QuantityRange::between(2, 4)),
    DropEntry::rare(SHENYUAN_ZHI_YAN, QuantityRange::fixed(1), 0.20),
];

const HEIWUSHI_DROPS: [DropEntry; 4] = [
    DropEntry::guaranteed("star_iron", QuantityRange::fixed(2)),
    DropEntry::guaranteed("sword_embryo_shard", QuantityRange::fixed(2)),
    DropEntry::rare("ancient_sword_embryo", QuantityRange::fixed(1), 0.30),
    DropEntry::rare("scroll_sword_manifest", QuantityRange::fixed(1), 0.10),
];

const POISON_DRAGON_DROPS: [DropEntry; 4] = [
    DropEntry::guaranteed(LONG_GU, QuantityRange::between(4, 6)),
    DropEntry::guaranteed(LONG_LIN, QuantityRange::between(2, 3)),
    DropEntry::guaranteed(YI_SHOU_GU, QuantityRange::between(5, 8)),
    DropEntry::rare(DU_LONG_ZHU, QuantityRange::fixed(1), 0.15),
];

const BONE_DRAGON_DROPS: [DropEntry; 4] = [
    DropEntry::guaranteed(KU_LONG_GU, QuantityRange::between(5, 8)),
    DropEntry::guaranteed(LONG_YA, QuantityRange::between(2, 3)),
    DropEntry::guaranteed(YI_SHOU_GU, QuantityRange::between(5, 8)),
    DropEntry::rare(GU_LONG_HUN_JING, QuantityRange::fixed(1), 0.15),
];

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
        BeastKind::GreenSpider => &GREEN_SPIDER_DROPS,
        BeastKind::JungleScorpion => &JUNGLE_SCORPION_DROPS,
        BeastKind::CockadeSnake => &COCKADE_SNAKE_DROPS,
        BeastKind::BlueSpider => &BLUE_SPIDER_DROPS,
        BeastKind::IceScorpion => &ICE_SCORPION_DROPS,
        BeastKind::MandrakeSnake => &MANDRAKE_SNAKE_DROPS,
        BeastKind::HybridBeast => &HYBRID_DROPS,
        BeastKind::VoidDistorted => &VOID_DISTORTED_DROPS,
        BeastKind::DarkTiger => &DARK_TIGER_DROPS,
        BeastKind::LivingPillar => &LIVING_PILLAR_DROPS,
        BeastKind::Heiwushi => &HEIWUSHI_DROPS,
        BeastKind::PoisonDragon => &POISON_DRAGON_DROPS,
        BeastKind::BoneDragon => &BONE_DRAGON_DROPS,
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

// ---------------------------------------------------------------------------
// 凡兽掉落表（plan-mundane-fauna-v1 P1）——平行于 `drop_table_for(BeastKind)`，
// 不复用 BeastKind 键。按物种微调掉落权重（鸡多肉少皮/牛皮厚/蛙无皮有腿肉→统一映射
// meat 键），威胁谱系越高（狐/狼）凡骨与生血命中率越高（骨架更结实、体量更大）。
// ---------------------------------------------------------------------------

const COW_DROPS: [DropEntry; 4] = [
    DropEntry::guaranteed(RAW_BEAST_MEAT, QuantityRange::between(2, 4)),
    // 牛皮厚——唯一保底 quantity 上探到 2 的物种，与其余单张皮凡兽拉开差异。
    DropEntry::guaranteed(RAW_BEAST_HIDE, QuantityRange::between(1, 2)),
    DropEntry::rare(FAN_GU, QuantityRange::fixed(1), 0.18),
    DropEntry::rare(RAW_BEAST_BLOOD, QuantityRange::fixed(1), 0.15),
];

const PIG_DROPS: [DropEntry; 4] = [
    DropEntry::guaranteed(RAW_BEAST_MEAT, QuantityRange::between(2, 3)),
    DropEntry::rare(RAW_BEAST_HIDE, QuantityRange::fixed(1), 0.30),
    DropEntry::rare(FAN_GU, QuantityRange::fixed(1), 0.15),
    DropEntry::rare(RAW_BEAST_BLOOD, QuantityRange::fixed(1), 0.12),
];

const SHEEP_DROPS: [DropEntry; 4] = [
    DropEntry::guaranteed(RAW_BEAST_MEAT, QuantityRange::between(1, 2)),
    DropEntry::guaranteed(RAW_BEAST_HIDE, QuantityRange::fixed(1)),
    DropEntry::rare(FAN_GU, QuantityRange::fixed(1), 0.15),
    DropEntry::rare(RAW_BEAST_BLOOD, QuantityRange::fixed(1), 0.12),
];

const GOAT_DROPS: [DropEntry; 4] = [
    DropEntry::guaranteed(RAW_BEAST_MEAT, QuantityRange::between(1, 2)),
    DropEntry::guaranteed(RAW_BEAST_HIDE, QuantityRange::fixed(1)),
    DropEntry::rare(FAN_GU, QuantityRange::fixed(1), 0.15),
    DropEntry::rare(RAW_BEAST_BLOOD, QuantityRange::fixed(1), 0.12),
];

const CHICKEN_DROPS: [DropEntry; 4] = [
    // 鸡多肉少皮：meat 保底区间高，hide 降级成稀有小概率。
    DropEntry::guaranteed(RAW_BEAST_MEAT, QuantityRange::between(1, 3)),
    DropEntry::rare(RAW_BEAST_HIDE, QuantityRange::fixed(1), 0.10),
    DropEntry::rare(FAN_GU, QuantityRange::fixed(1), 0.08),
    DropEntry::rare(RAW_BEAST_BLOOD, QuantityRange::fixed(1), 0.10),
];

const RABBIT_DROPS: [DropEntry; 5] = [
    DropEntry::guaranteed(RAW_BEAST_MEAT, QuantityRange::fixed(1)),
    DropEntry::rare(RAW_BEAST_HIDE, QuantityRange::fixed(1), 0.15),
    DropEntry::guaranteed(RABBIT_PELT, QuantityRange::fixed(1)),
    DropEntry::rare(FAN_GU, QuantityRange::fixed(1), 0.08),
    DropEntry::rare(RAW_BEAST_BLOOD, QuantityRange::fixed(1), 0.08),
];

const FROG_DROPS: [DropEntry; 3] = [
    // 蛙无皮有腿肉——统一映射 meat 键，不产 hide。
    DropEntry::guaranteed(RAW_BEAST_MEAT, QuantityRange::fixed(1)),
    DropEntry::rare(FAN_GU, QuantityRange::fixed(1), 0.05),
    DropEntry::rare(RAW_BEAST_BLOOD, QuantityRange::fixed(1), 0.06),
];

const FOX_DROPS: [DropEntry; 4] = [
    DropEntry::guaranteed(RAW_BEAST_MEAT, QuantityRange::between(1, 2)),
    DropEntry::guaranteed(RAW_BEAST_HIDE, QuantityRange::fixed(1)),
    DropEntry::rare(FAN_GU, QuantityRange::fixed(1), 0.20),
    DropEntry::rare(RAW_BEAST_BLOOD, QuantityRange::fixed(1), 0.15),
];

const WOLF_DROPS: [DropEntry; 4] = [
    DropEntry::guaranteed(RAW_BEAST_MEAT, QuantityRange::between(2, 3)),
    DropEntry::guaranteed(RAW_BEAST_HIDE, QuantityRange::fixed(1)),
    // T2.5 群体掠食——凡兽威胁谱系最高档，骨架最结实，凡骨命中率也最高。
    DropEntry::rare(FAN_GU, QuantityRange::fixed(1), 0.25),
    DropEntry::rare(RAW_BEAST_BLOOD, QuantityRange::fixed(1), 0.18),
];

pub fn drop_table_for_mundane(kind: MundaneFaunaKind) -> &'static [DropEntry] {
    match kind {
        MundaneFaunaKind::Cow => &COW_DROPS,
        MundaneFaunaKind::Pig => &PIG_DROPS,
        MundaneFaunaKind::Sheep => &SHEEP_DROPS,
        MundaneFaunaKind::Goat => &GOAT_DROPS,
        MundaneFaunaKind::Chicken => &CHICKEN_DROPS,
        MundaneFaunaKind::Rabbit => &RABBIT_DROPS,
        MundaneFaunaKind::Frog => &FROG_DROPS,
        MundaneFaunaKind::Fox => &FOX_DROPS,
        MundaneFaunaKind::Wolf => &WOLF_DROPS,
    }
}

/// 凡兽版 `roll_fauna_drops`——凡兽无 `BeastVariant`（无变异档），概率直接取
/// `DropEntry::probability`，不像妖兽那样乘 `rare_drop_multiplier`。
pub fn roll_mundane_fauna_drops(kind: MundaneFaunaKind, seed: u64) -> Vec<RolledFaunaDrop> {
    let mut out = Vec::new();
    for (idx, entry) in drop_table_for_mundane(kind).iter().enumerate() {
        let idx_seed = seed.wrapping_add((idx as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15));
        if splitmix64_unit(idx_seed) > entry.probability.clamp(0.0, 1.0) {
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
        let Ok((tag, archetype, species, pos, dimension, issued)) = npcs.get(event.target) else {
            continue;
        };
        if issued.is_some() {
            continue;
        }
        let seed = fauna_drop_seed(event.target, event.at_tick);
        // plan-mundane-fauna-v1 P1：凡兽（挂 MundaneFaunaSpecies）走独立掉落表，不复用
        // BeastKind 键；否则回退到既有妖兽 FaunaTag / legacy archetype 分支。
        let (drops, source_tag) = if let Some(species) = species {
            (
                roll_mundane_fauna_drops(species.0, seed),
                format!("fauna_drop:mundane:{}", species.0.as_str()),
            )
        } else {
            let Some(tag) = tag.copied().or_else(|| fallback_tag(archetype.copied())) else {
                continue;
            };
            (
                roll_fauna_drops(tag, seed),
                format!("fauna_drop:{}", tag.beast_kind.as_str()),
            )
        };
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
                    source_container_id: source_tag.clone(),
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

        commands
            .entity(event.target)
            .insert((FaunaDropIssued, Despawned));
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
        // plan-mundane-fauna-v1 P1：生肉/生血挂 Spoil freshness（腐败快于熟肉/生血快于生肉）。
        // initial_qi 取 fauna.toml 里对应 item 的 spirit_quality_initial（0.35 / 0.30）。
        RAW_BEAST_MEAT => Some(("raw_beast_meat_v1", 0.35)),
        RAW_BEAST_BLOOD => Some(("raw_beast_blood_v1", 0.30)),
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

pub(crate) fn jittered_drop_pos(base: valence::prelude::DVec3, seed: u64, idx: u64) -> [f64; 3] {
    let x = splitmix64_unit(seed.wrapping_add(idx)) as f64 - 0.5;
    let z = splitmix64_unit(seed.wrapping_add(idx.rotate_left(11))) as f64 - 0.5;
    [base.x + x * 0.7, base.y, base.z + z * 0.7]
}

fn splitmix64_u32(seed: u64) -> u32 {
    (splitmix64(seed) >> 32) as u32
}

pub(crate) fn splitmix64_unit(seed: u64) -> f32 {
    let bits = ((splitmix64(seed) >> 40) & 0x00FF_FFFF) as u32;
    bits as f32 / (1u32 << 24) as f32
}

pub(crate) fn splitmix64(seed: u64) -> u64 {
    let mut x = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use valence::prelude::{App, Despawned, Events, Update};

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
            placeable: None,
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
            technique_scroll_spec: None,
            readable_scroll_spec: None,
            recipe_fragment_spec: None,
            container_spec: None,
            shelflife_profile: None,
            shield_spec: None,
            shelflife_track: None,
            wearer_race: crate::body_plan::types::RaceGateOwned::default(),
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
            ZHU_SI,
            LV_ZHU_DUNANG,
            XIE_KE,
            XIE_WEI_ZHEN,
            XIE_DU_XIAN,
            SHE_LIN,
            SHE_DAN,
            JIGUAN_SHE_GUAN,
            BING_ZHU_SI,
            SHUANG_ZHU_HE,
            BINGBI_JIAPIAN,
            BINGBI_XIE_HE,
            SHE_YA,
            MANTUOLUO_SHE_TONG,
            HU_GU,
            HU_PI,
            XIESHEN_HU_XIN,
            LONG_GU,
            LONG_LIN,
            DU_LONG_ZHU,
            KU_LONG_GU,
            LONG_YA,
            GU_LONG_HUN_JING,
            ZHU_HE_SUIPIAN,
            CHU_XU,
            SHENYUAN_ZHI_YAN,
            "bone_coin_5",
            "bone_coin_15",
            "bone_coin_40",
            // plan-mundane-fauna-v1 P1
            RAW_BEAST_MEAT,
            RAW_BEAST_HIDE,
            RAW_BEAST_BLOOD,
            FAN_GU,
            RABBIT_PELT,
        ];
        ItemRegistry::from_map(
            ids.into_iter()
                .map(|id| (id.to_string(), template(id)))
                .collect::<HashMap<_, _>>(),
        )
    }

    fn empty_player_inventory() -> PlayerInventory {
        PlayerInventory {
            triggered_treasures: Vec::new(),
            revision: InventoryRevision(1),
            containers: vec![ContainerState {
                quick_access: false,
                id: MAIN_PACK_CONTAINER_ID.to_string(),
                name: "主背包".to_string(),
                rows: 5,
                cols: 7,
                items: Vec::new(),
                owner_instance_id: None,
            }],
            equipped: HashMap::new(),
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 50.0,
        }
    }

    #[test]
    fn each_beast_kind_rolls_at_least_one_guaranteed_drop() {
        let all_kinds = [
            BeastKind::Rat,
            BeastKind::Spider,
            BeastKind::GreenSpider,
            BeastKind::JungleScorpion,
            BeastKind::CockadeSnake,
            BeastKind::BlueSpider,
            BeastKind::IceScorpion,
            BeastKind::MandrakeSnake,
            BeastKind::HybridBeast,
            BeastKind::VoidDistorted,
            BeastKind::DarkTiger,
            BeastKind::LivingPillar,
            BeastKind::Heiwushi, // plan-sword-path-v2 P3 boss — 修复 review 发现的遗漏
            BeastKind::PoisonDragon,
            BeastKind::BoneDragon,
            BeastKind::Whale,
        ];
        for kind in all_kinds {
            for seed in [1, 42, 99] {
                let drops = roll_fauna_drops(FaunaTag::new(kind), seed);
                assert!(
                    !drops.is_empty(),
                    "{kind:?} must produce at least one guaranteed drop for seed {seed}"
                );
                assert!(
                    drops.iter().any(|drop| drop.quantity >= 1),
                    "{kind:?} must emit at least one positive stack"
                );
            }
        }
    }

    // ── plan-mundane-fauna-v1 P1：凡兽掉落矩阵（9 物种 × 掉落表专属 case）───────────────

    #[test]
    fn each_mundane_kind_rolls_at_least_one_guaranteed_meat_drop() {
        for kind in MundaneFaunaKind::ALL {
            for seed in [1, 42, 99, 12345] {
                let drops = roll_mundane_fauna_drops(kind, seed);
                assert!(
                    drops.iter().any(|d| d.item_id == RAW_BEAST_MEAT),
                    "{kind:?} must always drop raw_beast_meat (guaranteed) for seed {seed}, got {drops:?}"
                );
            }
        }
    }

    #[test]
    fn rabbit_always_produces_recipe_pelt() {
        let table = drop_table_for_mundane(MundaneFaunaKind::Rabbit);
        let pelt = table
            .iter()
            .find(|entry| entry.item_id == RABBIT_PELT)
            .expect("rabbit production table must expose rabbit_pelt");
        assert_eq!(pelt.probability, 1.0);
        assert_eq!(pelt.quantity, QuantityRange::fixed(1));

        for seed in [0, 1, 42, u64::MAX] {
            let drops = roll_mundane_fauna_drops(MundaneFaunaKind::Rabbit, seed);
            assert!(
                drops
                    .iter()
                    .any(|drop| drop.item_id == RABBIT_PELT && drop.quantity == 1),
                "rabbit_pelt must be obtainable from every rabbit death seed, got {drops:?}"
            );
        }
    }

    #[test]
    fn mundane_drop_tables_never_leak_beast_exclusive_items() {
        // 契约：凡兽掉落表与妖兽掉落表物品池互不相通——凡兽无灵，绝不产出骨材/变异核心。
        let beast_only_items = [
            SHU_GU,
            ZHU_GU,
            FENG_HE_GU,
            YI_SHOU_GU,
            BIAN_YI_HEXIN,
            FU_YA_HESUI,
            ZHEN_SHI_CHU,
        ];
        for kind in MundaneFaunaKind::ALL {
            let table = drop_table_for_mundane(kind);
            for entry in table {
                assert!(
                    !beast_only_items.contains(&entry.item_id),
                    "{kind:?} drop table must not contain beast-exclusive item `{}`",
                    entry.item_id
                );
            }
        }
    }

    #[test]
    fn frog_drop_table_has_no_hide_entry_at_all() {
        // 蛙无皮有腿肉——统一映射 meat 键，drop_table_for_mundane(Frog) 里不应出现
        // RAW_BEAST_HIDE（不只是概率低，而是压根没有这一条目）。
        let table = drop_table_for_mundane(MundaneFaunaKind::Frog);
        assert!(
            !table.iter().any(|e| e.item_id == RAW_BEAST_HIDE),
            "Frog drop table must not contain raw_beast_hide entry, got {table:?}"
        );
    }

    #[test]
    fn chicken_hide_is_rare_not_guaranteed() {
        // 鸡多肉少皮：hide 存在但概率 < 1.0（不是保底）。
        let table = drop_table_for_mundane(MundaneFaunaKind::Chicken);
        let hide = table
            .iter()
            .find(|e| e.item_id == RAW_BEAST_HIDE)
            .expect("Chicken drop table should still contain a (rare) hide entry");
        assert!(
            hide.probability < 1.0,
            "chicken hide probability={} must be < 1.0 (少皮, not guaranteed)",
            hide.probability
        );
    }

    #[test]
    fn cow_hide_quantity_upper_bound_exceeds_single_hide_species() {
        // 牛皮厚——cow 的保底 hide quantity 上界必须 > 1，拉开与羊/山羊/狐/狼的单张皮差异。
        let cow_hide = drop_table_for_mundane(MundaneFaunaKind::Cow)
            .iter()
            .find(|e| e.item_id == RAW_BEAST_HIDE)
            .expect("cow must have a guaranteed hide entry");
        assert!(
            cow_hide.probability >= 1.0,
            "cow hide must be guaranteed (probability=1.0), got {}",
            cow_hide.probability
        );
        assert!(
            cow_hide.quantity.max > 1,
            "cow hide quantity upper bound must exceed 1 (牛皮厚), got {:?}",
            cow_hide.quantity
        );

        for (kind, item) in [
            (MundaneFaunaKind::Sheep, RAW_BEAST_HIDE),
            (MundaneFaunaKind::Goat, RAW_BEAST_HIDE),
            (MundaneFaunaKind::Fox, RAW_BEAST_HIDE),
            (MundaneFaunaKind::Wolf, RAW_BEAST_HIDE),
        ] {
            let entry = drop_table_for_mundane(kind)
                .iter()
                .find(|e| e.item_id == item)
                .unwrap_or_else(|| panic!("{kind:?} must have a hide entry"));
            assert_eq!(
                entry.quantity,
                QuantityRange::fixed(1),
                "{kind:?} hide should be single-quantity, unlike cow's thick hide"
            );
        }
    }

    #[test]
    fn wolf_has_the_highest_fan_gu_probability_across_all_mundane_species() {
        // 威胁谱系：T2.5 狼骨架最结实，凡骨命中率必须是 9 物种里最高的一档。
        let wolf_fan_gu = drop_table_for_mundane(MundaneFaunaKind::Wolf)
            .iter()
            .find(|e| e.item_id == FAN_GU)
            .expect("wolf must drop fan_gu")
            .probability;
        for kind in MundaneFaunaKind::ALL {
            if kind == MundaneFaunaKind::Wolf {
                continue;
            }
            let other = drop_table_for_mundane(kind)
                .iter()
                .find(|e| e.item_id == FAN_GU)
                .map(|e| e.probability)
                .unwrap_or(0.0);
            assert!(
                wolf_fan_gu > other,
                "wolf fan_gu probability {wolf_fan_gu} must exceed {kind:?}'s {other}"
            );
        }
    }

    #[test]
    fn fauna_item_instance_attaches_raw_beast_meat_freshness_faster_than_bone() {
        let registry = fauna_registry();
        let profiles = crate::shelflife::build_default_registry();
        let mut allocator = InventoryInstanceIdAllocator::new(100);
        let item = build_fauna_item_instance(
            RAW_BEAST_MEAT,
            2,
            10,
            &registry,
            Some(&profiles),
            &mut allocator,
        )
        .expect("raw_beast_meat template and profile should exist");

        let freshness = item
            .freshness
            .expect("mundane meat drop should carry freshness");
        assert_eq!(freshness.profile.as_str(), "raw_beast_meat_v1");
        assert_eq!(freshness.created_at_tick, 10);
    }

    #[test]
    fn death_event_for_mundane_species_drops_from_mundane_table_with_mundane_source_tag() {
        let mut app = App::new();
        app.add_event::<DeathEvent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.insert_resource(fauna_registry());
        app.insert_resource(crate::shelflife::build_default_registry());
        app.insert_resource(InventoryInstanceIdAllocator::new(10));
        app.insert_resource(DroppedLootRegistry::default());
        app.add_systems(Update, fauna_drop_system);

        let cow = app
            .world_mut()
            .spawn((
                NpcMarker,
                MundaneFaunaSpecies(MundaneFaunaKind::Cow),
                Position::new([3.0, 64.0, 4.0]),
            ))
            .id();
        app.world_mut().send_event(DeathEvent {
            target: cow,
            cause: "test".to_string(),
            attacker: None,
            attacker_player_id: None,
            at_tick: 200,
        });

        app.update();

        let drops = app.world().resource::<DroppedLootRegistry>();
        let meat_entry = drops
            .entries
            .values()
            .find(|entry| entry.item.template_id == RAW_BEAST_MEAT)
            .expect("cow death should drop raw_beast_meat via mundane branch");
        assert_eq!(
            meat_entry.source_container_id, "fauna_drop:mundane:cow",
            "mundane drop source_container_id must be tagged distinctly from beast drops"
        );
        assert!(
            app.world().get::<FaunaDropIssued>(cow).is_some(),
            "processed mundane fauna should be marked to prevent duplicate drops"
        );
        assert!(
            app.world().get::<Despawned>(cow).is_some(),
            "processed mundane fauna should be marked despawned"
        );
    }

    #[test]
    fn mundane_species_never_falls_back_to_legacy_beast_table() {
        // 回归防护：MundaneFaunaSpecies 分支必须优先于 tag/archetype fallback 判定，
        // 否则若凡兽实体意外挂了 NpcArchetype::Beast 会被吞进老鼠表（悖离契约）。
        let mut app = App::new();
        app.add_event::<DeathEvent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.insert_resource(fauna_registry());
        app.insert_resource(crate::shelflife::build_default_registry());
        app.insert_resource(InventoryInstanceIdAllocator::new(10));
        app.insert_resource(DroppedLootRegistry::default());
        app.add_systems(Update, fauna_drop_system);

        let chicken = app
            .world_mut()
            .spawn((
                NpcMarker,
                NpcArchetype::Beast,
                MundaneFaunaSpecies(MundaneFaunaKind::Chicken),
                Position::new([0.0, 64.0, 0.0]),
            ))
            .id();
        app.world_mut().send_event(DeathEvent {
            target: chicken,
            cause: "test".to_string(),
            attacker: None,
            attacker_player_id: None,
            at_tick: 5,
        });

        app.update();

        let drops = app.world().resource::<DroppedLootRegistry>();
        let dropped_templates: Vec<&str> = drops
            .entries
            .values()
            .map(|entry| entry.item.template_id.as_str())
            .collect();
        assert!(
            !dropped_templates.contains(&SHU_GU),
            "must not fall back to rat table when MundaneFaunaSpecies is present, got {dropped_templates:?}"
        );
        assert!(
            dropped_templates.contains(&RAW_BEAST_MEAT),
            "should drop mundane raw_beast_meat instead, got {dropped_templates:?}"
        );
    }

    // ── HEIWUSHI_DROPS 饱和锁（plan-sword-path-v2 P3 review 补测）───────────────────────────
    //
    // 设计表：
    //   star_iron        ×2   保底（guaranteed, fixed）
    //   sword_embryo_shard ×2 保底（guaranteed, fixed）
    //   ancient_sword_embryo ×1 30%（rare）
    //   scroll_sword_manifest ×1 10%（rare）
    //
    // 本测试锁定以上契约；任何对 HEIWUSHI_DROPS 的修改必须同步改此测试。
    #[test]
    fn heiwushi_drops_match_boss_design_table() {
        let mut star_iron_hits = 0;
        let mut sword_shard_hits = 0;
        let mut ancient_embryo_hits = 0;
        let mut scroll_manifest_hits = 0;
        const SAMPLES: u64 = 2000;

        for seed in 0..SAMPLES {
            let drops = roll_fauna_drops(FaunaTag::new(BeastKind::Heiwushi), seed.wrapping_mul(37));

            // 保底项：每次都必须出现
            let star_iron = drops
                .iter()
                .find(|d| d.item_id == "star_iron")
                .expect("黑武士必须保底掉落 star_iron（guaranteed），seed={seed} 未命中");
            assert_eq!(
                star_iron.quantity, 2,
                "star_iron 数量应为固定 2，seed={seed} 实际={:?}",
                star_iron.quantity
            );
            star_iron_hits += 1;

            let sword_shard = drops
                .iter()
                .find(|d| d.item_id == "sword_embryo_shard")
                .expect("黑武士必须保底掉落 sword_embryo_shard（guaranteed），seed={seed} 未命中");
            assert_eq!(
                sword_shard.quantity, 2,
                "sword_embryo_shard 数量应为固定 2，seed={seed} 实际={:?}",
                sword_shard.quantity
            );
            sword_shard_hits += 1;

            // 稀有项：计数
            if drops.iter().any(|d| d.item_id == "ancient_sword_embryo") {
                ancient_embryo_hits += 1;
            }
            if drops.iter().any(|d| d.item_id == "scroll_sword_manifest") {
                scroll_manifest_hits += 1;
            }
        }

        // 保底项：100% 命中
        assert_eq!(
            star_iron_hits, SAMPLES,
            "star_iron 保底：期望 {SAMPLES} 次全中，实际 {star_iron_hits}"
        );
        assert_eq!(
            sword_shard_hits, SAMPLES,
            "sword_embryo_shard 保底：期望 {SAMPLES} 次全中，实际 {sword_shard_hits}"
        );

        // ancient_sword_embryo ~30%（容差 ±5%）
        let ancient_rate = ancient_embryo_hits as f64 / SAMPLES as f64;
        assert!(
            (0.25..=0.35).contains(&ancient_rate),
            "ancient_sword_embryo 期望概率 ~0.30 (±0.05)，实际 {ancient_rate:.3}（{ancient_embryo_hits}/{SAMPLES}）"
        );

        // scroll_sword_manifest ~10%（容差 ±4%）
        let scroll_rate = scroll_manifest_hits as f64 / SAMPLES as f64;
        assert!(
            (0.06..=0.14).contains(&scroll_rate),
            "scroll_sword_manifest 期望概率 ~0.10 (±0.04)，实际 {scroll_rate:.3}（{scroll_manifest_hits}/{SAMPLES}）"
        );
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
    fn death_event_creates_dropped_loot_and_marks_target_despawned() {
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
        assert!(
            app.world().get::<Despawned>(beast).is_some(),
            "processed beast should be marked despawned so clients clear the dead entity"
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
