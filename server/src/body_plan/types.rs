//! plan-race-system-v1 P0 — 通用身体构型数据模型。
//!
//! `BodyPlan` 是「部位 / 命中几何 / 装备槽 / 经脉档案」的数据驱动真源，替代当前散落在
//! `combat::components::BodyPart`（8 段人形 unit enum）+ `combat::raycast`（唯一 1.8m
//! 直立 AABB）+ `combat::resolve::body_part_multipliers`（8 分支 match）里的硬编码人形
//! 假设。P0 只交付类型 + `humanoid.json` 与现状 bit-for-bit 对齐的数据；`combat::*` 消费点
//! 改造（`body_part_multipliers` / `classify_body_part` / `standing_humanoid_aabb` /
//! `carrier.rs` 投射物分支）在后续阶段接入，本文件不改动 `combat` 模块任何行为。

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use valence::prelude::{bevy_ecs, Component};

use crate::dandao::mutation::BodySlot;
use crate::schema::inventory::EquipSlotV1;

/// `BodyPlanRegistry` 主键——`server/assets/body_plans/plans/*.json` 每个文件的 `id` 字段。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BodyPlanId(pub String);

impl BodyPlanId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for BodyPlanId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<String> for BodyPlanId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl std::fmt::Display for BodyPlanId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// 部位 id（string，取代 `combat::components::BodyPart` unit enum 的封闭 8 段假设）。
/// humanoid plan 沿用现有 wire 字符串（`head`/`chest`/`back`/`abdomen`/`arm_l`/`arm_r`/
/// `leg_l`/`leg_r`，见 `network::cast_emit::parse_wound_heal_body_part`），确保 P0b 接入
/// 消费点时无需重新定义映射表。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BodyPartId(pub String);

impl BodyPartId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for BodyPartId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<String> for BodyPartId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl std::fmt::Display for BodyPartId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// 种族 id（string，`server/assets/body_plans/races.json` 的 `races[].id`）。
/// `Cultivation.race` 默认值、`IntrinsicRace` 组件负载均为本类型。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RaceId(pub String);

impl RaceId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for RaceId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<String> for RaceId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl std::fmt::Display for RaceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// plan-race-system-v1 P3a —— 装备 / 功法种族三档匹配门（决议 §8.1 #5/#6）。
///
/// 三档语义：`Any` 全通用（不看种族）；`Humanoid` 人形通用（判据 = 判定域 BodyPlan 的
/// `is_humanoid` 字段，不做种族名单硬编码）；`Species` 种族专属（精确 `RaceId` 白名单）。
///
/// `Species` 携带 `&'static [&'static str]`（而非 `&'static [RaceId]`）——`RaceId` 内部是
/// `String`，无法出现在 `const` 数组字面量里（`String::from` 不是 const fn），而
/// [`TechniqueDefinition`]（`known_techniques.rs`）的 48 条定义是 `Copy` + `const` 数组，
/// 本类型必须能在同一 const 上下文构造。比对用 [`RaceGate::allows`]，内部按
/// `RaceId::as_str()` 做字符串比较，语义与「`&'static [RaceId]`」完全等价。owned 场景
/// （`ItemTemplate` TOML 运行时加载）用 [`RaceGateOwned`]。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RaceGate {
    #[default]
    Any,
    Humanoid,
    Species(&'static [&'static str]),
}

impl RaceGate {
    /// 判定给定身份（`race_id` + 判定域 `is_humanoid`）是否通过本门。
    pub fn allows(&self, race_id: &RaceId, is_humanoid: bool) -> bool {
        match self {
            RaceGate::Any => true,
            RaceGate::Humanoid => is_humanoid,
            RaceGate::Species(allowed) => allowed.iter().any(|id| *id == race_id.as_str()),
        }
    }
}

/// [`RaceGate`] 的 owned / serde 形态——`ItemTemplate.wearer_race`（TOML 运行时加载，
/// P3b）等场景用。wire 形状为 tagged struct `{kind: "any"|"humanoid"|"species", species:
/// [...]}`（`kind` 用 string tag 而非 proto enum，避免枚举前缀 noOp——见
/// `plan-wire-format-bridge-v1` 教训；`species` 仅 `kind="species"` 时携带）；
/// serde 内部标签枚举对未知 `kind` 天然拒绝反序列化（fail-closed，非静默兜底 `Any`），
/// 与 `PartConsequence`（同文件）同一惯例。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum RaceGateOwned {
    #[default]
    Any,
    Humanoid,
    Species {
        species: Vec<RaceId>,
    },
}

impl RaceGateOwned {
    pub fn allows(&self, race_id: &RaceId, is_humanoid: bool) -> bool {
        match self {
            RaceGateOwned::Any => true,
            RaceGateOwned::Humanoid => is_humanoid,
            RaceGateOwned::Species { species } => species.contains(race_id),
        }
    }
}

/// 部位受击后果语义（决议 §P0）——枚举化现有「腿伤减速 / 头伤眩晕 / 臂伤六维」的隐性分类，
/// 非人形部位挂同一枚举（如鲸尾鳍 = `Locomotion`）。
///
/// - `Locomotion`：移动相关（人形 `leg_l`/`leg_r`，对应 `movement::leg_wound`）。
/// - `Sensory`：感知相关（人形 `head`，眩晕/视觉减益）。
/// - `Manipulator { main_hand }`：持械/操作相关（人形 `arm_l`/`arm_r`，对应
///   `combat::arm_wound`；`main_hand` 标记是否为主手——人形 `MAIN_ARM = ArmR`，见
///   `combat/arm_wound.rs:97`）。
/// - `Core`：躯干核心，无肢体级功能性后果（人形 `chest`/`back`/`abdomen`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum PartConsequence {
    Locomotion,
    Sensory,
    Manipulator { main_hand: bool },
    Core,
}

/// 单个部位的伤害/污染/流血倍率 + 功能性后果标签。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BodyPartDef {
    pub id: BodyPartId,
    pub damage_mul: f32,
    pub contam_mul: f32,
    pub bleed_mul: f32,
    pub consequence: PartConsequence,
}

/// `HeightBands` 模式的直立包围盒规格——`combat::raycast::standing_humanoid_aabb` 的
/// 参数化（`half_width`/`height` 对应现 `STANDING_HALF_WIDTH=0.3`/`STANDING_HEIGHT=1.8`）。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StandingAabbSpec {
    pub half_width: f64,
    pub height: f64,
}

/// 单条高度带的部位判定规则（见 `body_plan::geometry::classify_height_bands`）。
///
/// - `Single`：无论 lateral 为何值，恒定指派该部位（人形 `head`/`abdomen`）。
/// - `LateralSplitWithCenter`：`|lateral| > HeightBands.lateral_threshold` 时按符号分
///   `left`/`right`，否则落 `center`——唯一消费顶层 `lateral_threshold` 的分支（人形
///   胸/臂分支，对应现 `ARM_LATERAL_THRESHOLD`）。
/// - `LateralSplit`：任意非零 lateral 立即按符号分 `left`/`right`，无阈值、无 center
///   兜底（人形腿分支——现 `classify_body_part` 的 `else` 分支没有独立阈值）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum HeightBandAssignment {
    Single {
        part: BodyPartId,
    },
    LateralSplitWithCenter {
        left: BodyPartId,
        right: BodyPartId,
        center: BodyPartId,
    },
    LateralSplit {
        left: BodyPartId,
        right: BodyPartId,
    },
}

impl HeightBandAssignment {
    /// 本条 assignment 引用到的全部部位 id（供 `validate_body_plan` 悬空检测使用）。
    pub fn referenced_part_ids(&self) -> Vec<&BodyPartId> {
        match self {
            HeightBandAssignment::Single { part } => vec![part],
            HeightBandAssignment::LateralSplitWithCenter {
                left,
                right,
                center,
            } => {
                vec![left, right, center]
            }
            HeightBandAssignment::LateralSplit { left, right } => vec![left, right],
        }
    }
}

/// 单条高度带：`rel_y`（命中点相对脚底高度 / 身高，clamp 到 `[0,1]`）严格大于
/// `min_rel_y` 时命中本带（`bands` 数组要求按 `min_rel_y` 严格降序排列，从上到下
/// 第一个匹配的带生效——`validate_body_plan` 校验排序与全覆盖，见 `validate.rs`）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HeightBand {
    pub min_rel_y: f64,
    pub assignment: HeightBandAssignment,
}

/// `PartBoxes` 模式下单个部位的局部系包围盒。坐标系：原点 = 实体位置，+Z 沿实体 yaw
/// 正前（P0 只支持 yaw，不做 pitch/roll，见 `body_plan::geometry`）。`priority` 用于
/// 等距命中时的稳定裁决（数值越大优先级越高；同 priority 再按声明顺序稳定裁决）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PartBox {
    pub part_id: BodyPartId,
    pub offset: [f64; 3],
    pub half_extents: [f64; 3],
    pub priority: i32,
}

/// 命中几何双模式：`HeightBands`（人形，`classify_body_part` 高度带 + 横向阈值的参数化）
/// / `PartBoxes`（非人形，逐部位局部盒——单一直立 AABB + 人体比例高度带表达不了飞鲸横长
/// 构型，见 plan §P0）。
///
/// 注：`PartBoxes` 用具名字段 `boxes` 而非 plan 原文的元组 `PartBoxes(Vec<PartBox>)`——
/// serde internally-tagged 枚举（`#[serde(tag = "mode")]`）不支持"newtype 包裹序列"的
/// 变体形状（序列化目标是 JSON 数组，无法与外层 tag 合并进同一个 JSON 对象），具名字段
/// 包一层是等价的最小改动。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
pub enum HitGeometry {
    HeightBands {
        aabb: StandingAabbSpec,
        bands: Vec<HeightBand>,
        lateral_threshold: f64,
    },
    PartBoxes {
        boxes: Vec<PartBox>,
    },
}

/// 经脉 family（沿用现有 12 正经 / 8 奇经二分——`realm_requirements` 子配额与
/// `MeridianSystem.regular`/`extraordinary` 分桶依据，见 `MeridianSystem::for_profile`）。
/// 与 `cultivation::components::MeridianFamily` 是两个独立类型——后者是 `MeridianId`
/// 的既有派生便捷方法（服务 legacy 桥接），本类型是 body_plan JSON schema 的权威定义，
/// 刻意不互相依赖以避免制造循环耦合。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MeridianFamily {
    Regular,
    Extraordinary,
}

/// 经脉在特定玩法场景里的语义角色标签（plan §P4 决议 —— `FormAnchor` = 易形前置检查
/// 关注的经脉；本类型 P1a 只声明枚举 + 序列化，尚无消费点，供 humanoid.json 提前给
/// Ren/Du 标注，P4 落地"易形前置＝本体 profile 内全部 form_anchor 已通且未断"时直接
/// 消费，不必再改一次 schema）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChannelRole {
    FormAnchor,
}

/// 单条经脉声明（`MeridianProfile.channels`）。
///
/// `body_part` 承接 `combat::baomai_v4::dead_armor::meridian_to_body_part` 私表的数据
/// （channel → 可选体表部位，`None` = 无可命中体表映射，如 6 条排除的奇经——见该函数
/// 文档）——本字段是 plan §P1 "经脉↔部位映射数据化"目标里**数据落地**的部分；
/// `dead_armor` 自身改为消费本字段（而非维护私表）与 `cultivation::dugu` 的另一张
/// **方向相反、语义不同**的私表（`body_part_to_meridian: BodyPart -> MeridianId`，
/// 体表命中→"排异注入哪条经脉"，多对一且值域不覆盖全部 20 条经脉，不是本字段的逆
/// 映射）留待后续 P1 消费点改造子阶段——P1a 范围只到"数据在 humanoid.json 里有唯一
/// 权威来源"，两张私表各自的运行时改造不在本次交付。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChannelDef {
    pub id: crate::cultivation::components::MeridianChannelId,
    pub family: MeridianFamily,
    #[serde(default)]
    pub body_part: Option<BodyPartId>,
    #[serde(default)]
    pub roles: Vec<ChannelRole>,
}

/// plan-race-system-v1 P1b —— `cultivation::dugu::body_part_to_meridian` 私表的数据
/// 落地（体表命中部位 → 排异毒素累积到哪条经脉，多对一、代表性映射，方向与值域都与
/// [`ChannelDef::body_part`] 不同——**不是**其逆函数，见 `ChannelDef` 文档的说明）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DuguInjectionEntry {
    pub body_part: BodyPartId,
    pub channel: crate::cultivation::components::MeridianChannelId,
}

/// 经脉拓扑边——无向，声明一次即代表双向相邻（替换 `cultivation::topology::
/// MeridianTopology::standard()` 的单张全局 Rust 图；P1a 只交付数据 + 校验，
/// `topology.rs` 及其消费点`meridian_open`/NPC 选招在后续 P1 子阶段改为读取本字段）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TopologyEdge {
    pub from: crate::cultivation::components::MeridianChannelId,
    pub to: crate::cultivation::components::MeridianChannelId,
}

/// 单一境界的经脉配额声明（§8.1 #8 "公式即数据"决议——每种构型在自己的
/// `realm_requirements` 里直接声明各境界所需 channel 总数与正/奇子配额，不设全局换算
/// 公式）。数组下标与 `cultivation::components::Realm::rank()` 对齐：
/// `realm_requirements[realm.rank() as usize - 1]`（rank 1..=6 对应 醒灵..化虚）。
/// `regular_min`/`extraordinary_min` 为 0 表示该境界不对相应 family 设子配额下限
/// （只受 `total` 约束）。
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RealmMeridianReq {
    pub total: u8,
    #[serde(default)]
    pub regular_min: u8,
    #[serde(default)]
    pub extraordinary_min: u8,
}

/// 经脉构型档案——`channels`（每条经脉的 id/family/体部映射/角色标签）+
/// `topology_edges`（拓扑邻接，替换 `MeridianTopology::standard()` 单张全局图）+
/// `realm_requirements`（六境界配额曲线，替换 `Realm::required_meridians` /
/// `breakthrough::breakthrough_precondition_error` 的硬编码 match，见 plan §P1）。
///
/// P0 仅锁定 `BodyPlan.meridian_profile: Option<MeridianProfile>` 的存在性语义；
/// **P1a 起本类型不再是空占位**——`validate_body_plan` 现在要求 `is_humanoid == true`
/// 的 plan 必须提供 `Some(meridian_profile)`（humanoid.json 缺省该字段不再合法，见
/// `validate.rs`），非人形 plan（P0 现存的测试 fixture）仍可留 `None`。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MeridianProfile {
    pub channels: Vec<ChannelDef>,
    #[serde(default)]
    pub topology_edges: Vec<TopologyEdge>,
    pub realm_requirements: [RealmMeridianReq; 6],
    /// plan-race-system-v1 P1b —— `cultivation::dugu::body_part_to_meridian` 私表数据
    /// （见 [`DuguInjectionEntry`]）。`#[serde(default)]` 空数组对非人形 plan / 未接入
    /// dugu 玩法的构型是合法状态（`resolve::dugu_injection_channel` 返回 `None`）。
    #[serde(default)]
    pub dugu_injection: Vec<DuguInjectionEntry>,
}

/// 单个种族/构型的完整身体定义。`is_humanoid` 是 P3 `RaceGate::Humanoid` 档的唯一判据
/// （不做名单硬编码）；易形配对不在本结构体内——唯一真源是 `races.json` 全局
/// `morph_pairs`（见 `race_registry`），防双真源。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BodyPlan {
    pub id: BodyPlanId,
    pub display_name: String,
    pub is_humanoid: bool,
    pub parts: Vec<BodyPartDef>,
    pub hit_geometry: HitGeometry,
    pub equip_slots: Vec<EquipSlotV1>,
    #[serde(default)]
    pub meridian_profile: Option<MeridianProfile>,
    /// dandao `BodySlot`（变异挂载部位，`dandao/mutation.rs:97-103`）→ 本 plan 部位 id
    /// 的映射表（§7 声明的落点）。非 humanoid plan 可留空（`{}`）——变异体系目前只在
    /// 人形构型上生效。查询 API 见 `resolve::body_part_for_mutation_slot`。
    #[serde(default)]
    pub mutation_slot_mapping: HashMap<BodySlot, BodyPartId>,
}

/// 实体的「本体」种族标识——与 `Cultivation.race`（玩家身份 / gate 判定用，见
/// `resolve::resolve_body_plan` 优先级）正交存在，供 P4 `resolve_morph_pair` 取
/// from 端。P0 仅锁定组件形状 + `RaceRegistry::race_id_for_beast_kind` 派生查询机制；
/// 尚无生产 spawn 链路主动 insert 本组件（P5 起给 whale 换，见 plan §P0 races 段）。
#[derive(Debug, Clone, PartialEq, Eq, Component, Serialize, Deserialize)]
pub struct IntrinsicRace(pub RaceId);

impl BodyPlan {
    /// plan-race-system-v1 P0 review r2（BLOCKING-2 收口）—— 按部位 id 查询该部位的
    /// [`PartConsequence`]。伤残后果消费点（`combat::resolve` 的腿伤减速 / 头伤眩晕 /
    /// 臂伤脱手判定）用本方法直接按目标实体解析出的 plan 分发，取代此前"把命中部位反向
    /// 转换成 legacy `BodyPart` 再 match 8 个变体"的做法——**任意** `BodyPartId`（含
    /// 非人形构型的部位 id，如未来 whale 的 `tail_fin`）都能拿到正确答案，不再要求
    /// "必须能转换回 legacy enum" 这个人形专属前提。找不到该部位（未知 id，例如伤口写入
    /// 时 plan 已变更 / 数据损坏）返回 `None`——调用方必须显式处理，不得默认某个后果。
    pub fn consequence_for(&self, part_id: &BodyPartId) -> Option<&PartConsequence> {
        self.parts
            .iter()
            .find(|def| &def.id == part_id)
            .map(|def| &def.consequence)
    }

    /// 按 [`PartConsequence`] 谓词筛选出全部匹配部位的 **id**（不做 legacy `BodyPart`
    /// 转换——取代 [`super::legacy::legacy_body_parts_matching`] 在 `Wound.location`
    /// 已经是 `BodyPartId` 之后的用途：`combat::arm_wound` / `movement::leg_wound` 现在
    /// 需要拿部位 id 去过滤 `Wounds.entries`，而不是拿 legacy enum）。返回顺序 =
    /// `parts` 数组声明顺序，稳定可复现。
    pub fn parts_matching<'a>(
        &'a self,
        predicate: impl Fn(&PartConsequence) -> bool + 'a,
    ) -> impl Iterator<Item = &'a BodyPartId> + 'a {
        self.parts
            .iter()
            .filter(move |def| predicate(&def.consequence))
            .map(|def| &def.id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn body_plan_id_display_and_accessors_round_trip() {
        let id = BodyPlanId::new("humanoid");
        assert_eq!(id.as_str(), "humanoid");
        assert_eq!(id.to_string(), "humanoid");
        assert_eq!(BodyPlanId::from("humanoid"), id);
        assert_eq!(BodyPlanId::from("humanoid".to_string()), id);
    }

    #[test]
    fn body_part_id_display_and_accessors_round_trip() {
        let id = BodyPartId::new("head");
        assert_eq!(id.as_str(), "head");
        assert_eq!(id.to_string(), "head");
        assert_eq!(BodyPartId::from("head"), id);
    }

    #[test]
    fn race_id_display_and_accessors_round_trip() {
        let id = RaceId::new("human");
        assert_eq!(id.as_str(), "human");
        assert_eq!(id.to_string(), "human");
        assert_eq!(RaceId::from("human".to_string()), id);
    }

    // ─────────────────────────────────────────────────────────────────
    // RaceGate（const/&'static 形态）：三档 allows() 矩阵。
    // ─────────────────────────────────────────────────────────────────

    const WHALE_SPECIES: &[&str] = &["whale"];

    #[test]
    fn race_gate_any_allows_every_identity() {
        let gate = RaceGate::Any;
        assert!(gate.allows(&RaceId::new("human"), true));
        assert!(gate.allows(&RaceId::new("whale"), false));
    }

    #[test]
    fn race_gate_humanoid_gates_on_is_humanoid_only() {
        let gate = RaceGate::Humanoid;
        assert!(
            gate.allows(&RaceId::new("human"), true),
            "is_humanoid=true 必须放行，不看具体 race_id"
        );
        assert!(
            gate.allows(&RaceId::new("some_other_humanoid_race"), true),
            "Humanoid 档判据只看 is_humanoid，任何人形构型种族均放行——非种族名单硬编码"
        );
        assert!(
            !gate.allows(&RaceId::new("whale"), false),
            "is_humanoid=false 必须拒绝，即便 race_id 未知"
        );
    }

    #[test]
    fn race_gate_species_matches_exact_allowlist_only() {
        let gate = RaceGate::Species(WHALE_SPECIES);
        assert!(gate.allows(&RaceId::new("whale"), false));
        assert!(
            !gate.allows(&RaceId::new("human"), true),
            "Species 档不因 is_humanoid=true 放行——精确名单以外一律拒绝"
        );
        assert!(!gate.allows(&RaceId::new("beast_common"), false));
    }

    #[test]
    fn race_gate_default_is_any() {
        assert_eq!(RaceGate::default(), RaceGate::Any);
    }

    // ─────────────────────────────────────────────────────────────────
    // RaceGateOwned（serde owned 形态）：三变体正反 sample + 未知 kind fail-closed +
    // 空 / 重复 species pin。
    // ─────────────────────────────────────────────────────────────────

    #[test]
    fn race_gate_owned_serde_pin_any_variant() {
        let value = RaceGateOwned::Any;
        let json = serde_json::to_string(&value).expect("serialize");
        assert_eq!(json, r#"{"kind":"any"}"#, "Any 变体序列化形状漂移");
        let round_tripped: RaceGateOwned = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(round_tripped, value);
    }

    #[test]
    fn race_gate_owned_serde_pin_humanoid_variant() {
        let value = RaceGateOwned::Humanoid;
        let json = serde_json::to_string(&value).expect("serialize");
        assert_eq!(
            json, r#"{"kind":"humanoid"}"#,
            "Humanoid 变体序列化形状漂移"
        );
        let round_tripped: RaceGateOwned = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(round_tripped, value);
    }

    #[test]
    fn race_gate_owned_serde_pin_species_variant() {
        let value = RaceGateOwned::Species {
            species: vec![RaceId::new("whale")],
        };
        let json = serde_json::to_string(&value).expect("serialize");
        assert_eq!(
            json, r#"{"kind":"species","species":["whale"]}"#,
            "Species 变体序列化形状漂移"
        );
        let round_tripped: RaceGateOwned = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(round_tripped, value);
    }

    #[test]
    fn race_gate_owned_species_empty_list_round_trips() {
        let value = RaceGateOwned::Species { species: vec![] };
        let json = serde_json::to_string(&value).expect("serialize");
        assert_eq!(json, r#"{"kind":"species","species":[]}"#);
        let round_tripped: RaceGateOwned = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(
            round_tripped, value,
            "空 species 列表是合法状态（虽无实际效果，但不得反序列化失败/被吞掉）"
        );
    }

    #[test]
    fn race_gate_owned_species_duplicate_entries_preserved() {
        let json = r#"{"kind":"species","species":["whale","whale"]}"#;
        let value: RaceGateOwned = serde_json::from_str(json).expect("deserialize");
        match &value {
            RaceGateOwned::Species { species } => {
                assert_eq!(
                    species.len(),
                    2,
                    "重复 species 条目原样保留，不做去重（去重是校验层的事，不是 wire 层）"
                );
            }
            other => panic!("expected Species, got {other:?}"),
        }
        let re_encoded = serde_json::to_string(&value).expect("serialize");
        assert_eq!(re_encoded, json, "重复条目往返序列化不变形");
    }

    #[test]
    fn race_gate_owned_rejects_unknown_kind() {
        let err = serde_json::from_str::<RaceGateOwned>(r#"{"kind":"bogus"}"#)
            .expect_err("unknown kind must fail closed, not silently default to Any");
        assert!(
            err.to_string().contains("bogus") || err.to_string().contains("kind"),
            "错误信息应带上未知 kind 定位线索，实际: {err}"
        );
    }

    #[test]
    fn race_gate_owned_species_missing_species_field_rejected() {
        // species 变体缺 species 字段——serde 内部标签枚举的必填字段校验。
        let err = serde_json::from_str::<RaceGateOwned>(r#"{"kind":"species"}"#)
            .expect_err("species kind 缺 species 字段应反序列化失败");
        assert!(err.to_string().contains("species"));
    }

    #[test]
    fn race_gate_owned_default_is_any() {
        assert_eq!(RaceGateOwned::default(), RaceGateOwned::Any);
    }

    #[test]
    fn race_gate_owned_allows_matches_race_gate_semantics() {
        // 两种形态（const &'static vs owned）判定逻辑必须一致——防止一处改了 allows()
        // 语义、另一处漏改的镜像漂移。
        let whale = RaceId::new("whale");
        let human = RaceId::new("human");

        assert_eq!(
            RaceGate::Any.allows(&whale, false),
            RaceGateOwned::Any.allows(&whale, false)
        );
        assert_eq!(
            RaceGate::Humanoid.allows(&human, true),
            RaceGateOwned::Humanoid.allows(&human, true)
        );
        assert_eq!(
            RaceGate::Humanoid.allows(&whale, false),
            RaceGateOwned::Humanoid.allows(&whale, false)
        );
        let species_const = RaceGate::Species(WHALE_SPECIES);
        let species_owned = RaceGateOwned::Species {
            species: vec![RaceId::new("whale")],
        };
        assert_eq!(
            species_const.allows(&whale, false),
            species_owned.allows(&whale, false)
        );
        assert_eq!(
            species_const.allows(&human, true),
            species_owned.allows(&human, true)
        );
    }

    /// plan-race-system-v1 P3a（§P3 身份快照 bullet）—— 两个不同 `RaceId` 共享同一
    /// `BodyPlan`（即 `is_humanoid` 判定结果相同）的反例：Humanoid 档只看 `is_humanoid`
    /// 放行两者，Species 档必须精确匹配 `RaceId`，共享者中不在白名单里的那个照样拒绝——
    /// 证明 Species 档不会因为"底层构型相同"而被 Humanoid 档的宽松语义污染。
    #[test]
    fn two_race_ids_sharing_same_body_plan_diverge_under_species_gate() {
        // 模拟场景：human 与 human_variant 两个种族都挂在同一份 humanoid BodyPlan 下
        // （resolve_body_plan 返回同一个 &BodyPlan，is_humanoid=true 对两者都成立）。
        let human = RaceId::new("human");
        let human_variant = RaceId::new("human_variant");
        let shared_is_humanoid = true;

        // Humanoid 档：只看 is_humanoid，两个共享同一 BodyPlan 的种族都放行。
        let humanoid_gate = RaceGate::Humanoid;
        assert!(humanoid_gate.allows(&human, shared_is_humanoid));
        assert!(humanoid_gate.allows(&human_variant, shared_is_humanoid));

        // Species 档：即便两者 is_humanoid 相同（共享同一 BodyPlan），精确名单只认
        // "human"，human_variant 必须被拒绝——不因构型相同而放行。
        let species_gate = RaceGate::Species(&["human"]);
        assert!(species_gate.allows(&human, shared_is_humanoid));
        assert!(
            !species_gate.allows(&human_variant, shared_is_humanoid),
            "human_variant 与 human 共享同一 BodyPlan（is_humanoid 相同）也不应被 Species(\"human\") 放行"
        );

        // owned 形态同一断言（防两形态语义漂移）。
        let humanoid_owned = RaceGateOwned::Humanoid;
        assert!(humanoid_owned.allows(&human, shared_is_humanoid));
        assert!(humanoid_owned.allows(&human_variant, shared_is_humanoid));
        let species_owned = RaceGateOwned::Species {
            species: vec![RaceId::new("human")],
        };
        assert!(species_owned.allows(&human, shared_is_humanoid));
        assert!(!species_owned.allows(&human_variant, shared_is_humanoid));
    }

    #[test]
    fn part_consequence_serde_pin_every_variant() {
        let cases: [(PartConsequence, &str); 4] = [
            (PartConsequence::Locomotion, r#"{"kind":"locomotion"}"#),
            (PartConsequence::Sensory, r#"{"kind":"sensory"}"#),
            (
                PartConsequence::Manipulator { main_hand: true },
                r#"{"kind":"manipulator","main_hand":true}"#,
            ),
            (PartConsequence::Core, r#"{"kind":"core"}"#),
        ];
        for (value, expected_json) in cases {
            let serialized = serde_json::to_string(&value).expect("serialize");
            assert_eq!(
                serialized, expected_json,
                "PartConsequence {value:?} 序列化形状漂移"
            );
            let deserialized: PartConsequence =
                serde_json::from_str(expected_json).expect("deserialize");
            assert_eq!(deserialized, value, "PartConsequence 反序列化往返不一致");
        }
    }

    #[test]
    fn part_consequence_manipulator_main_hand_false_variant() {
        let value = PartConsequence::Manipulator { main_hand: false };
        let serialized = serde_json::to_string(&value).expect("serialize");
        assert_eq!(serialized, r#"{"kind":"manipulator","main_hand":false}"#);
    }

    #[test]
    fn part_consequence_rejects_unknown_kind() {
        let err = serde_json::from_str::<PartConsequence>(r#"{"kind":"unknown_kind"}"#)
            .expect_err("unknown kind must fail closed, not silently default");
        assert!(err.to_string().contains("unknown_kind") || err.to_string().contains("kind"));
    }

    #[test]
    fn height_band_assignment_referenced_part_ids_every_variant() {
        let single = HeightBandAssignment::Single {
            part: BodyPartId::new("head"),
        };
        assert_eq!(single.referenced_part_ids(), vec![&BodyPartId::new("head")]);

        let split_with_center = HeightBandAssignment::LateralSplitWithCenter {
            left: BodyPartId::new("arm_l"),
            right: BodyPartId::new("arm_r"),
            center: BodyPartId::new("chest"),
        };
        assert_eq!(
            split_with_center.referenced_part_ids(),
            vec![
                &BodyPartId::new("arm_l"),
                &BodyPartId::new("arm_r"),
                &BodyPartId::new("chest"),
            ]
        );

        let split = HeightBandAssignment::LateralSplit {
            left: BodyPartId::new("leg_l"),
            right: BodyPartId::new("leg_r"),
        };
        assert_eq!(
            split.referenced_part_ids(),
            vec![&BodyPartId::new("leg_l"), &BodyPartId::new("leg_r")]
        );
    }

    #[test]
    fn hit_geometry_height_bands_serde_round_trip() {
        let geometry = HitGeometry::HeightBands {
            aabb: StandingAabbSpec {
                half_width: 0.3,
                height: 1.8,
            },
            bands: vec![HeightBand {
                min_rel_y: 0.88,
                assignment: HeightBandAssignment::Single {
                    part: BodyPartId::new("head"),
                },
            }],
            lateral_threshold: 0.19,
        };
        let json = serde_json::to_value(&geometry).expect("serialize");
        assert_eq!(json["mode"], "height_bands");
        let round_tripped: HitGeometry = serde_json::from_value(json).expect("deserialize");
        assert_eq!(round_tripped, geometry);
    }

    #[test]
    fn hit_geometry_part_boxes_serde_round_trip() {
        let geometry = HitGeometry::PartBoxes {
            boxes: vec![PartBox {
                part_id: BodyPartId::new("skull"),
                offset: [0.0, 1.5, 0.0],
                half_extents: [0.3, 0.3, 0.3],
                priority: 10,
            }],
        };
        let json = serde_json::to_value(&geometry).expect("serialize");
        assert_eq!(json["mode"], "part_boxes");
        let round_tripped: HitGeometry = serde_json::from_value(json).expect("deserialize");
        assert_eq!(round_tripped, geometry);
    }

    #[test]
    fn meridian_profile_missing_field_defaults_to_none() {
        #[derive(Debug, Deserialize)]
        struct Wrapper {
            #[serde(default)]
            meridian_profile: Option<MeridianProfile>,
        }
        let missing: Wrapper =
            serde_json::from_str("{}").expect("missing field should deserialize");
        assert_eq!(missing.meridian_profile, None);
    }

    /// P1a 起 `MeridianProfile` 不再是空占位结构体——`channels`/`realm_requirements`
    /// 是必填字段，空对象 `{}` 不再是合法内容（P0 时期曾合法，见该测试此前版本）。
    #[test]
    fn meridian_profile_present_but_empty_object_is_now_rejected() {
        #[derive(Debug, Deserialize)]
        struct Wrapper {
            #[serde(default)]
            meridian_profile: Option<MeridianProfile>,
        }
        let err = serde_json::from_str::<Wrapper>(r#"{"meridian_profile":{}}"#)
            .expect_err("P1a 起空对象缺少必填 channels/realm_requirements，必须拒绝而非静默默认");
        let message = err.to_string();
        assert!(
            message.contains("channels") || message.contains("missing field"),
            "错误消息应指出缺失字段，got: {message}"
        );
    }

    fn one_channel_profile() -> MeridianProfile {
        MeridianProfile {
            channels: vec![ChannelDef {
                id: "lung".into(),
                family: MeridianFamily::Regular,
                body_part: Some(BodyPartId::new("arm_l")),
                roles: vec![],
            }],
            topology_edges: vec![],
            dugu_injection: vec![],
            realm_requirements: [RealmMeridianReq {
                total: 1,
                regular_min: 1,
                extraordinary_min: 0,
            }; 6],
        }
    }

    #[test]
    fn meridian_profile_present_with_full_content_is_some() {
        #[derive(Debug, Deserialize)]
        struct Wrapper {
            #[serde(default)]
            meridian_profile: Option<MeridianProfile>,
        }
        let profile = one_channel_profile();
        let json = serde_json::json!({ "meridian_profile": profile });
        let present: Wrapper =
            serde_json::from_value(json).expect("fully populated profile should deserialize");
        assert_eq!(present.meridian_profile, Some(profile));
    }

    #[test]
    fn meridian_family_serde_pin_both_variants() {
        assert_eq!(
            serde_json::to_string(&MeridianFamily::Regular).unwrap(),
            r#""regular""#
        );
        assert_eq!(
            serde_json::to_string(&MeridianFamily::Extraordinary).unwrap(),
            r#""extraordinary""#
        );
        assert_eq!(
            serde_json::from_str::<MeridianFamily>(r#""regular""#).unwrap(),
            MeridianFamily::Regular
        );
        assert_eq!(
            serde_json::from_str::<MeridianFamily>(r#""extraordinary""#).unwrap(),
            MeridianFamily::Extraordinary
        );
    }

    #[test]
    fn meridian_family_rejects_unknown_variant() {
        assert!(serde_json::from_str::<MeridianFamily>(r#""mystic""#).is_err());
    }

    #[test]
    fn channel_role_serde_pin() {
        assert_eq!(
            serde_json::to_string(&ChannelRole::FormAnchor).unwrap(),
            r#""form_anchor""#
        );
        assert_eq!(
            serde_json::from_str::<ChannelRole>(r#""form_anchor""#).unwrap(),
            ChannelRole::FormAnchor
        );
    }

    #[test]
    fn channel_def_body_part_defaults_to_none_when_absent() {
        let json = serde_json::json!({
            "id": "chong",
            "family": "extraordinary",
        });
        let decoded: ChannelDef = serde_json::from_value(json).expect("deserialize");
        assert_eq!(decoded.body_part, None);
        assert!(decoded.roles.is_empty());
    }

    #[test]
    fn channel_def_body_part_some_round_trips() {
        let def = ChannelDef {
            id: "ren".into(),
            family: MeridianFamily::Extraordinary,
            body_part: Some(BodyPartId::new("chest")),
            roles: vec![ChannelRole::FormAnchor],
        };
        let json = serde_json::to_value(&def).expect("serialize");
        let round_tripped: ChannelDef = serde_json::from_value(json).expect("deserialize");
        assert_eq!(round_tripped, def);
    }

    #[test]
    fn topology_edge_serde_round_trip() {
        let edge = TopologyEdge {
            from: "lung".into(),
            to: "large_intestine".into(),
        };
        let json = serde_json::to_value(&edge).expect("serialize");
        let round_tripped: TopologyEdge = serde_json::from_value(json).expect("deserialize");
        assert_eq!(round_tripped, edge);
    }

    #[test]
    fn realm_meridian_req_default_is_all_zero() {
        let req = RealmMeridianReq::default();
        assert_eq!(req.total, 0);
        assert_eq!(req.regular_min, 0);
        assert_eq!(req.extraordinary_min, 0);
    }

    #[test]
    fn realm_meridian_req_sub_quota_fields_default_when_absent() {
        let json = serde_json::json!({ "total": 6 });
        let decoded: RealmMeridianReq = serde_json::from_value(json).expect("deserialize");
        assert_eq!(decoded.total, 6);
        assert_eq!(decoded.regular_min, 0);
        assert_eq!(decoded.extraordinary_min, 0);
    }

    #[test]
    fn meridian_profile_channels_topology_realm_requirements_round_trip() {
        let profile = one_channel_profile();
        let json = serde_json::to_value(&profile).expect("serialize");
        let round_tripped: MeridianProfile = serde_json::from_value(json).expect("deserialize");
        assert_eq!(round_tripped, profile);
    }

    #[test]
    fn intrinsic_race_component_equality() {
        let a = IntrinsicRace(RaceId::new("human"));
        let b = IntrinsicRace(RaceId::new("human"));
        let c = IntrinsicRace(RaceId::new("beast_common"));
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    // ───────────────────── BodyPlan::consequence_for / parts_matching ─────────────────────

    fn plan_with_four_consequence_kinds() -> BodyPlan {
        BodyPlan {
            id: "fixture_consequences".into(),
            display_name: "四类后果测试构型".to_string(),
            is_humanoid: false,
            parts: vec![
                BodyPartDef {
                    id: "tail_fin".into(),
                    damage_mul: 0.6,
                    contam_mul: 0.7,
                    bleed_mul: 1.0,
                    consequence: PartConsequence::Locomotion,
                },
                BodyPartDef {
                    id: "skull".into(),
                    damage_mul: 2.0,
                    contam_mul: 1.5,
                    bleed_mul: 1.5,
                    consequence: PartConsequence::Sensory,
                },
                BodyPartDef {
                    id: "left_pincer".into(),
                    damage_mul: 0.7,
                    contam_mul: 0.8,
                    bleed_mul: 0.8,
                    consequence: PartConsequence::Manipulator { main_hand: true },
                },
                BodyPartDef {
                    id: "right_pincer".into(),
                    damage_mul: 0.7,
                    contam_mul: 0.8,
                    bleed_mul: 0.8,
                    consequence: PartConsequence::Manipulator { main_hand: false },
                },
                BodyPartDef {
                    id: "carapace".into(),
                    damage_mul: 1.0,
                    contam_mul: 1.0,
                    bleed_mul: 1.0,
                    consequence: PartConsequence::Core,
                },
            ],
            hit_geometry: HitGeometry::PartBoxes { boxes: vec![] },
            equip_slots: vec![],
            meridian_profile: None,
            mutation_slot_mapping: HashMap::new(),
        }
    }

    #[test]
    fn consequence_for_finds_every_declared_part_by_id() {
        let plan = plan_with_four_consequence_kinds();
        assert_eq!(
            plan.consequence_for(&BodyPartId::new("tail_fin")),
            Some(&PartConsequence::Locomotion)
        );
        assert_eq!(
            plan.consequence_for(&BodyPartId::new("skull")),
            Some(&PartConsequence::Sensory)
        );
        assert_eq!(
            plan.consequence_for(&BodyPartId::new("left_pincer")),
            Some(&PartConsequence::Manipulator { main_hand: true })
        );
        assert_eq!(
            plan.consequence_for(&BodyPartId::new("right_pincer")),
            Some(&PartConsequence::Manipulator { main_hand: false })
        );
        assert_eq!(
            plan.consequence_for(&BodyPartId::new("carapace")),
            Some(&PartConsequence::Core)
        );
    }

    #[test]
    fn consequence_for_returns_none_for_unknown_part_id() {
        let plan = plan_with_four_consequence_kinds();
        assert_eq!(
            plan.consequence_for(&BodyPartId::new("does_not_exist")),
            None,
            "未知 part id 必须显式返回 None，调用方自行决定无后果策略，而不是猜一个默认后果"
        );
    }

    #[test]
    fn consequence_for_returns_none_on_empty_parts() {
        let mut plan = plan_with_four_consequence_kinds();
        plan.parts.clear();
        assert_eq!(plan.consequence_for(&BodyPartId::new("tail_fin")), None);
    }

    #[test]
    fn parts_matching_locomotion_finds_only_tail_fin() {
        let plan = plan_with_four_consequence_kinds();
        let found: Vec<&BodyPartId> = plan
            .parts_matching(|c| matches!(c, PartConsequence::Locomotion))
            .collect();
        assert_eq!(found, vec![&BodyPartId::new("tail_fin")]);
    }

    #[test]
    fn parts_matching_manipulator_main_hand_true_finds_only_left_pincer() {
        let plan = plan_with_four_consequence_kinds();
        let found: Vec<&BodyPartId> = plan
            .parts_matching(|c| matches!(c, PartConsequence::Manipulator { main_hand: true }))
            .collect();
        assert_eq!(found, vec![&BodyPartId::new("left_pincer")]);
    }

    #[test]
    fn parts_matching_manipulator_main_hand_false_finds_only_right_pincer() {
        let plan = plan_with_four_consequence_kinds();
        let found: Vec<&BodyPartId> = plan
            .parts_matching(|c| matches!(c, PartConsequence::Manipulator { main_hand: false }))
            .collect();
        assert_eq!(found, vec![&BodyPartId::new("right_pincer")]);
    }

    #[test]
    fn parts_matching_sensory_finds_only_skull() {
        let plan = plan_with_four_consequence_kinds();
        let found: Vec<&BodyPartId> = plan
            .parts_matching(|c| matches!(c, PartConsequence::Sensory))
            .collect();
        assert_eq!(found, vec![&BodyPartId::new("skull")]);
    }

    #[test]
    fn parts_matching_core_finds_only_carapace() {
        let plan = plan_with_four_consequence_kinds();
        let found: Vec<&BodyPartId> = plan
            .parts_matching(|c| matches!(c, PartConsequence::Core))
            .collect();
        assert_eq!(found, vec![&BodyPartId::new("carapace")]);
    }

    #[test]
    fn parts_matching_no_match_yields_empty_iterator() {
        let plan = plan_with_four_consequence_kinds();
        let found: Vec<&BodyPartId> = plan.parts_matching(|_| false).collect();
        assert!(found.is_empty());
    }

    #[test]
    fn parts_matching_preserves_declaration_order() {
        // humanoid.json 声明双腿为 leg_l 后 leg_r（parts 数组序）；parts_matching 必须
        // 保持这个声明顺序，不做隐式排序——`arm_wound`/`leg_wound` 的调用方依赖这一点
        // 做"取最重伤"归约时结果与遍历顺序无关，但顺序本身仍需可预期以便调试。
        let plan = crate::body_plan::humanoid_plan_static();
        let legs: Vec<&BodyPartId> = plan
            .parts_matching(|c| matches!(c, PartConsequence::Locomotion))
            .collect();
        assert_eq!(
            legs,
            vec![&BodyPartId::new("leg_l"), &BodyPartId::new("leg_r")],
            "humanoid.json 的 parts 数组顺序必须是 leg_l 在前"
        );
    }
}
