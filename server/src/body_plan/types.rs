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
