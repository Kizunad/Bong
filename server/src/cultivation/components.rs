//! P1 cultivation components.
//!
//! Minimal slice: Cultivation / MeridianSystem / QiColor / Karma, plus the
//! enums they depend on. Mutations, breakthrough logic, forging, and color
//! evolution live in later slices; this file only defines state shape and
//! trivially pure helpers.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use valence::prelude::{bevy_ecs, Component};

use crate::body_plan::RaceId;

/// 修为境界 — see plan §1.1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Realm {
    Awaken,   // 醒灵
    Induce,   // 引气
    Condense, // 凝脉
    Solidify, // 固元
    Spirit,   // 通灵
    Void,     // 化虚
}

impl Realm {
    pub fn previous(self) -> Option<Self> {
        match self {
            Realm::Awaken => None,
            Realm::Induce => Some(Realm::Awaken),
            Realm::Condense => Some(Realm::Induce),
            Realm::Solidify => Some(Realm::Condense),
            Realm::Spirit => Some(Realm::Solidify),
            Realm::Void => Some(Realm::Spirit),
        }
    }

    /// 此境界需要已打通的经脉数量（含正经 + 奇经，参考 plan §3.1）。
    ///
    /// plan-race-system-v1 P1a：数值来源已从本文件硬编码 match 迁移至
    /// `body_plan::MeridianProfile.realm_requirements`（`humanoid.json`，§8.1 #8
    /// "公式即数据"决议）——本函数读取 humanoid 曲线，数值 1/3/6/12/16/20 bit-for-bit
    /// 不变。当前全部已注册种族均映射到 humanoid body plan（`races.json`），尚无
    /// 需要按 `Cultivation.race` 差异化取值的真实调用场景；等 P5 引入非 humanoid
    /// 战斗构型时，需要真正"目标实体"上下文的调用点（如 `breakthrough_precondition_error`）
    /// 再各自改走 `resolve_body_plan_for_target` 解析出的 profile，本函数签名保持
    /// 零参数不变（大量既有调用点无 entity 上下文，见该函数文档）。
    pub fn required_meridians(self) -> usize {
        let profile = crate::body_plan::humanoid_plan_static()
            .meridian_profile
            .as_ref()
            .expect(
                "humanoid body plan must declare meridian_profile from plan-race-system-v1 P1 \
                 onward — validate_body_plan should have rejected a humanoid plan missing it",
            );
        profile.realm_requirements[self.rank() as usize - 1].total as usize
    }

    /// plan-agent-ui-data-v1 P0 — 境界 1-indexed rank（与 `realm_gate` 比较）。
    ///
    /// 醒灵=1, 引气=2, 凝脉=3, 固元=4, 通灵=5, 化虚=6。
    /// realm_gate=0 表示不门控，判断时 player_rank >= realm_gate（0 恒真）。
    pub fn rank(self) -> u8 {
        match self {
            Realm::Awaken => 1,
            Realm::Induce => 2,
            Realm::Condense => 3,
            Realm::Solidify => 4,
            Realm::Spirit => 5,
            Realm::Void => 6,
        }
    }
}

/// 经脉 channel id（string，plan-race-system-v1 P1）——`MeridianSystem`/`Meridian.id`
/// 的系统真源，`body_plan::MeridianProfile.channels[].id` 同型。取代 `MeridianId`
/// 20 变体闭合枚举作为运行时存储 key：非人形构型（如 P5 飞鲸）的经脉不在这 20 个
/// TCM 名字之列，必须能用任意 snake_case 字符串表达。
///
/// `MeridianId` 仍保留（见其文档），二者互转走 [`MeridianId::channel_id`] /
/// [`MeridianChannelId::to_meridian_id`]；`MeridianSystem::get`/`get_mut` 接受
/// `impl Into<MeridianChannelId>`，故现存传 `MeridianId::X` 字面量的调用点无需改写。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MeridianChannelId(pub String);

impl MeridianChannelId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// 反向查询——仅当本 id 恰好是 humanoid 20 条经脉之一的规范 snake_case 名时返回
    /// `Some`。供仍以 `MeridianId` 为公共接口的既有消费点（`life_record::BiographyEntry`
    /// / `combat::baomai_v4` 私表 / `MeridianSeveredEvent` 等）在读取 `Meridian.id` 时
    /// 转回旧类型；非 humanoid 构型（P5 飞鲸等）的 channel id 会合法地返回 `None`——
    /// 调用方必须显式处理，不允许静默 fallback 到某个哨兵 id（那会伪造数据）。
    pub fn to_meridian_id(&self) -> Option<MeridianId> {
        MeridianId::from_channel_str(self.0.as_str())
    }
}

impl std::fmt::Display for MeridianChannelId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&str> for MeridianChannelId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<String> for MeridianChannelId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<MeridianId> for MeridianChannelId {
    fn from(id: MeridianId) -> Self {
        id.channel_id()
    }
}

/// 20 条经脉（12 正经 + 8 奇经）。
///
/// **migration-only（plan-race-system-v1 P1a）**：`MeridianSystem`/`Meridian.id` 的
/// 系统真源已换轨为 [`MeridianChannelId`]（string，见其文档）。本枚举 + `REGULAR` /
/// `EXTRAORDINARY` / `ALL` 常量保留，仅用于：① 旧存档迁移（`persistence` bundle 里
/// PascalCase 枚举名 → 新 snake_case channel id 的一次性重映射）② 尚未迁移到
/// channel-id 原生表达的既有公共接口的桥接类型（`MeridianTopology` / 各流派技能
/// 依赖表 / `life_record::BiographyEntry` / `combat::baomai_v4` 私表等——它们仍以
/// `MeridianId` 作为参数/字段类型，本枚举经 [`channel_id`](MeridianId::channel_id) /
/// [`MeridianChannelId::to_meridian_id`] 与新类型互转）。**运行时新增引用请优先走
/// `MeridianChannelId`**——直接依赖本枚举意味着无法表达非 humanoid 构型的经脉
/// （P5 飞鲸等），只在对接上述尚未迁移的旧接口时才应继续使用。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MeridianId {
    // 12 正经
    Lung,
    LargeIntestine,
    Stomach,
    Spleen,
    Heart,
    SmallIntestine,
    Bladder,
    Kidney,
    Pericardium,
    TripleEnergizer,
    Gallbladder,
    Liver,
    // 8 奇经
    Ren,
    Du,
    Chong,
    Dai,
    YinQiao,
    YangQiao,
    YinWei,
    YangWei,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeridianFamily {
    Regular,
    Extraordinary,
}

impl MeridianId {
    pub const REGULAR: [MeridianId; 12] = [
        MeridianId::Lung,
        MeridianId::LargeIntestine,
        MeridianId::Stomach,
        MeridianId::Spleen,
        MeridianId::Heart,
        MeridianId::SmallIntestine,
        MeridianId::Bladder,
        MeridianId::Kidney,
        MeridianId::Pericardium,
        MeridianId::TripleEnergizer,
        MeridianId::Gallbladder,
        MeridianId::Liver,
    ];

    pub const EXTRAORDINARY: [MeridianId; 8] = [
        MeridianId::Ren,
        MeridianId::Du,
        MeridianId::Chong,
        MeridianId::Dai,
        MeridianId::YinQiao,
        MeridianId::YangQiao,
        MeridianId::YinWei,
        MeridianId::YangWei,
    ];

    pub const ALL: [MeridianId; 20] = [
        MeridianId::Lung,
        MeridianId::LargeIntestine,
        MeridianId::Stomach,
        MeridianId::Spleen,
        MeridianId::Heart,
        MeridianId::SmallIntestine,
        MeridianId::Bladder,
        MeridianId::Kidney,
        MeridianId::Pericardium,
        MeridianId::TripleEnergizer,
        MeridianId::Gallbladder,
        MeridianId::Liver,
        MeridianId::Ren,
        MeridianId::Du,
        MeridianId::Chong,
        MeridianId::Dai,
        MeridianId::YinQiao,
        MeridianId::YangQiao,
        MeridianId::YinWei,
        MeridianId::YangWei,
    ];

    pub fn family(self) -> MeridianFamily {
        if Self::REGULAR.contains(&self) {
            MeridianFamily::Regular
        } else {
            MeridianFamily::Extraordinary
        }
    }

    /// 规范 snake_case channel id（`body_plan::registry::humanoid_plan_static()` 的
    /// `humanoid.json meridian_profile.channels[].id` 必须与本表逐条 bit-for-bit 一致
    /// ——见 `body_plan` 侧对拍测试）。
    pub fn channel_id(self) -> MeridianChannelId {
        MeridianChannelId::new(match self {
            MeridianId::Lung => "lung",
            MeridianId::LargeIntestine => "large_intestine",
            MeridianId::Stomach => "stomach",
            MeridianId::Spleen => "spleen",
            MeridianId::Heart => "heart",
            MeridianId::SmallIntestine => "small_intestine",
            MeridianId::Bladder => "bladder",
            MeridianId::Kidney => "kidney",
            MeridianId::Pericardium => "pericardium",
            MeridianId::TripleEnergizer => "triple_energizer",
            MeridianId::Gallbladder => "gallbladder",
            MeridianId::Liver => "liver",
            MeridianId::Ren => "ren",
            MeridianId::Du => "du",
            MeridianId::Chong => "chong",
            MeridianId::Dai => "dai",
            MeridianId::YinQiao => "yin_qiao",
            MeridianId::YangQiao => "yang_qiao",
            MeridianId::YinWei => "yin_wei",
            MeridianId::YangWei => "yang_wei",
        })
    }

    /// [`channel_id`](Self::channel_id) 的反函数，仅接受规范 snake_case 字符串——不接受
    /// 旧版 PascalCase 变体名（那是 `Debug`/`serde` 派生的另一种表示，迁移函数按需自行
    /// 用 `serde_json` 解码，不走本函数）。
    fn from_channel_str(s: &str) -> Option<Self> {
        Some(match s {
            "lung" => MeridianId::Lung,
            "large_intestine" => MeridianId::LargeIntestine,
            "stomach" => MeridianId::Stomach,
            "spleen" => MeridianId::Spleen,
            "heart" => MeridianId::Heart,
            "small_intestine" => MeridianId::SmallIntestine,
            "bladder" => MeridianId::Bladder,
            "kidney" => MeridianId::Kidney,
            "pericardium" => MeridianId::Pericardium,
            "triple_energizer" => MeridianId::TripleEnergizer,
            "gallbladder" => MeridianId::Gallbladder,
            "liver" => MeridianId::Liver,
            "ren" => MeridianId::Ren,
            "du" => MeridianId::Du,
            "chong" => MeridianId::Chong,
            "dai" => MeridianId::Dai,
            "yin_qiao" => MeridianId::YinQiao,
            "yang_qiao" => MeridianId::YangQiao,
            "yin_wei" => MeridianId::YinWei,
            "yang_wei" => MeridianId::YangWei,
            _ => return None,
        })
    }
}

/// 单条经脉。`flow_rate` / `flow_capacity` 是相互独立的锻造轴
/// （plan §3.3）。
///
/// `id` 类型是 plan-race-system-v1 P1a 的核心换轨点：从闭合枚举 [`MeridianId`] 改为
/// string [`MeridianChannelId`]，使非 humanoid 构型（P5 飞鲸等）的经脉可以拥有 TCM
/// 20 名之外的 id。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Meridian {
    pub id: MeridianChannelId,
    pub opened: bool,
    pub open_progress: f64, // 0.0..=1.0, 未打通时累积
    pub flow_rate: f64,
    pub flow_capacity: f64,
    pub rate_tier: u8,
    pub capacity_tier: u8,
    pub throughput_current: f64,
    pub integrity: f64, // 0.0..=1.0
    pub cracks: Vec<MeridianCrack>,
    pub opened_at: u64, // tick 时戳，LIFO 排序用
}

/// 经脉裂痕（plan §1.1）。严重度 0..=1。`healing_progress` 达到 severity
/// 时移除。成因区分过载 / 被攻击 / 走火 / 淬炼失败。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MeridianCrack {
    pub severity: f64,
    pub healing_progress: f64,
    pub cause: CrackCause,
    pub created_at: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CrackCause {
    Overload,
    Attack,
    Backfire,        // 走火入魔
    ForgeFailure,    // 淬炼失败
    VoluntarySever,  // plan-meridian-severed-v1 §4 #1：zhenmai ⑤ 主动绝脉
    TribulationFail, // plan-meridian-severed-v1 §4 #5：渡劫失败爆脉
    DuguDistortion,  // plan-meridian-severed-v1 §4 #6：dugu 阴诡色形貌异化侵蚀
}

impl Meridian {
    pub fn new(id: MeridianChannelId) -> Self {
        Self {
            id,
            opened: false,
            open_progress: 0.0,
            flow_rate: 1.0,
            flow_capacity: 10.0,
            rate_tier: 0,
            capacity_tier: 0,
            throughput_current: 0.0,
            integrity: 1.0,
            cracks: Vec::new(),
            opened_at: 0,
        }
    }

    /// 供仍以 [`MeridianId`] 为公共接口的既有调用点（测试 / migration）直接构造。
    pub fn from_meridian_id(id: MeridianId) -> Self {
        Self::new(id.channel_id())
    }

    /// plan §3.3.2 渐进非线性 flow_rate 曲线。
    pub fn rate_for_tier(tier: u8) -> f64 {
        const CURVE: [f64; 11] = [1.0, 2.0, 3.0, 5.0, 8.0, 12.0, 17.0, 23.0, 30.0, 40.0, 55.0];
        CURVE[(tier as usize).min(10)]
    }

    /// plan §3.3.2 flow_capacity 曲线（以 10 为打通基准，tier 0 即基础容量）。
    pub fn capacity_for_tier(tier: u8) -> f64 {
        const CURVE: [f64; 11] = [
            10.0, 20.0, 30.0, 40.0, 60.0, 90.0, 130.0, 180.0, 250.0, 350.0, 500.0,
        ];
        CURVE[(tier as usize).min(10)]
    }
}

/// 异种真元污染源（plan §1.1）。由战斗 plan 写入，本 plan 负责排异演化。
///
/// `meridian_id` 类型是 plan-race-system-v1 P6b review BLOCKER 收口的核心换轨点：
/// 从闭合枚举 `Option<MeridianId>` 改为 `Option<MeridianChannelId>`（string），使非
/// humanoid 构型（如 P5 飞鲸的 `tail_core`）声明的 dugu 注入映射能真实挂靠到自己的
/// channel 并被 `contamination_tick`/`resolve_crack_target` 消费——换轨前的实现在
/// `combat::resolve` 构造处把已经拿到的通用 channel 又经 `to_meridian_id()` 压回
/// legacy 枚举，非人形专属 channel 必然 `None`，专属 channel 实际不可消费（见
/// `combat::resolve::dugu_contam_meridian_routing_tests` 换轨前后对比）。
#[derive(Debug, Clone, Serialize)]
pub struct ContamSource {
    pub amount: f64,
    pub color: ColorKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meridian_id: Option<MeridianChannelId>,
    #[serde(default)]
    pub attacker_id: Option<String>,
    pub introduced_at: u64,
}

impl<'de> Deserialize<'de> for ContamSource {
    /// 手写 `Deserialize`（而非 derive）只为 `meridian_id` 一个字段做**存档兼容**：
    /// P1a 之前写入的持久化 `contamination` bundle 里，`meridian_id` 是 `MeridianId`
    /// 的 serde 派生表示——PascalCase 枚举变体名字符串（如 `"Lung"`）。字段类型换轨
    /// 到 `MeridianChannelId`（transparent 任意字符串）后，若直接对旧 JSON 做透传式
    /// 反序列化，`"Lung"` 会被原样存成 channel id `"Lung"`（大小写不匹配新
    /// `humanoid.json` 声明的 snake_case `"lung"`），导致老存档的污染裂痕路由悄悄
    /// 失效而不是报错——这是数据内容漂移，不是硬崩溃，比 `legacy_meridian_bundle`
    /// 的显式版本号迁移更容易被漏测。这里改为：先尝试按 legacy `MeridianId`（严格
    /// PascalCase 变体名）解码，成功则经 `channel_id()` 归一化到新 snake_case；失败
    /// 再按当前形态（任意字符串，含非 humanoid channel 与已经是 snake_case 的新
    /// 存档）解码。
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct RawContamSource {
            amount: f64,
            color: ColorKind,
            #[serde(default)]
            meridian_id: Option<serde_json::Value>,
            #[serde(default)]
            attacker_id: Option<String>,
            introduced_at: u64,
        }

        let raw = RawContamSource::deserialize(deserializer)?;
        let meridian_id = match raw.meridian_id {
            None => None,
            Some(value) => {
                if let Ok(legacy) = serde_json::from_value::<MeridianId>(value.clone()) {
                    Some(legacy.channel_id())
                } else {
                    Some(
                        serde_json::from_value::<MeridianChannelId>(value)
                            .map_err(serde::de::Error::custom)?,
                    )
                }
            }
        };
        Ok(ContamSource {
            amount: raw.amount,
            color: raw.color,
            meridian_id,
            attacker_id: raw.attacker_id,
            introduced_at: raw.introduced_at,
        })
    }
}

/// 污染 Component。`entries` 为空 = 纯净。
#[derive(Debug, Clone, Default, Component, Serialize, Deserialize)]
pub struct Contamination {
    pub entries: Vec<ContamSource>,
}

/// 玩家经脉系统（plan-race-system-v1 P1a——从 `[Meridian;12]+[Meridian;8]` 定长数组
/// 改造为 `Vec<Meridian>`，解除"恰好 20 条 TCM 经脉"的硬编码假设，容纳按
/// `body_plan::MeridianProfile` 声明的任意条数/构型（P5 飞鲸等）。字段名 `regular` /
/// `extraordinary` 保留（沿用 `MeridianFamily` 二分），但类型从定长数组换成
/// `Vec`——绝大多数既有消费点（`.iter()`/`.iter_mut()`/`[idx]` 索引）语法不变，
/// 只是不再假设固定 12/8 条数。**查找按 `MeridianChannelId` 字符串键**（`get`/
/// `get_mut` 均已改为 `impl Into<MeridianChannelId>` 参数，见下方实现）。
#[derive(Debug, Clone, Component, Serialize, Deserialize, PartialEq)]
pub struct MeridianSystem {
    pub regular: Vec<Meridian>,
    pub extraordinary: Vec<Meridian>,
}

impl Default for MeridianSystem {
    /// humanoid 20 经曲线的唯一入口——数据源已从本文件的 Rust match 表迁移进
    /// `server/assets/body_plans/plans/humanoid.json` 的 `meridian_profile.channels`
    /// （plan §P1）。`Default` 不代表 humanoid 是特权路径，只是"迄今唯一存在的构型"
    /// 的便捷构造；未来构型走 [`MeridianSystem::for_profile`] 直接传自己的 profile。
    fn default() -> Self {
        let profile = crate::body_plan::humanoid_plan_static()
            .meridian_profile
            .as_ref()
            .expect(
                "humanoid body plan must declare meridian_profile from plan-race-system-v1 P1 \
                 onward — validate_body_plan should have rejected a humanoid plan missing it",
            );
        Self::for_profile(profile)
    }
}

impl MeridianSystem {
    /// 从任意 [`crate::body_plan::MeridianProfile`] 构造——`channels` 按声明顺序分桶进
    /// `regular`/`extraordinary`（顺序即 profile 声明顺序，不重排）。
    pub fn for_profile(profile: &crate::body_plan::MeridianProfile) -> Self {
        let mut regular = Vec::new();
        let mut extraordinary = Vec::new();
        for channel in &profile.channels {
            let meridian = Meridian::new(channel.id.clone());
            match channel.family {
                crate::body_plan::MeridianFamily::Regular => regular.push(meridian),
                crate::body_plan::MeridianFamily::Extraordinary => extraordinary.push(meridian),
            }
        }
        Self {
            regular,
            extraordinary,
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &Meridian> {
        self.regular.iter().chain(self.extraordinary.iter())
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Meridian> {
        self.regular.iter_mut().chain(self.extraordinary.iter_mut())
    }

    pub fn opened_count(&self) -> usize {
        self.iter().filter(|m| m.opened).count()
    }

    pub fn regular_opened_count(&self) -> usize {
        self.regular.iter().filter(|m| m.opened).count()
    }

    pub fn extraordinary_opened_count(&self) -> usize {
        self.extraordinary.iter().filter(|m| m.opened).count()
    }

    pub fn sum_capacity(&self) -> f64 {
        self.iter()
            .filter(|m| m.opened)
            .map(|m| m.flow_capacity)
            .sum()
    }

    pub fn sum_rate(&self) -> f64 {
        self.iter().filter(|m| m.opened).map(|m| m.flow_rate).sum()
    }

    /// 按 channel id 查找（接受 `MeridianChannelId` 或 legacy `MeridianId`——后者经
    /// `impl Into<MeridianChannelId>` 自动转换，既有传 `MeridianId::X` 字面量的调用点
    /// 无需改写）。未知 channel id 是编程错误（entity 的 profile 不该被查询它不拥有的
    /// 经脉）——panic 而非静默返回哨兵，防止调用方在错误分支里悄悄读到错误经脉。
    pub fn get(&self, id: impl Into<MeridianChannelId>) -> &Meridian {
        let id = id.into();
        self.regular
            .iter()
            .chain(self.extraordinary.iter())
            .find(|m| m.id == id)
            .unwrap_or_else(|| {
                panic!(
                    "[bong][cultivation] MeridianSystem::get: unknown channel id {id} — entity's \
                     meridian profile does not declare this channel"
                )
            })
    }

    pub fn get_mut(&mut self, id: impl Into<MeridianChannelId>) -> &mut Meridian {
        let id = id.into();
        self.regular
            .iter_mut()
            .chain(self.extraordinary.iter_mut())
            .find(|m| m.id == id)
            .unwrap_or_else(|| {
                panic!(
                    "[bong][cultivation] MeridianSystem::get_mut: unknown channel id {id} — \
                     entity's meridian profile does not declare this channel"
                )
            })
    }

    /// 非 panic 归属校验——plan-race-system-v1 P1 对抗审查 B1：任意 C2S 消费点
    /// （forge_request 等）把 wire 上的任意字符串 channel id 直送 `get`/`get_mut`
    /// 之前，必须先在消费边界用本方法确认该 id 属于该实体的经脉构型；未知 id
    /// （含旧 PascalCase `MeridianId` 字面量、伪造串）应安全拒绝，不得触发
    /// `get`/`get_mut` 的 panic 分支。
    pub fn contains(&self, id: impl Into<MeridianChannelId>) -> bool {
        let id = id.into();
        self.regular
            .iter()
            .chain(self.extraordinary.iter())
            .any(|m| m.id == id)
    }
}

/// 10 种真元色（plan §1.1 / worldview §六）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ColorKind {
    Sharp,     // 锐
    Heavy,     // 厚
    Mellow,    // 醇
    Solid,     // 实
    Light,     // 轻
    Intricate, // 巧
    Gentle,    // 柔
    Insidious, // 阴
    Violent,   // 烈
    Turbid,    // 浊
}

#[derive(Debug, Clone, Component, Serialize, Deserialize)]
pub struct QiColor {
    pub main: ColorKind,
    pub secondary: Option<ColorKind>,
    pub is_chaotic: bool,
    pub is_hunyuan: bool,
    #[serde(default)]
    pub permanent_lock_mask: HashSet<ColorKind>,
}

impl Default for QiColor {
    fn default() -> Self {
        Self {
            main: ColorKind::Mellow,
            secondary: None,
            is_chaotic: false,
            is_hunyuan: false,
            permanent_lock_mask: HashSet::new(),
        }
    }
}

impl QiColor {
    /// 永久锁定某色。
    ///
    /// **混元保护（plan-color-v1 P2）**：若 `is_hunyuan=true`，忽略写入——
    /// 混元色的本质是「五色均衡、无主无从」，不允许任何色被永久锁定。
    pub fn lock_permanent(&mut self, color: ColorKind) {
        if self.is_hunyuan {
            // 混元状态不允许永久锁定任何色（§六.2 决议）
            return;
        }
        self.permanent_lock_mask.insert(color);
    }

    pub fn is_permanently_locked(&self, color: ColorKind) -> bool {
        self.permanent_lock_mask.contains(&color)
    }
}

/// 修为主组件。`qi_max_frozen` 用于 QiZeroDecay 窗口期（plan §2）。
#[derive(Debug, Clone, Component, Serialize, Deserialize, PartialEq)]
pub struct Cultivation {
    pub realm: Realm,
    pub qi_current: f64,
    pub qi_max: f64,
    pub qi_max_frozen: Option<f64>,
    pub last_qi_zero_at: Option<u64>, // tick 计数
    pub pending_material_bonus: f64,
    pub composure: f64, // 0.0..=1.0
    pub composure_recover_rate: f64,
    /// plan-race-system-v1 P0 — 种族标识，`body_plan::resolve_body_plan` 的玩家身份
    /// 权威真源（未知 id 拒绝解析，见该函数文档）。`#[serde(default)]` 让旧存档
    /// （`cultivation_json` bundle 缺该字段）反序列化时自动落 `"human"`，
    /// persistence 层（`persist_player_cultivation_bundle` 等）零改动即可透传。
    #[serde(default = "default_race_id")]
    pub race: RaceId,
}

fn default_race_id() -> RaceId {
    RaceId::new(crate::body_plan::HUMAN_RACE_ID)
}

impl Default for Cultivation {
    fn default() -> Self {
        Self {
            realm: Realm::Awaken,
            qi_current: 0.0,
            qi_max: 10.0,
            qi_max_frozen: None,
            last_qi_zero_at: None,
            pending_material_bonus: 0.0,
            composure: 1.0,
            composure_recover_rate: 0.001,
            race: default_race_id(),
        }
    }
}

pub fn recover_current_qi(cultivation: &mut Cultivation, amount: f64) -> f64 {
    let amount = if amount.is_finite() {
        amount.max(0.0)
    } else {
        0.0
    };
    let effective_qi_max = (cultivation.qi_max - cultivation.qi_max_frozen.unwrap_or(0.0)).max(0.0);
    let before = cultivation.qi_current;
    cultivation.qi_current = (cultivation.qi_current + amount).clamp(0.0, effective_qi_max);
    (cultivation.qi_current - before).max(0.0)
}

/// 因果 / 业力（plan §1.1）— P1 仅存储权重，推演留给后续切片。
#[derive(Debug, Clone, Default, Component, Serialize, Deserialize)]
pub struct Karma {
    pub weight: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::collections::HashSet;

    use crate::npc::brain::canonical_npc_id;
    use crate::player::state::canonical_player_id;
    use valence::prelude::App;

    #[test]
    fn realm_previous_chain() {
        assert_eq!(Realm::Awaken.previous(), None);
        assert_eq!(Realm::Void.previous(), Some(Realm::Spirit));
    }

    /// plan-agent-ui-data-v1 — Realm::rank() 枚举映射 pin 测试。
    ///
    /// 每个变体独立断言：
    ///   醒灵=1, 引气=2, 凝脉=3, 固元=4, 通灵=5, 化虚=6。
    /// 与 realm_gate 上限 6 一起，确保"仅化虚可见（realm_gate=6）"可被正确表达。
    #[test]
    fn realm_rank_pin_all_variants() {
        assert_eq!(
            Realm::Awaken.rank(),
            1,
            "醒灵 rank 期望=1（realm_gate=1 表示醒灵+），实为 {}",
            Realm::Awaken.rank()
        );
        assert_eq!(
            Realm::Induce.rank(),
            2,
            "引气 rank 期望=2，实为 {}",
            Realm::Induce.rank()
        );
        assert_eq!(
            Realm::Condense.rank(),
            3,
            "凝脉 rank 期望=3，实为 {}",
            Realm::Condense.rank()
        );
        assert_eq!(
            Realm::Solidify.rank(),
            4,
            "固元 rank 期望=4，实为 {}",
            Realm::Solidify.rank()
        );
        assert_eq!(
            Realm::Spirit.rank(),
            5,
            "通灵 rank 期望=5，实为 {}",
            Realm::Spirit.rank()
        );
        assert_eq!(
            Realm::Void.rank(),
            6,
            "化虚 rank 期望=6（realm_gate 上限，仅化虚可见），实为 {}",
            Realm::Void.rank()
        );
    }

    #[test]
    fn realm_meridian_requirements_monotonic() {
        let chain = [
            Realm::Awaken,
            Realm::Induce,
            Realm::Condense,
            Realm::Solidify,
            Realm::Spirit,
            Realm::Void,
        ];
        let mut prev = 0;
        for r in chain {
            let n = r.required_meridians();
            assert!(n > prev, "required_meridians must strictly increase");
            prev = n;
        }
        assert_eq!(chain.map(Realm::required_meridians), [1, 3, 6, 12, 16, 20]);
        assert_eq!(Realm::Void.required_meridians(), 20);
    }

    #[test]
    fn meridian_all_matches_partition_without_duplicates() {
        let merged: Vec<MeridianId> = MeridianId::REGULAR
            .iter()
            .chain(MeridianId::EXTRAORDINARY.iter())
            .copied()
            .collect();
        assert_eq!(merged.as_slice(), MeridianId::ALL.as_slice());

        let unique: HashSet<MeridianId> = MeridianId::ALL.iter().copied().collect();
        assert_eq!(unique.len(), MeridianId::ALL.len());
    }

    #[test]
    fn recover_current_qi_clamps_to_effective_max_without_raising_cap() {
        let mut cultivation = Cultivation {
            qi_current: 160.0,
            qi_max: 210.0,
            qi_max_frozen: Some(30.0),
            ..Default::default()
        };

        let recovered = recover_current_qi(&mut cultivation, 80.0);

        assert_eq!(recovered, 20.0);
        assert_eq!(cultivation.qi_current, 180.0);
        assert_eq!(cultivation.qi_max, 210.0);
        assert_eq!(cultivation.qi_max_frozen, Some(30.0));
    }

    #[test]
    fn qi_color_tracks_permanent_lock_mask() {
        let mut color = QiColor::default();
        color.lock_permanent(ColorKind::Insidious);
        assert!(color.is_permanently_locked(ColorKind::Insidious));
        assert!(!color.is_permanently_locked(ColorKind::Mellow));
    }

    // P2: 混元状态下 lock_permanent 被忽略（混元「无主无从」约束）
    #[test]
    fn lock_permanent_ignored_when_is_hunyuan() {
        let mut color = QiColor {
            is_hunyuan: true,
            ..QiColor::default()
        };
        color.lock_permanent(ColorKind::Sharp);
        assert!(
            !color.is_permanently_locked(ColorKind::Sharp),
            "期望混元状态下 lock_permanent 被忽略（is_hunyuan=true 不允许写入 permanent_lock_mask），实际仍被写入"
        );
        assert!(
            color.permanent_lock_mask.is_empty(),
            "期望混元状态下 permanent_lock_mask 保持空，实际 {:?}",
            color.permanent_lock_mask
        );
    }

    // P2: 非混元时 lock_permanent 正常写入
    #[test]
    fn lock_permanent_works_when_not_hunyuan() {
        let mut color = QiColor {
            is_hunyuan: false,
            ..QiColor::default()
        };
        color.lock_permanent(ColorKind::Violent);
        assert!(
            color.is_permanently_locked(ColorKind::Violent),
            "期望非混元状态下 lock_permanent 正常写入 ColorKind::Violent，但未找到"
        );
    }

    // P2: 混元状态下全部 10 色的 lock_permanent 均被忽略
    #[test]
    fn lock_permanent_ignored_for_all_colors_when_hunyuan() {
        use ColorKind::*;
        let all = [
            Sharp, Heavy, Mellow, Solid, Light, Intricate, Gentle, Insidious, Violent, Turbid,
        ];
        let mut color = QiColor {
            is_hunyuan: true,
            ..QiColor::default()
        };
        for c in all {
            color.lock_permanent(c);
        }
        assert!(
            color.permanent_lock_mask.is_empty(),
            "期望混元状态下对全 10 色调用 lock_permanent 后 mask 仍为空，实际 {:?}",
            color.permanent_lock_mask
        );
    }

    #[test]
    fn meridian_system_defaults_20_closed() {
        let ms = MeridianSystem::default();
        assert_eq!(ms.iter().count(), 20);
        assert_eq!(ms.opened_count(), 0);
    }

    #[test]
    fn meridian_family_partition() {
        for id in MeridianId::REGULAR {
            assert_eq!(id.family(), MeridianFamily::Regular);
        }
        for id in MeridianId::EXTRAORDINARY {
            assert_eq!(id.family(), MeridianFamily::Extraordinary);
        }
    }

    #[test]
    fn meridian_get_mut_roundtrip() {
        let mut ms = MeridianSystem::default();
        ms.get_mut(MeridianId::Lung).opened = true;
        assert!(ms.get(MeridianId::Lung).opened);
        assert_eq!(ms.opened_count(), 1);
        assert_eq!(ms.regular_opened_count(), 1);
        assert_eq!(ms.extraordinary_opened_count(), 0);
    }

    #[test]
    fn meridian_family_opened_counts_are_separate() {
        let mut ms = MeridianSystem::default();
        ms.get_mut(MeridianId::Lung).opened = true;
        ms.get_mut(MeridianId::Ren).opened = true;
        ms.get_mut(MeridianId::Du).opened = true;

        assert_eq!(ms.opened_count(), 3);
        assert_eq!(ms.regular_opened_count(), 1);
        assert_eq!(ms.extraordinary_opened_count(), 2);
    }

    #[test]
    fn contam_source_serde_roundtrip_preserves_attacker_id() {
        let source = ContamSource {
            amount: 2.5,
            color: ColorKind::Violent,
            meridian_id: Some(MeridianId::Lung.channel_id()),
            attacker_id: Some(canonical_player_id("Alice")),
            introduced_at: 77,
        };

        let json = serde_json::to_string(&source).expect("contam source should serialize");
        let decoded: ContamSource =
            serde_json::from_str(&json).expect("contam source should deserialize");

        assert_eq!(decoded.attacker_id.as_deref(), Some("offline:Alice"));
        assert_eq!(
            decoded.meridian_id,
            Some(MeridianChannelId::new("lung")),
            "current-form ContamSource must round-trip its channel id verbatim (snake_case)"
        );
        assert_eq!(decoded.introduced_at, 77);
    }

    #[test]
    fn contam_source_serde_defaults_missing_attacker_id_for_legacy_payloads() {
        let legacy = serde_json::json!({
            "amount": 1.0,
            "color": "Sharp",
            "introduced_at": 9,
        });

        let decoded: ContamSource =
            serde_json::from_value(legacy).expect("legacy contam payload should deserialize");

        assert_eq!(decoded.attacker_id, None);
        assert_eq!(decoded.introduced_at, 9);
    }

    /// P6b review BLOCKER 收口专属 pin：老存档（P1a `MeridianChannelId` 换轨之前）里
    /// `ContamSource.meridian_id` 是 legacy `MeridianId` 的 serde 派生表示——PascalCase
    /// 枚举变体名字符串。反序列化必须把它归一化到当前的 snake_case channel id，而不是
    /// 把 `"Lung"` 原样囫囵存成一个"看起来像新形态但其实大小写不对"的 channel id
    /// （那会让老存档的裂痕路由与 `MeridianSystem`（存储 `"lung"`）永久对不上，静默
    /// 失效而不报错）。
    #[test]
    fn contam_source_deserialize_migrates_legacy_pascal_case_meridian_id_to_channel_id() {
        for (legacy_variant, expected_channel) in [
            ("Lung", "lung"),
            ("Heart", "heart"),
            ("Ren", "ren"),
            ("YangWei", "yang_wei"),
        ] {
            let legacy = serde_json::json!({
                "amount": 3.0,
                "color": "Mellow",
                "meridian_id": legacy_variant,
                "introduced_at": 5,
            });

            let decoded: ContamSource = serde_json::from_value(legacy).unwrap_or_else(|error| {
                panic!(
                    "legacy PascalCase meridian_id {legacy_variant:?} must still decode: {error}"
                )
            });

            assert_eq!(
                decoded.meridian_id,
                Some(MeridianChannelId::new(expected_channel)),
                "legacy PascalCase {legacy_variant:?} must migrate to snake_case channel id \
                 {expected_channel:?}, not be stored verbatim as {legacy_variant:?}"
            );
        }
    }

    /// 非 humanoid channel（换轨后的新形态，如 P5 飞鲸的 `tail_core`）不是任何
    /// legacy `MeridianId` 变体名，必须原样透传，不能被误判成 legacy 表示后丢弃/改写。
    #[test]
    fn contam_source_deserialize_passes_through_non_humanoid_channel_id_verbatim() {
        let current = serde_json::json!({
            "amount": 1.5,
            "color": "Insidious",
            "meridian_id": "tail_core",
            "introduced_at": 3,
        });

        let decoded: ContamSource =
            serde_json::from_value(current).expect("non-humanoid channel id should decode");

        assert_eq!(
            decoded.meridian_id,
            Some(MeridianChannelId::new("tail_core")),
            "non-humanoid channel ids have no legacy MeridianId mapping and must pass through \
             verbatim, not be silently dropped or mutated"
        );
    }

    #[test]
    fn contam_source_deserialize_none_meridian_id_stays_none() {
        let current = serde_json::json!({
            "amount": 1.0,
            "color": "Sharp",
            "meridian_id": null,
            "introduced_at": 1,
        });

        let decoded: ContamSource =
            serde_json::from_value(current).expect("explicit null meridian_id should decode");

        assert_eq!(decoded.meridian_id, None);
    }

    #[test]
    fn canonical_identity_anchors_are_string_based_not_transient_ids() {
        let mut app = App::new();
        let entity = app.world_mut().spawn_empty().id();
        let player_id = canonical_player_id("Wanderer");
        let npc_id = canonical_npc_id(entity);

        assert_eq!(player_id, "offline:Wanderer");
        assert!(npc_id.starts_with(&format!("npc_{}", entity.index())));
        assert!(npc_id.ends_with(&format!("v{}", entity.generation())));
        assert_ne!(npc_id, format!("npc_{}", entity.index()));
        assert_ne!(npc_id, entity.index().to_string());
    }
}
