use serde::{Deserialize, Serialize};
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
pub const REVIVE_WEAKENED_TICKS: u64 = 180 * TICKS_PER_SECOND;
pub const BLEED_TICK_INTERVAL_TICKS: u64 = TICKS_PER_SECOND;
pub const STAMINA_TICK_INTERVAL_TICKS: u64 = 4;
pub const COMBAT_STATE_TICK_INTERVAL_TICKS: u64 = TICKS_PER_SECOND;
pub const NEAR_DEATH_HEALTH_FRACTION: f32 = 0.05;
pub const REVIVE_HEALTH_FRACTION: f32 = 0.20;
pub const JIEMAI_DEFENSE_WINDOW_MS: u32 = 200;
pub const JIEMAI_DEFENSE_QI_COST: f64 = 5.0;
pub const JIEMAI_CONTAM_MULTIPLIER: f64 = 0.2;
pub const JIEMAI_CONCUSSION_SEVERITY: f32 = 0.3;
pub const JIEMAI_CONCUSSION_BLEEDING_PER_SEC: f32 = 0.0;
pub const STATUS_EFFECT_TICK_INTERVAL_TICKS: u64 = 4;
pub const LEG_SLOWED_SEVERITY_THRESHOLD: f32 = 0.3;
pub const LEG_SLOWED_DURATION_TICKS: u64 = 40;
pub const HEAD_STUN_SEVERITY_THRESHOLD: f32 = 0.5;
pub const HEAD_STUN_DURATION_TICKS: u64 = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BodyPart {
    Head,
    Chest,
    Abdomen,
    ArmL,
    ArmR,
    LegL,
    LegR,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Wound {
    pub location: BodyPart,
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

#[derive(Debug, Clone, Component, Serialize, Deserialize)]
pub struct Lifecycle {
    pub character_id: String,
    pub death_count: u32,
    pub fortune_remaining: u8,
    pub last_death_tick: Option<u64>,
    pub last_revive_tick: Option<u64>,
    #[serde(default)]
    pub near_death_deadline_tick: Option<u64>,
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
            near_death_deadline_tick: None,
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
        self.last_revive_tick = Some(now_tick);
        self.near_death_deadline_tick = None;
        self.weakened_until_tick = Some(now_tick.saturating_add(REVIVE_WEAKENED_TICKS));
        self.state = LifecycleState::Alive;
    }

    pub fn terminate(&mut self, now_tick: u64) {
        self.last_death_tick = Some(now_tick);
        self.near_death_deadline_tick = None;
        self.state = LifecycleState::Terminated;
    }
}

#[derive(Debug, Clone, Component, Serialize, Deserialize)]
pub struct DerivedAttrs {
    pub attack_power: f32,
    pub defense_power: f32,
    pub move_speed_multiplier: f32,
}

impl Default for DerivedAttrs {
    fn default() -> Self {
        Self {
            attack_power: 1.0,
            defense_power: 1.0,
            move_speed_multiplier: 1.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveStatusEffect {
    pub kind: StatusEffectKind,
    pub magnitude: f32,
    pub remaining_ticks: u64,
}

#[derive(Debug, Clone, Component, Default, Serialize, Deserialize)]
pub struct StatusEffects {
    pub active: Vec<ActiveStatusEffect>,
}

/// plan-HUD-v1 §4 玩家正在 cast F1-F9 快捷槽时挂在 Player 实体上。
/// 完成 / 中断后移除。
#[derive(Debug, Clone, Component)]
pub struct Casting {
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
}
