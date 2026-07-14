//! plan-race-system-v1 P0 — `RaceRegistry`：单文件 `server/assets/body_plans/races.json`
//! loader（不进 `plans/` glob，见 `registry.rs` 顶部注释的目录边界）。
//!
//! 职责：
//! - `races[]`：`RaceId → BodyPlanId` 映射 + 可选 `beast_kinds`（`BeastKind → RaceId`
//!   派生索引的数据源——查询走这里而非硬编码 Rust match，见 `race_id_for_beast_kind`）
//! - `morph_pairs[]`：易形配对的唯一真源（P4 `resolve_morph_pair` 消费，P0 只锁 schema +
//!   跨 registry 校验，见 `MorphPairDef` 文档）
//!
//! 跨 registry 校验（`body_plan_id` 存在性 / `morph_pairs.part_mapping` 端点）在
//! `load_file` 内完成——本 registry 天然需要持有已加载完毕的 `BodyPlanRegistry` 才能校验
//! 这些引用，因此这就是 plan 所说的「跨 registry post-load 校验」的落点。

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use valence::prelude::Resource;

use crate::fauna::components::BeastKind;

use super::registry::BodyPlanRegistry;
use super::types::{BodyPartId, BodyPlanId, RaceId};
use crate::cultivation::components::MeridianChannelId;

pub const DEFAULT_RACES_PATH: &str = "assets/body_plans/races.json";
/// 必须存在的默认种族条目——加载期强制校验（`resolve_body_plan` 拒载未知种族依赖此）。
pub const HUMAN_RACE_ID: &str = "human";

/// 全部 16 个 `BeastKind` 变体（`fauna/components.rs` 无 `ALL` 常量覆盖全部变体——
/// `ALL_TERRESTRIAL` 排除了 Boss/化虚级——故在此本地维护一份用于 races.json 的
/// `beast_kinds` 字符串校验，`parse_beast_kind` 是唯一消费点）。
const ALL_BEAST_KINDS: &[BeastKind] = &[
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
    BeastKind::Heiwushi,
    BeastKind::PoisonDragon,
    BeastKind::BoneDragon,
    BeastKind::Whale,
];

fn parse_beast_kind(raw: &str) -> Option<BeastKind> {
    ALL_BEAST_KINDS
        .iter()
        .copied()
        .find(|kind| kind.as_str() == raw)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RaceEntry {
    pub id: RaceId,
    pub display_name: String,
    pub body_plan_id: BodyPlanId,
    #[serde(default)]
    pub beast_kinds: Vec<String>,
}

/// 易形配对——`from` 种族的实体易形为 `to` 种族外观。`part_mapping` 方向 =
/// **form_part（`to` 的部位）→ intrinsic_part（`from` 的部位）**：P4 结算命中时把
/// 落在 form 部位上的伤害折算回本体部位，允许部分单射（`to` 部位数 != `from` 部位数
/// 时天然缺项，不要求满射）。P0 只做 schema + 端点存在性校验，不实现
/// `resolve_morph_pair` 查询（P4 落地）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct MorphPairDef {
    pub from: RaceId,
    pub to: RaceId,
    #[serde(default)]
    pub part_mapping: HashMap<BodyPartId, BodyPartId>,
}

impl MorphPairDef {
    /// plan-race-system-v1 P4 —— `part_mapping` 单一封装查询：给定 form 部位（`to` 种族
    /// 的部位 id），返回其折算回的 intrinsic 部位（`from` 种族的部位 id）。`None` = 该
    /// form 部位未声明映射（part_mapping 允许部分单射，见类型文档）。
    pub fn intrinsic_part_for(&self, form_part: &BodyPartId) -> Option<&BodyPartId> {
        self.part_mapping.get(form_part)
    }

    /// plan-race-system-v1 P4 —— 逆查：给定 intrinsic 部位，返回映射到它的 form 部位
    /// （`part_mapping` 值唯一——见加载期单射校验——故至多一个）。供护甲折算消费点
    /// （`combat::resolve`）把命中在 intrinsic 部位上的伤害，反向折算回"形态外观"的
    /// 护甲覆盖部位。
    pub fn form_part_for_intrinsic(&self, intrinsic_part: &BodyPartId) -> Option<&BodyPartId> {
        self.part_mapping
            .iter()
            .find(|(_, v)| *v == intrinsic_part)
            .map(|(k, _)| k)
    }
}

/// plan-race-system-v1 P5/PR-6a §8#9 —— 经脉迁移映射：`RaceChange` 换种族时，
/// `from` 种族某条经脉 channel id 一对一映射到 `to` 种族的哪条 channel id（保留状态
/// 直接迁移）。**未在 `entries` 中出现的 `from` 侧 channel** 不会被摧毁——由
/// `cultivation::race_change` 挂进 `MeridianSeveredPermanent` 的"休眠登记"（见
/// `cultivation::meridian::severed::MeridianSeveredPermanent::register_dormant`），
/// 换回一个 profile 里恰好含有该 channel id 的种族时按 id 精确恢复。
///
/// 校验（`load_file`/`from_file_contents`）：
/// - `from`/`to` 种族必须存在；
/// - `entries` 每一端 channel id 必须真实存在于对应种族 body_plan 的
///   `meridian_profile.channels`（body_plan 无 `meridian_profile` 时视为空 channel
///   集合，任何引用都会失败）；
/// - `entries` 必须**一对一**：同一 `from` channel 不可出现两次（禁一对多），
///   同一 `to` channel 也不可被两个不同 `from` channel 同时瞄准（禁多对一）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct MeridianMappingDef {
    pub from: RaceId,
    pub to: RaceId,
    pub entries: Vec<(MeridianChannelId, MeridianChannelId)>,
}

impl MeridianMappingDef {
    /// 给定 `from` 种族的 channel id，返回它迁移到的 `to` 种族 channel id
    /// （加载期一对一校验保证至多一条匹配）。
    pub fn target_for(&self, from_channel: &MeridianChannelId) -> Option<&MeridianChannelId> {
        self.entries
            .iter()
            .find(|(f, _)| f == from_channel)
            .map(|(_, t)| t)
    }

    /// 逆查：给定 `to` 种族的 channel id，返回它是从 `from` 种族哪条 channel 迁移
    /// 过来的（加载期一对一校验保证至多一条匹配）。
    pub fn source_for(&self, to_channel: &MeridianChannelId) -> Option<&MeridianChannelId> {
        self.entries
            .iter()
            .find(|(_, t)| t == to_channel)
            .map(|(f, _)| f)
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct RaceFile {
    races: Vec<RaceEntry>,
    #[serde(default)]
    morph_pairs: Vec<MorphPairDef>,
    #[serde(default)]
    meridian_mappings: Vec<MeridianMappingDef>,
}

#[derive(Debug, Default, Clone)]
pub struct RaceRegistry {
    by_id: HashMap<RaceId, RaceEntry>,
    beast_kind_owner: HashMap<String, RaceId>,
    morph_pairs: HashMap<(RaceId, RaceId), MorphPairDef>,
    meridian_mappings: HashMap<(RaceId, RaceId), MeridianMappingDef>,
}

impl Resource for RaceRegistry {}

#[derive(Debug)]
pub enum RaceLoadError {
    Io(std::io::Error),
    Json {
        path: PathBuf,
        source: serde_json::Error,
    },
    Invalid(String),
}

impl std::fmt::Display for RaceLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RaceLoadError::Io(e) => write!(f, "io: {e}"),
            RaceLoadError::Json { path, source } => {
                write!(f, "json: {}: {source}", path.display())
            }
            RaceLoadError::Invalid(reason) => write!(f, "invalid races.json: {reason}"),
        }
    }
}

impl std::error::Error for RaceLoadError {}

impl From<std::io::Error> for RaceLoadError {
    fn from(e: std::io::Error) -> Self {
        RaceLoadError::Io(e)
    }
}

impl RaceRegistry {
    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }

    pub fn get(&self, id: &RaceId) -> Option<&RaceEntry> {
        self.by_id.get(id)
    }

    pub fn morph_pair(&self, from: &RaceId, to: &RaceId) -> Option<&MorphPairDef> {
        self.morph_pairs.get(&(from.clone(), to.clone()))
    }

    /// plan-race-system-v1 P4 —— `morph_pair` 别名（规格命名 `resolve_morph_pair`），
    /// 供易形消费点按语义更贴切的名字调用；行为与 [`Self::morph_pair`] 完全一致。
    pub fn resolve_morph_pair(&self, from: &RaceId, to: &RaceId) -> Option<&MorphPairDef> {
        self.morph_pair(from, to)
    }

    pub fn morph_pair_count(&self) -> usize {
        self.morph_pairs.len()
    }

    /// plan-race-system-v1 P5/PR-6a §8#9 —— 查询 `(from, to)` 种族对之间声明的经脉
    /// 迁移映射。`None` = 未声明（`cultivation::race_change` 据此把 `from` 种族全部
    /// 经脉视为无映射，直接进休眠登记）。
    pub fn meridian_mapping(&self, from: &RaceId, to: &RaceId) -> Option<&MeridianMappingDef> {
        self.meridian_mappings.get(&(from.clone(), to.clone()))
    }

    pub fn meridian_mapping_count(&self) -> usize {
        self.meridian_mappings.len()
    }

    /// plan-race-system-v1 P4 —— 给定本体（intrinsic）种族，列出全部它可易形前往的
    /// 目标种族 id（按声明的 `to` 字典序排序，保证结果确定性）。目前生产 `races.json`
    /// `morph_pairs` 为空数组（P5 才引入 whale 真数据），调用方（`morph.yixing` cast
    /// resolver）据此可预期在生产环境恒返回空——"机制已通，缺数据"是预期状态而非 bug。
    pub fn morph_targets_from(&self, from: &RaceId) -> Vec<&RaceId> {
        let mut targets: Vec<&RaceId> = self
            .morph_pairs
            .keys()
            .filter(|(pair_from, _)| pair_from == from)
            .map(|(_, to)| to)
            .collect();
        targets.sort_by(|a, b| a.as_str().cmp(b.as_str()));
        targets
    }

    /// `BeastKind → RaceId` 派生查询——数据源是 races.json 各条目的 `beast_kinds`
    /// 列表（加载期已保证互斥归属），而非硬编码 Rust match。理论上不可达的
    /// "未登记 beast_kind" 分支（今天全部 16 个变体都已在 `races.json` 登记，见
    /// `load_file` 的 uniqueness 校验）防御性兜底到 `human`，避免任何遗漏在生产环境
    /// 直接 panic 中断战斗结算。
    pub fn race_id_for_beast_kind(&self, kind: BeastKind) -> RaceId {
        self.beast_kind_owner
            .get(kind.as_str())
            .cloned()
            .unwrap_or_else(|| {
                tracing::warn!(
                    "[bong][body_plan] beast_kind {} has no races.json owner — falling back to {HUMAN_RACE_ID}",
                    kind.as_str()
                );
                RaceId::new(HUMAN_RACE_ID)
            })
    }

    pub fn load_file(
        path: impl AsRef<Path>,
        body_plans: &BodyPlanRegistry,
    ) -> Result<Self, RaceLoadError> {
        let path = path.as_ref();
        let text = fs::read_to_string(path)?;
        let parsed: RaceFile = serde_json::from_str(&text).map_err(|e| RaceLoadError::Json {
            path: path.to_path_buf(),
            source: e,
        })?;
        Self::from_file_contents(parsed, body_plans)
    }

    fn from_file_contents(
        parsed: RaceFile,
        body_plans: &BodyPlanRegistry,
    ) -> Result<Self, RaceLoadError> {
        let mut by_id: HashMap<RaceId, RaceEntry> = HashMap::new();
        let mut beast_kind_owner: HashMap<String, RaceId> = HashMap::new();

        for entry in parsed.races {
            if by_id.contains_key(&entry.id) {
                return Err(RaceLoadError::Invalid(format!(
                    "duplicate race id {}",
                    entry.id
                )));
            }
            if body_plans.get(&entry.body_plan_id).is_none() {
                return Err(RaceLoadError::Invalid(format!(
                    "race {} references unknown body_plan_id {}",
                    entry.id, entry.body_plan_id
                )));
            }
            for beast_kind_str in &entry.beast_kinds {
                if parse_beast_kind(beast_kind_str).is_none() {
                    return Err(RaceLoadError::Invalid(format!(
                        "race {} lists unknown beast_kind {beast_kind_str:?}",
                        entry.id
                    )));
                }
                if let Some(owner) = beast_kind_owner.get(beast_kind_str) {
                    return Err(RaceLoadError::Invalid(format!(
                        "beast_kind {beast_kind_str:?} claimed by both race {owner} and {}",
                        entry.id
                    )));
                }
                beast_kind_owner.insert(beast_kind_str.clone(), entry.id.clone());
            }
            by_id.insert(entry.id.clone(), entry);
        }

        if !by_id.contains_key(&RaceId::new(HUMAN_RACE_ID)) {
            return Err(RaceLoadError::Invalid(format!(
                "races.json must contain a default {HUMAN_RACE_ID:?} race entry"
            )));
        }

        let mut morph_pairs: HashMap<(RaceId, RaceId), MorphPairDef> = HashMap::new();
        for pair in parsed.morph_pairs {
            let key = (pair.from.clone(), pair.to.clone());
            if morph_pairs.contains_key(&key) {
                return Err(RaceLoadError::Invalid(format!(
                    "duplicate morph pair {} -> {}",
                    pair.from, pair.to
                )));
            }
            let from_entry = by_id.get(&pair.from).ok_or_else(|| {
                RaceLoadError::Invalid(format!(
                    "morph pair references unknown from-race {}",
                    pair.from
                ))
            })?;
            let to_entry = by_id.get(&pair.to).ok_or_else(|| {
                RaceLoadError::Invalid(format!("morph pair references unknown to-race {}", pair.to))
            })?;
            let from_plan = body_plans
                .get(&from_entry.body_plan_id)
                .expect("body_plan_id existence already validated above");
            let to_plan = body_plans
                .get(&to_entry.body_plan_id)
                .expect("body_plan_id existence already validated above");

            for (form_part, intrinsic_part) in &pair.part_mapping {
                if !to_plan.parts.iter().any(|p| p.id == *form_part) {
                    return Err(RaceLoadError::Invalid(format!(
                        "morph pair {}->{} part_mapping key {} not found in to-race body plan {}",
                        pair.from, pair.to, form_part, to_entry.body_plan_id
                    )));
                }
                if !from_plan.parts.iter().any(|p| p.id == *intrinsic_part) {
                    return Err(RaceLoadError::Invalid(format!(
                        "morph pair {}->{} part_mapping value {} not found in from-race body plan {}",
                        pair.from, pair.to, intrinsic_part, from_entry.body_plan_id
                    )));
                }
            }

            // plan-race-system-v1 P4（决议 #2）—— part_mapping 必须部分**单射**：两个不同
            // form_part 不得映射到同一个 intrinsic_part（否则伤害折算/护甲逆查会撞出
            // 一对多歧义，见 `MorphPairDef::form_part_for_intrinsic`）。允许部分缺项
            // （不要求满射），只禁止值重复。
            {
                let mut seen_intrinsic_parts: std::collections::HashSet<&BodyPartId> =
                    std::collections::HashSet::new();
                for intrinsic_part in pair.part_mapping.values() {
                    if !seen_intrinsic_parts.insert(intrinsic_part) {
                        return Err(RaceLoadError::Invalid(format!(
                            "morph pair {}->{} part_mapping is not injective: intrinsic part {} \
                             is targeted by more than one form part",
                            pair.from, pair.to, intrinsic_part
                        )));
                    }
                }
            }

            morph_pairs.insert(key, pair);
        }

        let mut meridian_mappings: HashMap<(RaceId, RaceId), MeridianMappingDef> = HashMap::new();
        for mapping in parsed.meridian_mappings {
            let key = (mapping.from.clone(), mapping.to.clone());
            if meridian_mappings.contains_key(&key) {
                return Err(RaceLoadError::Invalid(format!(
                    "duplicate meridian mapping {} -> {}",
                    mapping.from, mapping.to
                )));
            }
            let from_entry = by_id.get(&mapping.from).ok_or_else(|| {
                RaceLoadError::Invalid(format!(
                    "meridian mapping references unknown from-race {}",
                    mapping.from
                ))
            })?;
            let to_entry = by_id.get(&mapping.to).ok_or_else(|| {
                RaceLoadError::Invalid(format!(
                    "meridian mapping references unknown to-race {}",
                    mapping.to
                ))
            })?;
            let from_plan = body_plans
                .get(&from_entry.body_plan_id)
                .expect("body_plan_id existence already validated above");
            let to_plan = body_plans
                .get(&to_entry.body_plan_id)
                .expect("body_plan_id existence already validated above");
            let from_channels: HashSet<&MeridianChannelId> = from_plan
                .meridian_profile
                .as_ref()
                .map(|p| p.channels.iter().map(|c| &c.id).collect())
                .unwrap_or_default();
            let to_channels: HashSet<&MeridianChannelId> = to_plan
                .meridian_profile
                .as_ref()
                .map(|p| p.channels.iter().map(|c| &c.id).collect())
                .unwrap_or_default();

            let mut seen_from: HashSet<&MeridianChannelId> = HashSet::new();
            let mut seen_to: HashSet<&MeridianChannelId> = HashSet::new();
            for (from_channel, to_channel) in &mapping.entries {
                if !from_channels.contains(from_channel) {
                    return Err(RaceLoadError::Invalid(format!(
                        "meridian mapping {}->{} entry from-channel {} not found in from-race \
                         body plan {} meridian_profile",
                        mapping.from, mapping.to, from_channel, from_entry.body_plan_id
                    )));
                }
                if !to_channels.contains(to_channel) {
                    return Err(RaceLoadError::Invalid(format!(
                        "meridian mapping {}->{} entry to-channel {} not found in to-race \
                         body plan {} meridian_profile",
                        mapping.from, mapping.to, to_channel, to_entry.body_plan_id
                    )));
                }
                if !seen_from.insert(from_channel) {
                    return Err(RaceLoadError::Invalid(format!(
                        "meridian mapping {}->{} is not one-to-one: from-channel {} is mapped \
                         more than once",
                        mapping.from, mapping.to, from_channel
                    )));
                }
                if !seen_to.insert(to_channel) {
                    return Err(RaceLoadError::Invalid(format!(
                        "meridian mapping {}->{} is not one-to-one: to-channel {} is targeted \
                         by more than one from-channel",
                        mapping.from, mapping.to, to_channel
                    )));
                }
            }

            meridian_mappings.insert(key, mapping);
        }

        Ok(Self {
            by_id,
            beast_kind_owner,
            morph_pairs,
            meridian_mappings,
        })
    }

    /// 供测试直接构造 registry，不走文件 IO。
    #[cfg(test)]
    pub fn from_parts_for_test(
        races: Vec<RaceEntry>,
        morph_pairs: Vec<MorphPairDef>,
        body_plans: &BodyPlanRegistry,
    ) -> Result<Self, RaceLoadError> {
        Self::from_file_contents(
            RaceFile {
                races,
                morph_pairs,
                meridian_mappings: Vec::new(),
            },
            body_plans,
        )
    }

    /// 供测试直接构造 registry（含 `meridian_mappings`），不走文件 IO。
    #[cfg(test)]
    pub fn from_parts_for_test_with_meridian_mappings(
        races: Vec<RaceEntry>,
        morph_pairs: Vec<MorphPairDef>,
        meridian_mappings: Vec<MeridianMappingDef>,
        body_plans: &BodyPlanRegistry,
    ) -> Result<Self, RaceLoadError> {
        Self::from_file_contents(
            RaceFile {
                races,
                morph_pairs,
                meridian_mappings,
            },
            body_plans,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::body_plan::types::{
        BodyPartDef, BodyPlan, HeightBand, HeightBandAssignment, HitGeometry, PartConsequence,
        StandingAabbSpec,
    };

    fn plan_with_parts(id: &str, part_ids: &[&str]) -> BodyPlan {
        BodyPlan {
            id: BodyPlanId::new(id),
            display_name: id.to_string(),
            // plan-race-system-v1 P1a：validate_body_plan 现在要求 is_humanoid==true 必须
            // 附带 meridian_profile；本 fixture 只练 race registry 加载/morph pair 机制，
            // 与经脉语义无关，设 false 避免每处都补一份 profile 数据。
            is_humanoid: false,
            parts: part_ids
                .iter()
                .map(|part_id| BodyPartDef {
                    id: BodyPartId::new(*part_id),
                    damage_mul: 1.0,
                    contam_mul: 1.0,
                    bleed_mul: 1.0,
                    consequence: PartConsequence::Core,
                })
                .collect(),
            hit_geometry: HitGeometry::HeightBands {
                aabb: StandingAabbSpec {
                    half_width: 0.3,
                    height: 1.8,
                },
                bands: vec![HeightBand {
                    min_rel_y: -1.0,
                    assignment: HeightBandAssignment::Single {
                        part: BodyPartId::new(part_ids[0]),
                    },
                }],
                lateral_threshold: 0.19,
            },
            equip_slots: vec![],
            meridian_profile: None,
            mutation_slot_mapping: HashMap::new(),
        }
    }

    /// `other_id == "humanoid"` 的调用方语义是"只要单独一个用 `other_parts` 定义的
    /// humanoid plan"（本仓多处既有测试如此使用），不是"humanoid 加另一个也叫
    /// humanoid 的 plan"——后者会撞 `DuplicatePlanId`。`other_id` 为任意其他值时
    /// 语义不变：humanoid（默认 4 段）+ 第二个不同 id 的 plan。
    fn body_plans_with_humanoid_and(other_id: &str, other_parts: &[&str]) -> BodyPlanRegistry {
        if other_id == "humanoid" {
            return BodyPlanRegistry::from_plans(vec![plan_with_parts("humanoid", other_parts)])
                .expect("fixture plan should validate");
        }
        BodyPlanRegistry::from_plans(vec![
            plan_with_parts("humanoid", &["head", "chest", "leg_l", "leg_r"]),
            plan_with_parts(other_id, other_parts),
        ])
        .expect("fixture plans should validate")
    }

    fn human_entry() -> RaceEntry {
        RaceEntry {
            id: RaceId::new(HUMAN_RACE_ID),
            display_name: "人族".to_string(),
            body_plan_id: BodyPlanId::new("humanoid"),
            beast_kinds: vec![],
        }
    }

    #[test]
    fn load_happy_path_with_human_and_beast_race() {
        let body_plans =
            body_plans_with_humanoid_and("humanoid", &["head", "chest", "leg_l", "leg_r"]);
        let races = RaceRegistry::from_parts_for_test(
            vec![
                human_entry(),
                RaceEntry {
                    id: RaceId::new("beast_common"),
                    display_name: "凡兽通用".to_string(),
                    body_plan_id: BodyPlanId::new("humanoid"),
                    beast_kinds: vec!["rat".to_string(), "whale".to_string()],
                },
            ],
            vec![],
            &body_plans,
        )
        .expect("valid races.json content should load");

        assert_eq!(races.len(), 2);
        assert_eq!(
            races.race_id_for_beast_kind(BeastKind::Rat),
            RaceId::new("beast_common")
        );
        assert_eq!(
            races.race_id_for_beast_kind(BeastKind::Whale),
            RaceId::new("beast_common")
        );
    }

    #[test]
    fn beast_kind_not_listed_anywhere_falls_back_to_human() {
        let body_plans =
            body_plans_with_humanoid_and("humanoid", &["head", "chest", "leg_l", "leg_r"]);
        let races = RaceRegistry::from_parts_for_test(vec![human_entry()], vec![], &body_plans)
            .expect("human-only races.json should load");
        assert_eq!(
            races.race_id_for_beast_kind(BeastKind::Spider),
            RaceId::new(HUMAN_RACE_ID),
            "未登记的 beast_kind 必须防御性兜底到 human，而不是 panic"
        );
    }

    #[test]
    fn duplicate_race_id_rejected() {
        let body_plans = body_plans_with_humanoid_and("humanoid", &["head"]);
        let err = RaceRegistry::from_parts_for_test(
            vec![human_entry(), human_entry()],
            vec![],
            &body_plans,
        )
        .expect_err("duplicate race id must fail");
        assert!(
            matches!(err, RaceLoadError::Invalid(reason) if reason.contains("duplicate race id"))
        );
    }

    #[test]
    fn missing_human_entry_rejected() {
        let body_plans = body_plans_with_humanoid_and("other", &["core"]);
        let err = RaceRegistry::from_parts_for_test(
            vec![RaceEntry {
                id: RaceId::new("other"),
                display_name: "other".to_string(),
                body_plan_id: BodyPlanId::new("other"),
                beast_kinds: vec![],
            }],
            vec![],
            &body_plans,
        )
        .expect_err("missing human entry must fail");
        assert!(matches!(err, RaceLoadError::Invalid(reason) if reason.contains("human")));
    }

    #[test]
    fn unknown_body_plan_id_rejected() {
        let body_plans = body_plans_with_humanoid_and("humanoid", &["head"]);
        let err = RaceRegistry::from_parts_for_test(
            vec![RaceEntry {
                id: RaceId::new(HUMAN_RACE_ID),
                display_name: "人族".to_string(),
                body_plan_id: BodyPlanId::new("does_not_exist"),
                beast_kinds: vec![],
            }],
            vec![],
            &body_plans,
        )
        .expect_err("unknown body_plan_id must fail");
        assert!(
            matches!(err, RaceLoadError::Invalid(reason) if reason.contains("unknown body_plan_id"))
        );
    }

    #[test]
    fn unknown_beast_kind_string_rejected() {
        let body_plans = body_plans_with_humanoid_and("humanoid", &["head"]);
        let err = RaceRegistry::from_parts_for_test(
            vec![
                human_entry(),
                RaceEntry {
                    id: RaceId::new("beast_common"),
                    display_name: "凡兽".to_string(),
                    body_plan_id: BodyPlanId::new("humanoid"),
                    beast_kinds: vec!["dragon_of_nonsense".to_string()],
                },
            ],
            vec![],
            &body_plans,
        )
        .expect_err("unknown beast_kind string must fail");
        assert!(
            matches!(err, RaceLoadError::Invalid(reason) if reason.contains("unknown beast_kind"))
        );
    }

    #[test]
    fn beast_kind_claimed_by_two_races_rejected() {
        let body_plans = body_plans_with_humanoid_and("humanoid", &["head"]);
        let err = RaceRegistry::from_parts_for_test(
            vec![
                human_entry(),
                RaceEntry {
                    id: RaceId::new("race_a"),
                    display_name: "甲".to_string(),
                    body_plan_id: BodyPlanId::new("humanoid"),
                    beast_kinds: vec!["rat".to_string()],
                },
                RaceEntry {
                    id: RaceId::new("race_b"),
                    display_name: "乙".to_string(),
                    body_plan_id: BodyPlanId::new("humanoid"),
                    beast_kinds: vec!["rat".to_string()],
                },
            ],
            vec![],
            &body_plans,
        )
        .expect_err("beast_kind claimed twice must fail");
        assert!(
            matches!(err, RaceLoadError::Invalid(reason) if reason.contains("claimed by both"))
        );
    }

    #[test]
    fn morph_pair_happy_path_round_trips() {
        let body_plans = body_plans_with_humanoid_and("whale", &["skull", "tail_fin"]);
        let mut part_mapping = HashMap::new();
        part_mapping.insert(BodyPartId::new("tail_fin"), BodyPartId::new("leg_l"));
        let races = RaceRegistry::from_parts_for_test(
            vec![
                human_entry(),
                RaceEntry {
                    id: RaceId::new("whale"),
                    display_name: "飞鲸".to_string(),
                    body_plan_id: BodyPlanId::new("whale"),
                    beast_kinds: vec!["whale".to_string()],
                },
            ],
            vec![MorphPairDef {
                from: RaceId::new(HUMAN_RACE_ID),
                to: RaceId::new("whale"),
                part_mapping,
            }],
            &body_plans,
        )
        .expect("valid morph pair should load");

        assert_eq!(races.morph_pair_count(), 1);
        let pair = races
            .morph_pair(&RaceId::new(HUMAN_RACE_ID), &RaceId::new("whale"))
            .expect("morph pair should be retrievable by (from, to) key");
        assert_eq!(
            pair.part_mapping.get(&BodyPartId::new("tail_fin")),
            Some(&BodyPartId::new("leg_l"))
        );
    }

    #[test]
    fn morph_pair_duplicate_from_to_rejected() {
        let body_plans = body_plans_with_humanoid_and("whale", &["skull"]);
        let races_entries = vec![
            human_entry(),
            RaceEntry {
                id: RaceId::new("whale"),
                display_name: "飞鲸".to_string(),
                body_plan_id: BodyPlanId::new("whale"),
                beast_kinds: vec![],
            },
        ];
        let pair = MorphPairDef {
            from: RaceId::new(HUMAN_RACE_ID),
            to: RaceId::new("whale"),
            part_mapping: HashMap::new(),
        };
        let err =
            RaceRegistry::from_parts_for_test(races_entries, vec![pair.clone(), pair], &body_plans)
                .expect_err("duplicate morph pair must fail");
        assert!(
            matches!(err, RaceLoadError::Invalid(reason) if reason.contains("duplicate morph pair"))
        );
    }

    #[test]
    fn morph_pair_dangling_from_race_rejected() {
        let body_plans = body_plans_with_humanoid_and("whale", &["skull"]);
        let err = RaceRegistry::from_parts_for_test(
            vec![human_entry()],
            vec![MorphPairDef {
                from: RaceId::new("ghost_race"),
                to: RaceId::new(HUMAN_RACE_ID),
                part_mapping: HashMap::new(),
            }],
            &body_plans,
        )
        .expect_err("dangling from-race must fail");
        assert!(
            matches!(err, RaceLoadError::Invalid(reason) if reason.contains("unknown from-race"))
        );
    }

    #[test]
    fn morph_pair_dangling_to_race_rejected() {
        let body_plans = body_plans_with_humanoid_and("whale", &["skull"]);
        let err = RaceRegistry::from_parts_for_test(
            vec![human_entry()],
            vec![MorphPairDef {
                from: RaceId::new(HUMAN_RACE_ID),
                to: RaceId::new("ghost_race"),
                part_mapping: HashMap::new(),
            }],
            &body_plans,
        )
        .expect_err("dangling to-race must fail");
        assert!(
            matches!(err, RaceLoadError::Invalid(reason) if reason.contains("unknown to-race"))
        );
    }

    #[test]
    fn morph_pair_part_mapping_key_dangling_in_to_plan_rejected() {
        let body_plans = body_plans_with_humanoid_and("whale", &["skull", "tail_fin"]);
        let mut part_mapping = HashMap::new();
        part_mapping.insert(
            BodyPartId::new("nonexistent_form_part"),
            BodyPartId::new("leg_l"),
        );
        let err = RaceRegistry::from_parts_for_test(
            vec![
                human_entry(),
                RaceEntry {
                    id: RaceId::new("whale"),
                    display_name: "飞鲸".to_string(),
                    body_plan_id: BodyPlanId::new("whale"),
                    beast_kinds: vec![],
                },
            ],
            vec![MorphPairDef {
                from: RaceId::new(HUMAN_RACE_ID),
                to: RaceId::new("whale"),
                part_mapping,
            }],
            &body_plans,
        )
        .expect_err("dangling part_mapping key must fail");
        assert!(
            matches!(err, RaceLoadError::Invalid(reason) if reason.contains("part_mapping key"))
        );
    }

    #[test]
    fn morph_pair_part_mapping_value_dangling_in_from_plan_rejected() {
        let body_plans = body_plans_with_humanoid_and("whale", &["skull", "tail_fin"]);
        let mut part_mapping = HashMap::new();
        part_mapping.insert(
            BodyPartId::new("tail_fin"),
            BodyPartId::new("nonexistent_intrinsic_part"),
        );
        let err = RaceRegistry::from_parts_for_test(
            vec![
                human_entry(),
                RaceEntry {
                    id: RaceId::new("whale"),
                    display_name: "飞鲸".to_string(),
                    body_plan_id: BodyPlanId::new("whale"),
                    beast_kinds: vec![],
                },
            ],
            vec![MorphPairDef {
                from: RaceId::new(HUMAN_RACE_ID),
                to: RaceId::new("whale"),
                part_mapping,
            }],
            &body_plans,
        )
        .expect_err("dangling part_mapping value must fail");
        assert!(
            matches!(err, RaceLoadError::Invalid(reason) if reason.contains("part_mapping value"))
        );
    }

    /// plan-race-system-v1 P4（决议 #2 单射校验）—— 两个不同 form_part 映射到同一
    /// intrinsic_part 必须在加载期拒绝，而不是静默接受后在护甲折算/命中折算时产生
    /// 一对多歧义。
    #[test]
    fn morph_pair_part_mapping_duplicate_intrinsic_value_rejected() {
        let body_plans = body_plans_with_humanoid_and("whale", &["skull", "tail_fin", "fin_l"]);
        let mut part_mapping = HashMap::new();
        part_mapping.insert(BodyPartId::new("tail_fin"), BodyPartId::new("leg_l"));
        part_mapping.insert(BodyPartId::new("fin_l"), BodyPartId::new("leg_l"));
        let err = RaceRegistry::from_parts_for_test(
            vec![
                human_entry(),
                RaceEntry {
                    id: RaceId::new("whale"),
                    display_name: "飞鲸".to_string(),
                    body_plan_id: BodyPlanId::new("whale"),
                    beast_kinds: vec![],
                },
            ],
            vec![MorphPairDef {
                from: RaceId::new(HUMAN_RACE_ID),
                to: RaceId::new("whale"),
                part_mapping,
            }],
            &body_plans,
        )
        .expect_err("part_mapping 值重复（两个 form 部位映射到同一 intrinsic 部位）必须拒绝加载");
        assert!(
            matches!(&err, RaceLoadError::Invalid(reason) if reason.contains("not injective")),
            "错误信息应指出非单射，实际: {err:?}"
        );
    }

    /// part_mapping 部分单射（值不重复但有缺项）必须仍能正常加载——单射只禁止值重复，
    /// 不要求满射（`to` 部位数可以多于/少于映射条目数）。
    #[test]
    fn morph_pair_part_mapping_partial_injective_map_accepted() {
        let body_plans = body_plans_with_humanoid_and("whale", &["skull", "tail_fin", "fin_l"]);
        let mut part_mapping = HashMap::new();
        part_mapping.insert(BodyPartId::new("tail_fin"), BodyPartId::new("leg_l"));
        part_mapping.insert(BodyPartId::new("fin_l"), BodyPartId::new("leg_r"));
        // "skull" 故意不映射——单射校验不要求满射。
        let races = RaceRegistry::from_parts_for_test(
            vec![
                human_entry(),
                RaceEntry {
                    id: RaceId::new("whale"),
                    display_name: "飞鲸".to_string(),
                    body_plan_id: BodyPlanId::new("whale"),
                    beast_kinds: vec![],
                },
            ],
            vec![MorphPairDef {
                from: RaceId::new(HUMAN_RACE_ID),
                to: RaceId::new("whale"),
                part_mapping,
            }],
            &body_plans,
        )
        .expect("部分单射（有缺项但值不重复）应合法加载");
        assert_eq!(races.morph_pair_count(), 1);
    }

    /// `MorphPairDef::intrinsic_part_for` / `form_part_for_intrinsic` 正反查询。
    #[test]
    fn morph_pair_def_intrinsic_part_for_and_reverse_lookup() {
        let mut part_mapping = HashMap::new();
        part_mapping.insert(BodyPartId::new("tail_fin"), BodyPartId::new("leg_l"));
        let pair = MorphPairDef {
            from: RaceId::new(HUMAN_RACE_ID),
            to: RaceId::new("whale"),
            part_mapping,
        };
        assert_eq!(
            pair.intrinsic_part_for(&BodyPartId::new("tail_fin")),
            Some(&BodyPartId::new("leg_l"))
        );
        assert_eq!(pair.intrinsic_part_for(&BodyPartId::new("skull")), None);
        assert_eq!(
            pair.form_part_for_intrinsic(&BodyPartId::new("leg_l")),
            Some(&BodyPartId::new("tail_fin"))
        );
        assert_eq!(
            pair.form_part_for_intrinsic(&BodyPartId::new("leg_r")),
            None
        );
    }

    /// `resolve_morph_pair` 是 `morph_pair` 的别名，行为必须完全一致。
    #[test]
    fn resolve_morph_pair_is_alias_of_morph_pair() {
        let body_plans = body_plans_with_humanoid_and("whale", &["skull", "tail_fin"]);
        let mut part_mapping = HashMap::new();
        part_mapping.insert(BodyPartId::new("tail_fin"), BodyPartId::new("leg_l"));
        let races = RaceRegistry::from_parts_for_test(
            vec![
                human_entry(),
                RaceEntry {
                    id: RaceId::new("whale"),
                    display_name: "飞鲸".to_string(),
                    body_plan_id: BodyPlanId::new("whale"),
                    beast_kinds: vec![],
                },
            ],
            vec![MorphPairDef {
                from: RaceId::new(HUMAN_RACE_ID),
                to: RaceId::new("whale"),
                part_mapping,
            }],
            &body_plans,
        )
        .expect("valid morph pair should load");
        assert_eq!(
            races.resolve_morph_pair(&RaceId::new(HUMAN_RACE_ID), &RaceId::new("whale")),
            races.morph_pair(&RaceId::new(HUMAN_RACE_ID), &RaceId::new("whale")),
        );
    }

    /// `morph_targets_from`：空 registry 返回空；多目标按字典序确定性排序；查询
    /// 无关种族（无正向 pair）返回空，不 panic。
    #[test]
    fn morph_targets_from_empty_when_no_pairs_declared() {
        let body_plans = body_plans_with_humanoid_and("humanoid", &["head"]);
        let races = RaceRegistry::from_parts_for_test(vec![human_entry()], vec![], &body_plans)
            .expect("human-only races.json should load");
        assert!(races
            .morph_targets_from(&RaceId::new(HUMAN_RACE_ID))
            .is_empty());
    }

    #[test]
    fn morph_targets_from_returns_sorted_targets_for_matching_from_race() {
        let body_plans = BodyPlanRegistry::from_plans(vec![
            plan_with_parts("humanoid", &["head", "chest", "leg_l", "leg_r"]),
            plan_with_parts("whale", &["skull"]),
            plan_with_parts("beetle", &["carapace"]),
        ])
        .expect("fixture plans should validate");
        let races = RaceRegistry::from_parts_for_test(
            vec![
                human_entry(),
                RaceEntry {
                    id: RaceId::new("whale"),
                    display_name: "飞鲸".to_string(),
                    body_plan_id: BodyPlanId::new("whale"),
                    beast_kinds: vec![],
                },
                RaceEntry {
                    id: RaceId::new("beetle"),
                    display_name: "甲虫".to_string(),
                    body_plan_id: BodyPlanId::new("beetle"),
                    beast_kinds: vec![],
                },
            ],
            vec![
                MorphPairDef {
                    from: RaceId::new(HUMAN_RACE_ID),
                    to: RaceId::new("whale"),
                    part_mapping: HashMap::new(),
                },
                MorphPairDef {
                    from: RaceId::new(HUMAN_RACE_ID),
                    to: RaceId::new("beetle"),
                    part_mapping: HashMap::new(),
                },
            ],
            &body_plans,
        )
        .expect("two morph pairs from human should load");

        let targets = races.morph_targets_from(&RaceId::new(HUMAN_RACE_ID));
        assert_eq!(
            targets,
            vec![&RaceId::new("beetle"), &RaceId::new("whale")],
            "多目标必须按 to race id 字典序确定性排序"
        );
        assert!(
            races.morph_targets_from(&RaceId::new("whale")).is_empty(),
            "whale 未声明任何 from=whale 的 pair，必须返回空而非 panic"
        );
    }

    #[test]
    fn empty_morph_pairs_is_valid() {
        let body_plans = body_plans_with_humanoid_and("humanoid", &["head"]);
        let races = RaceRegistry::from_parts_for_test(vec![human_entry()], vec![], &body_plans)
            .expect("empty morph_pairs is a valid state (P0 production races.json)");
        assert_eq!(races.morph_pair_count(), 0);
    }

    // ─────────────────────────────────────────────────────────────────────────
    // plan-race-system-v1 P5/PR-6a §8#9 —— meridian_mappings 加载期校验 + 查询。
    // ─────────────────────────────────────────────────────────────────────────

    fn plan_with_meridian_channels(id: &str, channel_ids: &[&str]) -> BodyPlan {
        use crate::body_plan::types::{
            ChannelDef, MeridianFamily, MeridianProfile, RealmMeridianReq,
        };

        let mut plan = plan_with_parts(id, &["core"]);
        plan.meridian_profile = Some(MeridianProfile {
            channels: channel_ids
                .iter()
                .map(|cid| ChannelDef {
                    id: MeridianChannelId::new(*cid),
                    family: MeridianFamily::Regular,
                    body_part: None,
                    roles: vec![],
                })
                .collect(),
            topology_edges: vec![],
            realm_requirements: [RealmMeridianReq::default(); 6],
            dugu_injection: vec![],
        });
        plan
    }

    fn human_and_whale_body_plans_with_channels(
        human_channels: &[&str],
        whale_channels: &[&str],
    ) -> BodyPlanRegistry {
        BodyPlanRegistry::from_plans(vec![
            plan_with_meridian_channels("humanoid", human_channels),
            plan_with_meridian_channels("whale", whale_channels),
        ])
        .expect("fixture plans with meridian_profile should validate")
    }

    fn human_whale_race_entries() -> Vec<RaceEntry> {
        vec![
            human_entry(),
            RaceEntry {
                id: RaceId::new("whale"),
                display_name: "飞鲸".to_string(),
                body_plan_id: BodyPlanId::new("whale"),
                beast_kinds: vec![],
            },
        ]
    }

    #[test]
    fn meridian_mapping_happy_path_round_trips_and_is_queryable() {
        let body_plans =
            human_and_whale_body_plans_with_channels(&["lung", "heart"], &["skull", "fin"]);
        let races = RaceRegistry::from_parts_for_test_with_meridian_mappings(
            human_whale_race_entries(),
            vec![],
            vec![MeridianMappingDef {
                from: RaceId::new(HUMAN_RACE_ID),
                to: RaceId::new("whale"),
                entries: vec![(
                    MeridianChannelId::new("lung"),
                    MeridianChannelId::new("fin"),
                )],
            }],
            &body_plans,
        )
        .expect("valid meridian mapping should load");

        assert_eq!(races.meridian_mapping_count(), 1);
        let mapping = races
            .meridian_mapping(&RaceId::new(HUMAN_RACE_ID), &RaceId::new("whale"))
            .expect("mapping should be retrievable by (from, to) key");
        assert_eq!(
            mapping.target_for(&MeridianChannelId::new("lung")),
            Some(&MeridianChannelId::new("fin"))
        );
        assert_eq!(
            mapping.source_for(&MeridianChannelId::new("fin")),
            Some(&MeridianChannelId::new("lung"))
        );
        assert_eq!(mapping.target_for(&MeridianChannelId::new("heart")), None);
        assert_eq!(mapping.source_for(&MeridianChannelId::new("skull")), None);
    }

    #[test]
    fn meridian_mapping_absent_pair_returns_none() {
        let body_plans = human_and_whale_body_plans_with_channels(&["lung"], &["fin"]);
        let races = RaceRegistry::from_parts_for_test_with_meridian_mappings(
            human_whale_race_entries(),
            vec![],
            vec![],
            &body_plans,
        )
        .expect("empty meridian_mappings is a valid state");
        assert_eq!(races.meridian_mapping_count(), 0);
        assert!(races
            .meridian_mapping(&RaceId::new(HUMAN_RACE_ID), &RaceId::new("whale"))
            .is_none());
    }

    #[test]
    fn meridian_mapping_duplicate_from_to_rejected() {
        let body_plans = human_and_whale_body_plans_with_channels(&["lung"], &["fin"]);
        let mapping = MeridianMappingDef {
            from: RaceId::new(HUMAN_RACE_ID),
            to: RaceId::new("whale"),
            entries: vec![],
        };
        let err = RaceRegistry::from_parts_for_test_with_meridian_mappings(
            human_whale_race_entries(),
            vec![],
            vec![mapping.clone(), mapping],
            &body_plans,
        )
        .expect_err("duplicate meridian mapping must fail");
        assert!(
            matches!(err, RaceLoadError::Invalid(reason) if reason.contains("duplicate meridian mapping"))
        );
    }

    #[test]
    fn meridian_mapping_dangling_from_race_rejected() {
        let body_plans = human_and_whale_body_plans_with_channels(&["lung"], &["fin"]);
        let err = RaceRegistry::from_parts_for_test_with_meridian_mappings(
            vec![human_entry()],
            vec![],
            vec![MeridianMappingDef {
                from: RaceId::new("ghost_race"),
                to: RaceId::new(HUMAN_RACE_ID),
                entries: vec![],
            }],
            &body_plans,
        )
        .expect_err("dangling from-race must fail");
        assert!(
            matches!(err, RaceLoadError::Invalid(reason) if reason.contains("unknown from-race"))
        );
    }

    #[test]
    fn meridian_mapping_dangling_to_race_rejected() {
        let body_plans = human_and_whale_body_plans_with_channels(&["lung"], &["fin"]);
        let err = RaceRegistry::from_parts_for_test_with_meridian_mappings(
            vec![human_entry()],
            vec![],
            vec![MeridianMappingDef {
                from: RaceId::new(HUMAN_RACE_ID),
                to: RaceId::new("ghost_race"),
                entries: vec![],
            }],
            &body_plans,
        )
        .expect_err("dangling to-race must fail");
        assert!(
            matches!(err, RaceLoadError::Invalid(reason) if reason.contains("unknown to-race"))
        );
    }

    #[test]
    fn meridian_mapping_entry_from_channel_not_in_from_plan_rejected() {
        let body_plans = human_and_whale_body_plans_with_channels(&["lung"], &["fin"]);
        let err = RaceRegistry::from_parts_for_test_with_meridian_mappings(
            human_whale_race_entries(),
            vec![],
            vec![MeridianMappingDef {
                from: RaceId::new(HUMAN_RACE_ID),
                to: RaceId::new("whale"),
                entries: vec![(
                    MeridianChannelId::new("nonexistent_channel"),
                    MeridianChannelId::new("fin"),
                )],
            }],
            &body_plans,
        )
        .expect_err("dangling from-channel must fail");
        assert!(
            matches!(err, RaceLoadError::Invalid(reason) if reason.contains("from-channel") && reason.contains("not found"))
        );
    }

    #[test]
    fn meridian_mapping_entry_to_channel_not_in_to_plan_rejected() {
        let body_plans = human_and_whale_body_plans_with_channels(&["lung"], &["fin"]);
        let err = RaceRegistry::from_parts_for_test_with_meridian_mappings(
            human_whale_race_entries(),
            vec![],
            vec![MeridianMappingDef {
                from: RaceId::new(HUMAN_RACE_ID),
                to: RaceId::new("whale"),
                entries: vec![(
                    MeridianChannelId::new("lung"),
                    MeridianChannelId::new("nonexistent_channel"),
                )],
            }],
            &body_plans,
        )
        .expect_err("dangling to-channel must fail");
        assert!(
            matches!(err, RaceLoadError::Invalid(reason) if reason.contains("to-channel") && reason.contains("not found"))
        );
    }

    #[test]
    fn meridian_mapping_one_to_many_from_channel_rejected() {
        // 同一个 from-channel 被两条 entry 同时映射 —— 一对多，禁止。
        let body_plans = human_and_whale_body_plans_with_channels(&["lung"], &["fin", "tail"]);
        let err = RaceRegistry::from_parts_for_test_with_meridian_mappings(
            human_whale_race_entries(),
            vec![],
            vec![MeridianMappingDef {
                from: RaceId::new(HUMAN_RACE_ID),
                to: RaceId::new("whale"),
                entries: vec![
                    (
                        MeridianChannelId::new("lung"),
                        MeridianChannelId::new("fin"),
                    ),
                    (
                        MeridianChannelId::new("lung"),
                        MeridianChannelId::new("tail"),
                    ),
                ],
            }],
            &body_plans,
        )
        .expect_err("one from-channel mapped twice (1:N) must fail");
        assert!(
            matches!(&err, RaceLoadError::Invalid(reason) if reason.contains("not one-to-one") && reason.contains("mapped more than once")),
            "got {err:?}"
        );
    }

    #[test]
    fn meridian_mapping_many_to_one_to_channel_rejected() {
        // 两个不同 from-channel 映射到同一个 to-channel —— 多对一，禁止。
        let body_plans = human_and_whale_body_plans_with_channels(&["lung", "heart"], &["fin"]);
        let err = RaceRegistry::from_parts_for_test_with_meridian_mappings(
            human_whale_race_entries(),
            vec![],
            vec![MeridianMappingDef {
                from: RaceId::new(HUMAN_RACE_ID),
                to: RaceId::new("whale"),
                entries: vec![
                    (
                        MeridianChannelId::new("lung"),
                        MeridianChannelId::new("fin"),
                    ),
                    (
                        MeridianChannelId::new("heart"),
                        MeridianChannelId::new("fin"),
                    ),
                ],
            }],
            &body_plans,
        )
        .expect_err("two from-channels targeting one to-channel (N:1) must fail");
        assert!(
            matches!(&err, RaceLoadError::Invalid(reason) if reason.contains("not one-to-one") && reason.contains("targeted by more than one")),
            "got {err:?}"
        );
    }

    #[test]
    fn meridian_mapping_referencing_body_plan_without_meridian_profile_rejected() {
        // to-race body plan 无 meridian_profile（None）——channel 集合视为空，任何引用必拒。
        let body_plans = BodyPlanRegistry::from_plans(vec![
            plan_with_meridian_channels("humanoid", &["lung"]),
            plan_with_parts("no_meridian_plan", &["core"]),
        ])
        .expect("fixture plans should validate");
        let races_entries = vec![
            human_entry(),
            RaceEntry {
                id: RaceId::new("mindless"),
                display_name: "无脉兽".to_string(),
                body_plan_id: BodyPlanId::new("no_meridian_plan"),
                beast_kinds: vec![],
            },
        ];
        let err = RaceRegistry::from_parts_for_test_with_meridian_mappings(
            races_entries,
            vec![],
            vec![MeridianMappingDef {
                from: RaceId::new(HUMAN_RACE_ID),
                to: RaceId::new("mindless"),
                entries: vec![(
                    MeridianChannelId::new("lung"),
                    MeridianChannelId::new("lung"),
                )],
            }],
            &body_plans,
        )
        .expect_err("to-race without meridian_profile must reject any channel reference");
        assert!(
            matches!(err, RaceLoadError::Invalid(reason) if reason.contains("to-channel") && reason.contains("not found"))
        );
    }

    #[test]
    fn meridian_mapping_empty_entries_is_valid() {
        // 声明了 mapping 但 entries 为空——合法状态（所有 from 侧经脉都进休眠登记）。
        let body_plans = human_and_whale_body_plans_with_channels(&["lung"], &["fin"]);
        let races = RaceRegistry::from_parts_for_test_with_meridian_mappings(
            human_whale_race_entries(),
            vec![],
            vec![MeridianMappingDef {
                from: RaceId::new(HUMAN_RACE_ID),
                to: RaceId::new("whale"),
                entries: vec![],
            }],
            &body_plans,
        )
        .expect("empty entries is a valid mapping declaration");
        let mapping = races
            .meridian_mapping(&RaceId::new(HUMAN_RACE_ID), &RaceId::new("whale"))
            .unwrap();
        assert!(mapping.entries.is_empty());
    }

    /// review A/B/C/D 共识 MAJOR —— 真实 races.json 的 `human<->whale` 双向经脉映射
    /// per-key pin。当前 20 条都是恒等映射（human 20 条 TCM 经脉 id 逐一映射到 whale
    /// 同名 channel），删字段 / 删单个方向 / 清空 entries 必须撞红此测试。
    ///
    /// **不锁全局 `meridian_mapping_count()` 总数**——PR-6c 引入真正跨构型映射（beast
    /// 种族补 `human<->beast_common`/`whale<->beast_common`）时那两对 key 的存在不
    /// 应影响本测试；本测试只按 `(from, to)` 具体 key 查询，与总数无关。
    #[test]
    fn real_races_json_asset_meridian_mappings_load_and_validate() {
        // 20 条 TCM 经脉 id——当前 races.json human<->whale 双向都声明为恒等映射
        // （from==to）。PR-6c 换真跨构型映射时，连同这份常量一起改。
        const EXPECTED_IDENTITY_CHANNELS: [&str; 20] = [
            "lung",
            "large_intestine",
            "stomach",
            "spleen",
            "heart",
            "small_intestine",
            "bladder",
            "kidney",
            "pericardium",
            "triple_energizer",
            "gallbladder",
            "liver",
            "ren",
            "du",
            "chong",
            "dai",
            "yin_qiao",
            "yang_qiao",
            "yin_wei",
            "yang_wei",
        ];

        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let plans_dir = manifest_dir.join(super::super::registry::DEFAULT_BODY_PLANS_DIR);
        let races_path = manifest_dir.join(DEFAULT_RACES_PATH);

        let body_plans = BodyPlanRegistry::load_dir(&plans_dir).expect("real plans/ should load");
        let races =
            RaceRegistry::load_file(&races_path, &body_plans).expect("real races.json should load");

        let expected: HashSet<(MeridianChannelId, MeridianChannelId)> = EXPECTED_IDENTITY_CHANNELS
            .iter()
            .map(|id| (MeridianChannelId::new(*id), MeridianChannelId::new(*id)))
            .collect();

        let human_to_whale = races
            .meridian_mapping(&RaceId::new(HUMAN_RACE_ID), &RaceId::new("whale"))
            .expect("races.json must declare human->whale meridian_mapping");
        assert_eq!(
            human_to_whale.entries.len(),
            20,
            "human->whale 必须精确声明 20 条经脉迁移条目"
        );
        let human_to_whale_set: HashSet<(MeridianChannelId, MeridianChannelId)> =
            human_to_whale.entries.iter().cloned().collect();
        assert_eq!(
            human_to_whale_set, expected,
            "human->whale 的 20 条 source/target identity 对必须与预期集合完全一致"
        );

        let whale_to_human = races
            .meridian_mapping(&RaceId::new("whale"), &RaceId::new(HUMAN_RACE_ID))
            .expect("races.json must declare whale->human meridian_mapping (reverse direction)");
        assert_eq!(
            whale_to_human.entries.len(),
            20,
            "whale->human 必须精确声明 20 条经脉迁移条目"
        );
        let whale_to_human_set: HashSet<(MeridianChannelId, MeridianChannelId)> =
            whale_to_human.entries.iter().cloned().collect();
        assert_eq!(
            whale_to_human_set, expected,
            "whale->human 的 20 条 source/target identity 对必须与预期集合完全一致"
        );
    }

    #[test]
    fn parse_beast_kind_covers_all_sixteen_variants_round_trip() {
        for kind in ALL_BEAST_KINDS.iter().copied() {
            assert_eq!(parse_beast_kind(kind.as_str()), Some(kind));
        }
        assert_eq!(parse_beast_kind("not_a_real_kind"), None);
    }

    #[test]
    fn load_file_missing_path_returns_io_error() {
        let body_plans = body_plans_with_humanoid_and("humanoid", &["head"]);
        let missing = std::env::temp_dir().join(format!(
            "bong-race-registry-test-missing-{}.json",
            std::process::id()
        ));
        let err = RaceRegistry::load_file(&missing, &body_plans)
            .expect_err("nonexistent races.json path must fail");
        assert!(matches!(err, RaceLoadError::Io(_)));
    }

    #[test]
    fn load_file_malformed_json_returns_json_error() {
        let body_plans = body_plans_with_humanoid_and("humanoid", &["head"]);
        let path = std::env::temp_dir().join(format!(
            "bong-race-registry-test-malformed-{}.json",
            std::process::id()
        ));
        std::fs::write(&path, "{ not valid json").expect("write malformed json");
        let err =
            RaceRegistry::load_file(&path, &body_plans).expect_err("malformed json must fail");
        assert!(matches!(err, RaceLoadError::Json { .. }));
        let _ = std::fs::remove_file(&path);
    }

    /// 「混装文件进错目录」反例的反向验证：`BodyPlan` 形状的文件（顶层 `id`/`parts`/
    /// `hit_geometry`/`equip_slots`，没有 `races` 数组）被误当成 `races.json` 路径喂给
    /// `RaceRegistry::load_file`——`races` 是必填字段（无 `#[serde(default)]`），必须在
    /// 解析阶段就 fail-fast 报 `Json` 错误，而不是静默解析出一个空 `RaceRegistry`。
    #[test]
    fn load_file_rejects_body_plan_shape_misplaced_as_races_file() {
        let body_plans = body_plans_with_humanoid_and("humanoid", &["head"]);
        let path = std::env::temp_dir().join(format!(
            "bong-race-registry-test-misplaced-body-plan-{}.json",
            std::process::id()
        ));
        let body_plan_shaped_json = r#"{
            "id": "humanoid",
            "display_name": "人形",
            "is_humanoid": true,
            "parts": [],
            "hit_geometry": {"mode": "part_boxes", "boxes": []},
            "equip_slots": []
        }"#;
        std::fs::write(&path, body_plan_shaped_json).expect("write misplaced BodyPlan-shaped file");

        let err = RaceRegistry::load_file(&path, &body_plans).expect_err(
            "BodyPlan 形状的文件被误当成 races.json 路径必须在解析期报错，而不是被静默接受",
        );
        assert!(
            matches!(err, RaceLoadError::Json { .. }),
            "expected Json parse error for a BodyPlan-shaped file lacking the required \
             `races` array, got {err:?}"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn real_races_json_asset_has_human_to_whale_morph_pair() {
        // plan-race-system-v1 PR-5b —— 生产 races.json 必须声明 human→whale 正向
        // morph_pair，否则 `morph.yixing` cast 永远 InvalidTarget（P4 遗留的已知缺口，
        // 本 PR 补上真数据让 cast 端到端可用）。
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let plans_dir = manifest_dir.join(super::super::registry::DEFAULT_BODY_PLANS_DIR);
        let races_path = manifest_dir.join(DEFAULT_RACES_PATH);

        let body_plans = BodyPlanRegistry::load_dir(&plans_dir).expect("real plans/ should load");
        let races =
            RaceRegistry::load_file(&races_path, &body_plans).expect("real races.json should load");

        assert!(
            races.get(&RaceId::new("whale")).is_some(),
            "races.json 必须声明独立的 whale 种族条目（易形目标）"
        );
        let targets = races.morph_targets_from(&RaceId::new(HUMAN_RACE_ID));
        assert!(
            targets.contains(&&RaceId::new("whale")),
            "human 必须有正向 morph_pair 指向 whale，实际 targets={targets:?}"
        );
    }

    #[test]
    fn real_races_json_asset_loads_and_covers_all_beast_kinds() {
        // 端到端锚点：真实 server/assets/body_plans/races.json 必须能在真实
        // BodyPlanRegistry（同样从磁盘加载 humanoid.json）之上通过全部校验，且
        // 16 个 BeastKind 变体全部能查到归属种族（不落入 fallback 分支）。
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let plans_dir = manifest_dir.join(super::super::registry::DEFAULT_BODY_PLANS_DIR);
        let races_path = manifest_dir.join(DEFAULT_RACES_PATH);

        let body_plans = BodyPlanRegistry::load_dir(&plans_dir).expect("real plans/ should load");
        let races =
            RaceRegistry::load_file(&races_path, &body_plans).expect("real races.json should load");

        assert!(races.get(&RaceId::new(HUMAN_RACE_ID)).is_some());
        for kind in ALL_BEAST_KINDS.iter().copied() {
            let race_id = races.race_id_for_beast_kind(kind);
            assert!(
                races.get(&race_id).is_some(),
                "beast_kind {:?} resolved to race {race_id} which is not registered",
                kind.as_str()
            );
            assert_ne!(
                race_id.as_str(),
                HUMAN_RACE_ID,
                "beast_kind {:?} fell back to the human default — races.json 必须显式登记 \
                 全部 16 个 BeastKind 变体，不能依赖 fallback 分支",
                kind.as_str()
            );
        }
    }
}
