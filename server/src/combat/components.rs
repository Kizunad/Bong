use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use valence::prelude::{bevy_ecs, Component};

use crate::combat::events::StatusEffectKind;

const DEFAULT_HEALTH_MAX: f32 = 100.0;
const DEFAULT_STAMINA_MAX: f32 = 100.0;
const DEFAULT_STAMINA_RECOVER_PER_SEC: f32 = 5.0;
const DEFAULT_FORTUNE_REMAINING: u8 = 3;

pub const TICKS_PER_SECOND: u64 = 20;
pub const ATTACK_STAMINA_COST: f32 = 3.0;
pub const IN_COMBAT_WINDOW_TICKS: u64 = 15 * TICKS_PER_SECOND;
pub const NEAR_DEATH_WINDOW_TICKS: u64 = 30 * TICKS_PER_SECOND;
pub const REVIVAL_CONFIRM_WINDOW_TICKS: u64 = 60 * TICKS_PER_SECOND;
pub const REVIVE_WEAKENED_TICKS: u64 = 180 * TICKS_PER_SECOND;
pub const BLEED_TICK_INTERVAL_TICKS: u64 = TICKS_PER_SECOND;
pub const HEALTH_REGEN_TICK_INTERVAL_TICKS: u64 = TICKS_PER_SECOND;
pub const STAMINA_TICK_INTERVAL_TICKS: u64 = 4;
pub const COMBAT_STATE_TICK_INTERVAL_TICKS: u64 = TICKS_PER_SECOND;
pub const NEAR_DEATH_HEALTH_FRACTION: f32 = 0.05;
pub const REVIVE_HEALTH_FRACTION: f32 = 0.20;
pub const STATUS_EFFECT_TICK_INTERVAL_TICKS: u64 = 4;
pub const LEG_SLOWED_SEVERITY_THRESHOLD: f32 = 0.3;
pub const LEG_SLOWED_DURATION_TICKS: u64 = 40;
pub const HEAD_STUN_SEVERITY_THRESHOLD: f32 = 0.5;
pub const HEAD_STUN_DURATION_TICKS: u64 = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BodyPart {
    Head,
    Chest,
    Back,
    Abdomen,
    ArmL,
    ArmR,
    LegL,
    LegR,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WoundKind {
    Cut,
    Blunt,
    Pierce,
    Burn,
    Concussion,
}

fn default_wound_kind() -> WoundKind {
    WoundKind::Blunt
}

/// plan-race-system-v1 P0 review r2（BLOCKING-2 收口）—— `location` 从 legacy 8 段
/// `BodyPart` unit enum 迁移为通用 `body_plan::BodyPartId`（string），伤口不再假设
/// 人形躯体。命中几何（`combat::raycast`）现在按目标实体的 `BodyPlan.hit_geometry`
/// 分派（`HeightBands`/`PartBoxes`），产出的部位 id 直接就是本字段的值——非人形构型
/// （如 P5 whale 的 `tail_fin`）不再需要"能反向映射回 legacy enum"这个前提才能受伤。
///
/// **迁移范围**：仅本字段与直接读取它的伤残后果消费点（`combat::arm_wound` /
/// `movement::leg_wound` / 腿伤减速 / 头伤眩晕 / 臂伤脱手，均已改为按目标实体解析出
/// 的 `BodyPlan.parts[].consequence` 分派）。仍以 legacy `BodyPart` 工作的人形专属
/// 子系统（`CombatEvent.body_part` wire / `DerivedAttrs.defense_profile` / 护甲
/// `body_coverage` / `DeadMeridianArmor.immune_regions` / `dugu::body_part_to_meridian`
/// / 状态效果 `BodyPartResist`/`BodyPartWeaken` / dandao 变异伤害倍率）本轮**不**跟进
/// 迁移（P1 经脉/wire 批次范围）——消费这些系统时在各自调用点显式转换
/// `body_plan::id_to_legacy_body_part`，非人形部位 id 转换失败时的行为逐点注释说明，
/// 不做静默 filter_map。
///
/// **不影响持久化 / wire**：`Wound`/`Wounds` 是纯战斗运行时 ECS 组件，从不写入 sqlite
/// （`Wounds::default()` 在重连/重生时重置）；`wounds_snapshot` wire 的 `part` 字段
/// 本就是 `String`（`network::wounds_snapshot_emit::body_part_wire` 只是把 legacy 8 段
/// 映射到更细的 16 段字符串——非人形 id 现在原样透传，见该函数注释）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Wound {
    pub location: crate::body_plan::BodyPartId,
    #[serde(default = "default_wound_kind")]
    pub kind: WoundKind,
    pub severity: f32,
    pub bleeding_per_sec: f32,
    pub created_at_tick: u64,
    pub inflicted_by: Option<String>,
}

#[derive(Debug, Clone, Component, Serialize, Deserialize)]
pub struct Wounds {
    pub entries: Vec<Wound>,
    pub health_current: f32,
    pub health_max: f32,
}

impl Default for Wounds {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            health_current: DEFAULT_HEALTH_MAX,
            health_max: DEFAULT_HEALTH_MAX,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StaminaState {
    Idle,
    Walking,
    Jogging,
    Sprinting,
    Combat,
    Exhausted,
    /// plan-shield-block-v1 P2 — 玩家举盾（ShieldBlocking 状态激活）时的持续体力消耗态。
    /// drain = SHIELD_DRAIN_PER_SEC (3.0/s)，不触 qi_physics（体力非真元）。
    ShieldBlocking,
}

/// plan-shield-block-v1 P4 — 盾牌格挡的每秒体力 drain 覆写（仅 ShieldBlocking 状态生效）。
/// `raise_shield_handler` 在玩家举盾时插入此 component，存储经 `shield_block_profile`
/// 按熟练度缩放后的 drain_per_s（范围 2.0..3.0）。
/// `stamina_tick` 读取此 component，覆写 `SHIELD_DRAIN_PER_SEC` 常量（P2 fallback 仍为 3.0）。
#[derive(Debug, Clone, Component)]
pub struct ShieldDrainOverride {
    /// 每秒体力消耗量（>= 2.0，由 `shield_block_profile.drain_per_s` 提供）。
    pub drain_per_s: f32,
}

#[derive(Debug, Clone, Component, Serialize, Deserialize)]
pub struct Stamina {
    pub current: f32,
    pub max: f32,
    pub recover_per_sec: f32,
    pub last_drain_tick: Option<u64>,
    pub state: StaminaState,
}

impl Default for Stamina {
    fn default() -> Self {
        Self {
            current: DEFAULT_STAMINA_MAX,
            max: DEFAULT_STAMINA_MAX,
            recover_per_sec: DEFAULT_STAMINA_RECOVER_PER_SEC,
            last_drain_tick: None,
            state: StaminaState::Idle,
        }
    }
}

impl Stamina {
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn normalized(&self) -> Self {
        let max = self.max.max(1.0);

        let mut normalized = self.clone();
        normalized.max = max;
        normalized.current = self.current.clamp(0.0, max);
        normalized.recover_per_sec = self.recover_per_sec.max(0.0);

        if normalized.current <= 0.0 && normalized.state == StaminaState::Sprinting {
            normalized.state = StaminaState::Exhausted;
        }

        normalized
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DefenseWindow {
    pub opened_at_tick: u64,
    pub duration_ms: u32,
}

impl DefenseWindow {
    pub fn expires_at_tick(&self) -> u64 {
        self.opened_at_tick
            .saturating_add((u64::from(self.duration_ms).saturating_add(49)) / 50)
    }
}

#[derive(Debug, Clone, Component, Default, Serialize, Deserialize)]
pub struct CombatState {
    pub in_combat_until_tick: Option<u64>,
    pub last_attack_at_tick: Option<u64>,
    pub incoming_window: Option<DefenseWindow>,
}

/// 仅挂在存在战斗窗口的实体上，供逐 tick 到期检查避免扫描全部战斗组件。
#[derive(Debug, Clone, Copy, Component, Default)]
pub struct ActiveCombatWindow;

impl CombatState {
    pub fn refresh_combat_window(&mut self, now_tick: u64) {
        let until_tick = now_tick.saturating_add(IN_COMBAT_WINDOW_TICKS);
        self.in_combat_until_tick = Some(self.in_combat_until_tick.unwrap_or(0).max(until_tick));
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LifecycleState {
    Alive,
    NearDeath,
    AwaitingRevival,
    Terminated,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum RevivalDecision {
    Fortune { chance: f64 },
    Tribulation { chance: f64 },
}

impl RevivalDecision {
    pub fn chance_shown(self) -> f64 {
        match self {
            Self::Fortune { chance } | Self::Tribulation { chance } => chance,
        }
    }

    pub fn can_reincarnate(self) -> bool {
        true
    }

    pub fn can_terminate(self) -> bool {
        matches!(self, Self::Tribulation { .. })
    }
}

#[derive(Debug, Clone, Component, Serialize, Deserialize)]
pub struct Lifecycle {
    pub character_id: String,
    pub death_count: u32,
    pub fortune_remaining: u8,
    pub last_death_tick: Option<u64>,
    pub last_revive_tick: Option<u64>,
    /// 玩家灵龛坐标（如有）。
    ///
    /// 仅用于重生点选择与“拥有灵龛归属”判定；灵龛保护/揭露等社交语义由 plan-social-v1 承接。
    #[serde(default)]
    pub spawn_anchor: Option<[f64; 3]>,
    #[serde(default)]
    pub spawn_anchor_damaged: bool,
    #[serde(default)]
    pub near_death_deadline_tick: Option<u64>,
    #[serde(default)]
    pub awaiting_decision: Option<RevivalDecision>,
    #[serde(default)]
    pub revival_decision_deadline_tick: Option<u64>,
    pub weakened_until_tick: Option<u64>,
    pub state: LifecycleState,
}

impl Default for Lifecycle {
    fn default() -> Self {
        Self {
            character_id: "unbound:character".to_string(),
            death_count: 0,
            fortune_remaining: DEFAULT_FORTUNE_REMAINING,
            last_death_tick: None,
            last_revive_tick: None,
            spawn_anchor: None,
            spawn_anchor_damaged: false,
            near_death_deadline_tick: None,
            awaiting_decision: None,
            revival_decision_deadline_tick: None,
            weakened_until_tick: None,
            state: LifecycleState::Alive,
        }
    }
}

impl Lifecycle {
    pub fn enter_near_death(&mut self, now_tick: u64) {
        if self.state == LifecycleState::NearDeath {
            return;
        }

        self.death_count = self.death_count.saturating_add(1);
        self.last_death_tick = Some(now_tick);
        self.near_death_deadline_tick = Some(now_tick.saturating_add(NEAR_DEATH_WINDOW_TICKS));
        self.state = LifecycleState::NearDeath;
    }

    pub fn revive(&mut self, now_tick: u64) {
        self.revive_with_weakened_multiplier(now_tick, 1);
    }

    pub fn revive_with_weakened_multiplier(&mut self, now_tick: u64, weakened_multiplier: u64) {
        self.last_revive_tick = Some(now_tick);
        self.near_death_deadline_tick = None;
        self.awaiting_decision = None;
        self.revival_decision_deadline_tick = None;
        self.weakened_until_tick = Some(
            now_tick.saturating_add(REVIVE_WEAKENED_TICKS.saturating_mul(weakened_multiplier)),
        );
        self.state = LifecycleState::Alive;
    }

    pub fn await_revival_decision(&mut self, decision: RevivalDecision, deadline_tick: u64) {
        self.near_death_deadline_tick = None;
        self.awaiting_decision = Some(decision);
        self.revival_decision_deadline_tick = Some(deadline_tick);
        self.state = LifecycleState::AwaitingRevival;
    }

    pub fn terminate(&mut self, now_tick: u64) {
        self.last_death_tick = Some(now_tick);
        self.near_death_deadline_tick = None;
        self.awaiting_decision = None;
        self.revival_decision_deadline_tick = None;
        self.state = LifecycleState::Terminated;
    }
}

#[derive(Debug, Clone, Component, Serialize, Deserialize)]
pub struct DerivedAttrs {
    pub attack_power: f32,
    pub defense_power: f32,
    pub move_speed_multiplier: f32,
    #[serde(default = "default_one_f32")]
    pub jump_height_multiplier: f32,
    /// plan-armor-v1 §1.2：被动护甲二维矩阵（BodyPart × WoundKind -> mitigation）。
    /// 查询 miss 表示该部位/伤害类型无护甲减免。
    #[serde(default)]
    pub defense_profile: HashMap<(BodyPart, WoundKind), f32>,
    /// plan-HUD-v1 §3.4 / plan-woliu-v1 §3.1.G：替尸伪皮剩余层数。
    #[serde(default)]
    pub tuike_layers: u8,
    /// plan-HUD-v1 §3.4 / plan-woliu-v1 §3.1.G：绝灵涡流当前激活态。
    #[serde(default)]
    pub vortex_active: bool,

    /// bughunt r4-P2#5：QiCapPermMinus debuff 导致的真元上限削减系数 [0.01, 1.0]。
    /// 1.0 = 无削减（无效果）；< 1.0 = 上限折损。由 attribute_aggregate_tick 每帧重算。
    #[serde(default = "default_one_f64")]
    pub qi_max_multiplier: f64,

    // ── plan-baomai-v4 §2.4 疤纹回路被动效果 ──
    /// 三阳合流：近战 reach 加成（blocks）。0.0 = 无加成。
    #[serde(default)]
    pub reach_bonus: f64,
    /// 心肺短路：焚血期间 qi regen 倍率。1.0 = 无加成。
    #[serde(default = "default_one_f64")]
    pub qi_regen_multiplier: f64,
    /// 肝肾交汇：contamination 排毒速率倍率。1.0 = 无加成。
    #[serde(default = "default_one_f64")]
    pub contam_purge_multiplier: f64,
    /// 脾肾固本：安全区经脉自愈速率倍率。1.0 = 无加成。
    #[serde(default = "default_one_f64")]
    pub healing_rate_multiplier: f64,

    // ── plan-baomai-v4 §3.3 活茧被动效果 ──
    /// 茧皮：BRUISE 伤害阈值倍率。1.0 = 无加成。
    #[serde(default = "default_one_f64")]
    pub bruise_threshold_multiplier: f64,
    /// 茧骨：FRACTURE 降级为 LACERATION 的概率 [0.0, 1.0]。
    #[serde(default)]
    pub fracture_downgrade_chance: f64,
    /// 茧肉：Cut/Pierce 类伤害降一档（LACERATION→ABRASION）。
    #[serde(default)]
    pub cut_pierce_downgrade: bool,
    /// 茧灵：有活跃回路的经脉 flow_rate +5%。
    #[serde(default)]
    pub scar_forged_flow_bonus: bool,
}

fn default_one_f32() -> f32 {
    1.0
}

fn default_one_f64() -> f64 {
    1.0
}

impl Default for DerivedAttrs {
    fn default() -> Self {
        Self {
            attack_power: 1.0,
            defense_power: 1.0,
            move_speed_multiplier: 1.0,
            jump_height_multiplier: 1.0,
            defense_profile: HashMap::new(),
            tuike_layers: 0,
            vortex_active: false,
            // bughunt r4-P2#5 QiCapPermMinus derived reducer (neutral default)
            qi_max_multiplier: 1.0,
            // plan-baomai-v4 scar circuit passives (neutral defaults)
            reach_bonus: 0.0,
            qi_regen_multiplier: 1.0,
            contam_purge_multiplier: 1.0,
            healing_rate_multiplier: 1.0,
            // plan-baomai-v4 iron cocoon passives (neutral defaults)
            bruise_threshold_multiplier: 1.0,
            fracture_downgrade_chance: 0.0,
            cut_pierce_downgrade: false,
            scar_forged_flow_bonus: false,
        }
    }
}

/// plan-armor-v1 §4.2 — 体修流派标记 component（MVP：仅标记，buff 由 status.rs 应用）。
///
/// 体修"不依赖外物"：通过 defense_power 基础加成（1.0/1.3 ≈ 0.77）替代护甲。
/// 此 component 可穿护甲，但 buff 与护甲 kind_mitigation 独立相乘。
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct BodyRefiningMarker;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveStatusEffect {
    pub kind: StatusEffectKind,
    pub magnitude: f32,
    pub remaining_ticks: u64,
    /// plan-cultivation-pacing-v1 §8.1 #7：丹药来源 PillKind ID，用于 per-pill cap。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_pill: Option<String>,
}

#[derive(Debug, Clone, Component, Default, Serialize, Deserialize)]
pub struct StatusEffects {
    pub active: Vec<ActiveStatusEffect>,
}

/// plan-HUD-v1 §4 玩家正在 cast 快捷槽时挂在 Player 实体上。
/// 完成 / 中断后移除。
#[derive(Debug, Clone, Component)]
pub struct Casting {
    pub source: CastSource,
    pub slot: u8,
    pub started_at_tick: u64,
    pub duration_ticks: u64,
    /// 推 `cast_sync` 给 client 时直接用 unix ms，避免 client 反推 tick 时间。
    pub started_at_ms: u64,
    pub duration_ms: u32,
    /// 完成时要消耗的 item instance_id（绑定时刻快照），cast 期间该物品丢出
    /// 背包则 complete 时找不到 → 视同失败（v1 不报错）。
    pub bound_instance_id: Option<u64>,
    /// 开始 cast 时玩家位置（plan §4.3 移动中断阈值用）。
    pub start_position: valence::prelude::DVec3,
    /// 完成成功后写到 QuickSlotBindings 的冷却 tick 数（中断走另一个固定值）。
    pub complete_cooldown_ticks: u64,
    /// SkillBar 技能 id。QuickSlot/物品 cast 为 None。
    pub skill_id: Option<String>,
    /// SkillBar cast 开始时的配置快照；cast 中修改不影响本次结算。
    #[allow(dead_code)]
    pub skill_config: Option<crate::skill::config::SkillConfig>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CastSource {
    QuickSlot,
    SkillBar,
}

/// plan-HUD-v1 §1.3 / §11.4 玩家解锁的防御流派。控制流派指示器的
/// 条件渲染门禁——未解锁完全不渲染（§1.4）。
///
/// v1 默认全部解锁以便观察 HUD；后续接入修炼系统按真实解锁条件 mutate。
#[derive(Debug, Clone, Copy, Component, PartialEq, Eq)]
pub struct UnlockedStyles {
    pub jiemai: bool,
    pub tishi: bool,
    pub jueling: bool,
}

impl Default for UnlockedStyles {
    fn default() -> Self {
        Self {
            jiemai: true,
            tishi: true,
            jueling: true,
        }
    }
}

/// plan-HUD-v1 §10.4 玩家 F1-F9 快捷槽 → 物品 instance_id 绑定。
/// 由 `quick_slot_bind` 客户端 intent 写入，`use_quick_slot` 时按 slot 取 instance。
/// 同时跟踪每个 slot 的 cooldown（plan §4.4）。
#[derive(Debug, Clone, Component, Default)]
pub struct QuickSlotBindings {
    pub slots: [Option<u64>; 9],
    /// 每个 slot 下次可用的 server tick；0 表示无冷却。
    pub cooldown_until_tick: [u64; 9],
}

impl QuickSlotBindings {
    pub const SLOT_COUNT: usize = 9;

    pub fn get(&self, slot: u8) -> Option<u64> {
        if slot as usize >= Self::SLOT_COUNT {
            return None;
        }
        self.slots[slot as usize]
    }

    pub fn set(&mut self, slot: u8, instance_id: Option<u64>) -> bool {
        if slot as usize >= Self::SLOT_COUNT {
            return false;
        }
        self.slots[slot as usize] = instance_id;
        true
    }

    pub fn is_on_cooldown(&self, slot: u8, now_tick: u64) -> bool {
        if slot as usize >= Self::SLOT_COUNT {
            return false;
        }
        self.cooldown_until_tick[slot as usize] > now_tick
    }

    pub fn set_cooldown(&mut self, slot: u8, until_tick: u64) {
        if (slot as usize) < Self::SLOT_COUNT {
            self.cooldown_until_tick[slot as usize] = until_tick;
        }
    }
}

/// plan-hotbar-modify-v1 §3.1 玩家 1-9 技能栏槽位。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum SkillSlot {
    #[default]
    Empty,
    Item {
        instance_id: u64,
    },
    Skill {
        skill_id: String,
    },
}

/// plan-hotbar-modify-v1 §3.1 玩家 1-9 技能栏绑定 + 冷却。
///
/// bughunt skillbar-rebind-cooldown-reset（返工裁决）：冷却曾按**槽位**记账
/// （`cooldown_until_tick: [u64; 9]`），`set()` 在换绑时清零对应槽——这不仅让
/// "把同一招式重新拖回原槽位"变成免费冷却重置，连"清空槽位→重新绑定"
/// "A→B→A 往返换绑"都同样清零冷却（换绑到 B 时即已清零 A 的冷却，绑回 A 时
/// A 冷却已不在）；且同一招式绑到多个槽位时，每个槽位各算各的冷却，等价于把
/// 冷却复制了 N 份，同样可无限连发。这两条路径都与"changed 才清零"的判断条件
/// 无关——只要冷却还挂在槽位上，任何槽位内容变更都是一次潜在的重置/复制手段。
///
/// 根治：冷却按 **skill_id** 归属而非槽位。`set()` 现在只改槽内容，从不触碰
/// 任何冷却 entry；`is_on_cooldown`/`set_cooldown` 只认 skill_id 字符串，
/// 与该 skill 当前绑在哪个槽、绑了几个槽都无关，天然消除上述两条攻击面。
#[derive(Debug, Clone, Component, Default)]
pub struct SkillBarBindings {
    pub slots: [SkillSlot; 9],
    /// key = skill_id（如 `"dugu.eclipse"`），value = cooldown_until_tick。
    /// 没有 entry 视为无冷却（就绪）。
    pub cooldowns: HashMap<String, u64>,
}

impl SkillBarBindings {
    pub const SLOT_COUNT: usize = 9;

    pub fn get(&self, slot: u8) -> Option<&SkillSlot> {
        if slot as usize >= Self::SLOT_COUNT {
            return None;
        }
        Some(&self.slots[slot as usize])
    }

    /// 绑定/换绑 `slot`。**绝不触碰任何冷却 entry**——冷却按 skill_id 归属，
    /// 与槽位内容变更完全解耦，杜绝"清空→重绑同招""A→B→A 往返换绑""同招绑
    /// 多槽"等冷却重置/复制手段（bughunt skillbar-rebind-cooldown-reset）。
    pub fn set(&mut self, slot: u8, value: SkillSlot) -> bool {
        if slot as usize >= Self::SLOT_COUNT {
            return false;
        }
        self.slots[slot as usize] = value;
        true
    }

    /// `skill_id` 从未 cast 过（无 cooldowns entry）视为不在冷却中。
    pub fn is_on_cooldown(&self, skill_id: &str, now_tick: u64) -> bool {
        self.cooldowns
            .get(skill_id)
            .is_some_and(|&until| now_tick < until)
    }

    pub fn set_cooldown(&mut self, skill_id: &str, until_tick: u64) {
        self.cooldowns.insert(skill_id.to_string(), until_tick);
    }

    /// baomai_v3 肉身超脱（Body Transcendence）窗口刻意设计的"冷却全清"奖励
    /// （`combat/baomai_v3/skills.rs::apply_transcendence_window`）——一次性
    /// 清空该玩家全部技能冷却 entry。这是本 bug 修复范围之外的既有设计，语义
    /// 上等价于旧 `cooldown_until_tick = [0; SLOT_COUNT]`，迁移到 skill_id
    /// 记账后同样保留为唯一的批量清零入口。
    pub fn clear_all_cooldowns(&mut self) {
        self.cooldowns.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lifecycle_character_id_supports_canonical_string_ids() {
        let player_lifecycle = Lifecycle {
            character_id: "offline:Alice".to_string(),
            ..Default::default()
        };
        let npc_lifecycle = Lifecycle {
            character_id: "npc_42v7".to_string(),
            ..Default::default()
        };

        assert_eq!(player_lifecycle.character_id, "offline:Alice");
        assert_eq!(npc_lifecycle.character_id, "npc_42v7");
    }

    #[test]
    fn stamina_normalized_clamps_values_and_exhausts_invalid_sprint() {
        let stamina = Stamina {
            current: -8.0,
            max: 0.0,
            recover_per_sec: -2.0,
            last_drain_tick: Some(12),
            state: StaminaState::Sprinting,
        };

        let normalized = stamina.normalized();

        assert_eq!(normalized.max, 1.0);
        assert_eq!(normalized.current, 0.0);
        assert_eq!(normalized.recover_per_sec, 0.0);
        assert_eq!(normalized.last_drain_tick, Some(12));
        assert_eq!(normalized.state, StaminaState::Exhausted);
    }

    // ─────────────────────────────────────────────────────────────────
    // bughunt skillbar-rebind-cooldown-reset — 冷却曾按槽位记账，`set()` 换绑时
    // 清零该槽冷却；返工后彻底改为按 skill_id 记账，`set()` 不再触碰任何冷却，
    // `is_on_cooldown`/`set_cooldown` 只认 skill_id，与槽位内容/数量无关。
    // ─────────────────────────────────────────────────────────────────

    const BENG_QUAN: &str = "burst_meridian.beng_quan";
    const TIE_SHAN_KAO: &str = "burst_meridian.tie_shan_kao";

    #[test]
    fn set_same_value_never_touches_cooldown() {
        let mut bindings = SkillBarBindings::default();
        assert!(bindings.set(
            0,
            SkillSlot::Skill {
                skill_id: BENG_QUAN.to_string()
            }
        ));
        bindings.set_cooldown(BENG_QUAN, 500);

        // 重新绑定完全相同的技能——`set()` 根本不看冷却，必须原封不动。
        assert!(bindings.set(
            0,
            SkillSlot::Skill {
                skill_id: BENG_QUAN.to_string()
            }
        ));

        assert_eq!(
            bindings.cooldowns.get(BENG_QUAN).copied(),
            Some(500),
            "同值重绑不该触碰 cooldowns map，期望 entry 仍为 500，实际 {:?}",
            bindings.cooldowns.get(BENG_QUAN)
        );
        assert!(bindings.is_on_cooldown(BENG_QUAN, 0));
    }

    #[test]
    fn set_different_value_never_touches_either_skills_cooldown() {
        // 换绑到不同技能是正常换绑体验，但**不再**清零任何技能的冷却——
        // 冷却按 skill_id 归属，与槽位内容变化完全解耦（否则就是 A→B→A 换绑
        // 清空 A 冷却的攻击面，见 skill_bar_bindings_rebind_a_to_b_back_to_a_preserves_a_cooldown）。
        let mut bindings = SkillBarBindings::default();
        assert!(bindings.set(
            0,
            SkillSlot::Skill {
                skill_id: BENG_QUAN.to_string()
            }
        ));
        bindings.set_cooldown(BENG_QUAN, 500);

        assert!(bindings.set(
            0,
            SkillSlot::Skill {
                skill_id: TIE_SHAN_KAO.to_string()
            }
        ));

        assert!(
            bindings.is_on_cooldown(BENG_QUAN, 0),
            "换绑走的槽位与 beng_quan 的冷却记账完全无关，beng_quan 的冷却必须原样保留"
        );
        assert!(
            !bindings.is_on_cooldown(TIE_SHAN_KAO, 0),
            "tie_shan_kao 从未被 set_cooldown 过，理应不在冷却中"
        );
    }

    #[test]
    fn set_empty_to_value_boundary_never_touches_cooldown() {
        // 边界：从默认 Empty 绑定到具体值——历史上这是"changed=true → 清零冷却"的
        // 边界分支；现在 set() 完全不查 changed，也就不存在这个分支了，行为统一为
        // "冷却 map 岿然不动"。
        let mut bindings = SkillBarBindings::default();
        assert!(matches!(bindings.slots[0], SkillSlot::Empty));
        bindings.set_cooldown(BENG_QUAN, 500);

        assert!(bindings.set(0, SkillSlot::Item { instance_id: 42 }));

        assert!(matches!(
            bindings.slots[0],
            SkillSlot::Item { instance_id: 42 }
        ));
        assert!(
            bindings.is_on_cooldown(BENG_QUAN, 0),
            "Empty→Item 换绑不应清零任何技能的冷却"
        );
    }

    #[test]
    fn set_out_of_range_returns_false_and_leaves_state_untouched() {
        let mut bindings = SkillBarBindings::default();
        bindings.set_cooldown(BENG_QUAN, 500);

        let ok = bindings.set(
            SkillBarBindings::SLOT_COUNT as u8,
            SkillSlot::Skill {
                skill_id: BENG_QUAN.to_string(),
            },
        );

        assert!(!ok, "越界 slot 必须返回 false");
        // 越界写入不应影响任何已有状态（槽位内容 + 冷却）。
        assert_eq!(bindings.cooldowns.get(BENG_QUAN).copied(), Some(500));
        assert!(matches!(bindings.slots[0], SkillSlot::Empty));
    }

    #[test]
    fn skill_bar_bindings_rebind_a_to_b_back_to_a_preserves_a_cooldown() {
        // bughunt skillbar-rebind-cooldown-reset 阻塞问题 A（往返换绑路径）：
        // 施放 beng_quan（冷却 500）→ 换绑同槽到 tie_shan_kao → 再换绑回
        // beng_quan——beng_quan 的冷却必须全程原样保留，不能被中途的换绑动作
        // 以任何方式清零一次。
        let mut bindings = SkillBarBindings::default();
        assert!(bindings.set(
            0,
            SkillSlot::Skill {
                skill_id: BENG_QUAN.to_string()
            }
        ));
        bindings.set_cooldown(BENG_QUAN, 500);

        assert!(bindings.set(
            0,
            SkillSlot::Skill {
                skill_id: TIE_SHAN_KAO.to_string()
            }
        ));
        assert!(bindings.set(
            0,
            SkillSlot::Skill {
                skill_id: BENG_QUAN.to_string()
            }
        ));

        assert!(
            bindings.is_on_cooldown(BENG_QUAN, 0),
            "A→B→A 往返换绑不得作为清空 A 冷却的手段"
        );
        assert_eq!(bindings.cooldowns.get(BENG_QUAN).copied(), Some(500));
    }

    #[test]
    fn skill_bar_bindings_same_skill_bound_to_multiple_slots_shares_one_cooldown() {
        // bughunt skillbar-rebind-cooldown-reset 阻塞问题 B（同技能绑多槽路径）：
        // 冷却按 skill_id 而非槽位记账，同一招式绑在几个槽上都共享同一份冷却，
        // 不会被"每槽各算各的"复制成 N 份可连发的冷却。
        let mut bindings = SkillBarBindings::default();
        assert!(bindings.set(
            0,
            SkillSlot::Skill {
                skill_id: BENG_QUAN.to_string()
            }
        ));
        assert!(bindings.set(
            3,
            SkillSlot::Skill {
                skill_id: BENG_QUAN.to_string()
            }
        ));
        assert!(bindings.set(
            8,
            SkillSlot::Skill {
                skill_id: BENG_QUAN.to_string()
            }
        ));

        // 只需在任意一个绑定槽施放一次——set_cooldown 不接收 slot，天然全局生效。
        bindings.set_cooldown(BENG_QUAN, 200);

        assert!(
            bindings.is_on_cooldown(BENG_QUAN, 0),
            "施放后 beng_quan 应进入冷却，且与从哪个槽施放无关"
        );
        // 旧实现下这里会是 3 个独立的 [u64;9] 槽位，只有 slot 0 会显示冷却；
        // 新实现下 is_on_cooldown 压根不接收 slot，槽位数量对判定结果零影响。
        assert_eq!(
            bindings.cooldowns.len(),
            1,
            "一个 skill_id 只应有一条 cooldowns entry"
        );
    }

    #[test]
    fn is_on_cooldown_boundary_ticks() {
        // 与 QuickSlotBindings 的边界语义对齐：now_tick < until 才算冷却中，
        // 恰好相等（到期瞬间）及之后均视为就绪。
        let mut bindings = SkillBarBindings::default();
        assert!(
            !bindings.is_on_cooldown(BENG_QUAN, 100),
            "从未 cast 过的技能不应报冷却中"
        );

        bindings.set_cooldown(BENG_QUAN, 130);
        assert!(bindings.is_on_cooldown(BENG_QUAN, 100));
        assert!(bindings.is_on_cooldown(BENG_QUAN, 129));
        assert!(
            !bindings.is_on_cooldown(BENG_QUAN, 130),
            "恰好到期（等号）应视为就绪"
        );
        assert!(!bindings.is_on_cooldown(BENG_QUAN, 131));
    }

    #[test]
    fn is_on_cooldown_unknown_skill_id_is_always_ready() {
        // 从未 set_cooldown 过的 skill_id（包括压根没在任何槽绑定过的）没有 map
        // entry，必须视为就绪，而不是 panic 或误报冷却中。
        let bindings = SkillBarBindings::default();
        assert!(!bindings.is_on_cooldown("no.such.skill", u64::MAX));
    }

    #[test]
    fn clear_all_cooldowns_resets_every_tracked_skill() {
        // baomai_v3 肉身超脱窗口的刻意设计：一次性清空全部冷却，与本 bug 修复
        // 目标（防止玩家自行触发的清零）不冲突——这是唯一被保留的批量清零入口。
        let mut bindings = SkillBarBindings::default();
        bindings.set_cooldown(BENG_QUAN, 500);
        bindings.set_cooldown(TIE_SHAN_KAO, 999);
        assert!(bindings.is_on_cooldown(BENG_QUAN, 0));
        assert!(bindings.is_on_cooldown(TIE_SHAN_KAO, 0));

        bindings.clear_all_cooldowns();

        assert!(!bindings.is_on_cooldown(BENG_QUAN, 0));
        assert!(!bindings.is_on_cooldown(TIE_SHAN_KAO, 0));
        assert!(bindings.cooldowns.is_empty());
    }
}
