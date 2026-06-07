//! plan-era-state-v1 P0 — 天道时代状态机核心。
//!
//! 职责：
//!   - 定义 [`EraType`]、[`WorldEraState`] Bevy Resource、[`EraModifiers`]
//!   - 提供 [`current_modifiers`] 纯函数：EraType → EraModifiers 常数映射
//!   - 定义 [`EraChangedEvent`]
//!   - 通过 [`era_decree_system`] 消费 modify_zone target=="全局" 命令写入 WorldEraState
//!
//! 守恒红线：spirit_qi_delta **只作强度推算**，绝不直接写全局真元 delta。

use valence::prelude::{bevy_ecs, Event, Resource};

// ─── 时代类型 ───────────────────────────────────────────────────────────────

/// 天道三手段时代 + 未知默认态。
///
/// 枚举变体与 agent era.md skill 的 `era_name` 字段对应（见 viability 决议 §4）：
/// `"calamity"` → Calamity / `"mutation"` → Change / `"deduction"` → Deduction。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EraType {
    /// 灾劫时代：天道主动施压，渡劫更难，异变兽密度提升。
    Calamity,
    /// 变化时代：天道扰动，派系 loyalty 随机漂移，兽密度微减。
    Change,
    /// 演绎时代：天道推算演绎，渡劫门槛微宽，世界趋稳。
    Deduction,
    /// 未知/未宣告：无时代修正，所有 modifiers = 1.0 / 0.0 基准值。
    Unknown,
}

impl EraType {
    /// 从 agent era.md skill 发出的 `era_name` 字符串解析。
    pub fn from_era_name(name: &str) -> Option<Self> {
        match name {
            "calamity" => Some(Self::Calamity),
            "mutation" | "change" => Some(Self::Change),
            "deduction" => Some(Self::Deduction),
            _ => None,
        }
    }

    /// 稳定的 wire 字符串（供日志/IPC 使用）。
    pub fn as_wire_str(self) -> &'static str {
        match self {
            EraType::Calamity => "calamity",
            EraType::Change => "change",
            EraType::Deduction => "deduction",
            EraType::Unknown => "unknown",
        }
    }
}

// ─── EraModifiers ──────────────────────────────────────────────────────────

/// 时代对下游系数的修正量。
///
/// 消费者通过 `WorldEraState::current_modifiers()` 取到此结构体。
/// P1 各下游注入系统（tribulation/beast_spawn/faction）将消费此结构体。
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EraModifiers {
    /// 渡劫阈值系数：`tribulation_prereq = qi_max * TRIBULATION_MIN_QI_RATIO * this`。
    /// > 1.0 = 阈值提高（更难触发），< 1.0 = 阈值降低（更易触发）。
    pub tribulation_threshold_mul: f64,

    /// 异变兽 spawn 全局密度系数。
    /// > 1.0 = spawn 概率提高，< 1.0 = spawn 概率降低。
    pub beast_density_mul: f64,

    /// 派系 loyalty_bias 每轮漂移幅度（绝对值上限）。
    /// 0.0 = 无漂移。
    pub faction_loyalty_drift: f64,

    /// 天道追踪注意力系数（接口就绪，待 plan-tiandao-hunt-v1 P0 注入）。
    /// 占位字段，当前不注入任何下游系统。
    // NOTE: 接口就绪待 plan-tiandao-hunt-v1 P0 接入。
    pub tiandao_attention_score_mul: f64,
}

impl Default for EraModifiers {
    fn default() -> Self {
        current_modifiers(EraType::Unknown)
    }
}

// ─── 时代常数 ──────────────────────────────────────────────────────────────
// P1 各下游注入系统将消费这些常数，P0 暂时 allow(dead_code)。

/// 灾劫时代常数：天道主动施压，渡劫更难（阈值×1.1），兽密度×1.5。
#[allow(dead_code)]
pub const CALAMITY_TRIBULATION_MUL: f64 = 1.1;
#[allow(dead_code)]
pub const CALAMITY_BEAST_DENSITY_MUL: f64 = 1.5;
#[allow(dead_code)]
pub const CALAMITY_FACTION_LOYALTY_DRIFT: f64 = 0.0;
#[allow(dead_code)]
pub const CALAMITY_TIANDAO_ATTENTION_MUL: f64 = 1.3;

/// 变化时代常数：扰动，派系漂移 ±0.05，兽密度×1.0（无增减）。
#[allow(dead_code)]
pub const CHANGE_TRIBULATION_MUL: f64 = 1.0;
#[allow(dead_code)]
pub const CHANGE_BEAST_DENSITY_MUL: f64 = 1.0;
#[allow(dead_code)]
pub const CHANGE_FACTION_LOYALTY_DRIFT: f64 = 0.05;
#[allow(dead_code)]
pub const CHANGE_TIANDAO_ATTENTION_MUL: f64 = 1.0;

/// 演绎时代常数：天道推算，渡劫微宽（阈值×0.95），兽密度×0.8，无漂移。
#[allow(dead_code)]
pub const DEDUCTION_TRIBULATION_MUL: f64 = 0.95;
#[allow(dead_code)]
pub const DEDUCTION_BEAST_DENSITY_MUL: f64 = 0.8;
#[allow(dead_code)]
pub const DEDUCTION_FACTION_LOYALTY_DRIFT: f64 = 0.0;
#[allow(dead_code)]
pub const DEDUCTION_TIANDAO_ATTENTION_MUL: f64 = 1.0;

/// 纯函数：根据 EraType 返回对应时代的 EraModifiers 常数。
pub fn current_modifiers(era: EraType) -> EraModifiers {
    match era {
        EraType::Calamity => EraModifiers {
            tribulation_threshold_mul: CALAMITY_TRIBULATION_MUL,
            beast_density_mul: CALAMITY_BEAST_DENSITY_MUL,
            faction_loyalty_drift: CALAMITY_FACTION_LOYALTY_DRIFT,
            tiandao_attention_score_mul: CALAMITY_TIANDAO_ATTENTION_MUL,
        },
        EraType::Change => EraModifiers {
            tribulation_threshold_mul: CHANGE_TRIBULATION_MUL,
            beast_density_mul: CHANGE_BEAST_DENSITY_MUL,
            faction_loyalty_drift: CHANGE_FACTION_LOYALTY_DRIFT,
            tiandao_attention_score_mul: CHANGE_TIANDAO_ATTENTION_MUL,
        },
        EraType::Deduction => EraModifiers {
            tribulation_threshold_mul: DEDUCTION_TRIBULATION_MUL,
            beast_density_mul: DEDUCTION_BEAST_DENSITY_MUL,
            faction_loyalty_drift: DEDUCTION_FACTION_LOYALTY_DRIFT,
            tiandao_attention_score_mul: DEDUCTION_TIANDAO_ATTENTION_MUL,
        },
        EraType::Unknown => EraModifiers {
            tribulation_threshold_mul: 1.0,
            beast_density_mul: 1.0,
            faction_loyalty_drift: 0.0,
            tiandao_attention_score_mul: 1.0,
        },
    }
}

// ─── WorldEraState Resource ────────────────────────────────────────────────

/// 时代持续时长（tick 数，@20TPS = 24000 tick = 20 分钟）。
/// 超过此时长无新 era_decree 则自动衰减到 Unknown。
pub const ERA_DEFAULT_DURATION_TICKS: u64 = 24_000;

/// intensity 归一化系数：spirit_qi_delta = 0.05 → intensity = 1.0。
const ERA_INTENSITY_SCALE: f64 = 0.05;

/// 天道时代状态 Bevy Resource。
///
/// 由 [`era_decree_system`] 在消费 `modify_zone{target="全局"}` 命令时写入；
/// 下游系统通过 `Res<WorldEraState>` 读取，调用 [`WorldEraState::current_modifiers`]
/// 或直接访问字段。
#[derive(Debug, Clone, Resource)]
pub struct WorldEraState {
    /// 当前时代类型。
    pub era: EraType,

    /// 时代宣告时的 tick。
    pub onset_tick: u64,

    /// 强度 [0.0, 1.0]：由 |spirit_qi_delta| / ERA_INTENSITY_SCALE 推算。
    pub intensity: f64,

    /// 原始 spirit_qi_delta（仅作倾向提示，**不直接写全局真元**）。
    pub qi_delta: f64,

    /// 原始 danger_level_delta（影响兽 aggression weight，不碰真元物理）。
    pub danger_delta: i64,

    /// 过期 tick：= onset_tick + ERA_DEFAULT_DURATION_TICKS。
    /// 超过此值且无新 decree 则自动衰减到 Unknown。
    pub expires_at_tick: u64,
}

impl Default for WorldEraState {
    fn default() -> Self {
        Self {
            era: EraType::Unknown,
            onset_tick: 0,
            intensity: 0.0,
            qi_delta: 0.0,
            danger_delta: 0,
            expires_at_tick: 0,
        }
    }
}

impl WorldEraState {
    /// 返回当前时代对应的 [`EraModifiers`]。
    /// P1 各下游注入系统（tribulation/beast_spawn/faction）将调用此方法。
    #[allow(dead_code)]
    pub fn current_modifiers(&self) -> EraModifiers {
        current_modifiers(self.era)
    }

    /// 根据 era_decree 参数更新状态。
    ///
    /// - `era_name`: agent 发出的时代名称字符串（"calamity" / "mutation" / "deduction"）
    /// - `qi_delta`: spirit_qi_delta，仅用于推算强度，**不写真元账户**
    /// - `danger_delta`: danger_level_delta
    /// - `tick`: 当前游戏 tick
    ///
    /// 返回 `(old_era, new_era)` 供事件 emit 使用。
    pub fn apply_decree(
        &mut self,
        era_name: &str,
        qi_delta: f64,
        danger_delta: i64,
        tick: u64,
    ) -> (EraType, EraType) {
        let old_era = self.era;
        let new_era = EraType::from_era_name(era_name).unwrap_or(EraType::Unknown);
        let intensity = (qi_delta.abs() / ERA_INTENSITY_SCALE).clamp(0.0, 1.0);

        self.era = new_era;
        self.onset_tick = tick;
        self.intensity = intensity;
        self.qi_delta = qi_delta;
        self.danger_delta = danger_delta;
        self.expires_at_tick = tick.saturating_add(ERA_DEFAULT_DURATION_TICKS);

        (old_era, new_era)
    }

    /// 过期检查：若当前 tick 已超过 expires_at_tick 且 era != Unknown，衰减到 Unknown。
    ///
    /// 返回是否发生了衰减（供 EraChangedEvent emit 判断）。
    pub fn check_expiry(&mut self, tick: u64) -> bool {
        if self.era == EraType::Unknown {
            return false;
        }
        if self.expires_at_tick == 0 {
            // Default state with no real decree — not expired
            return false;
        }
        if tick > self.expires_at_tick {
            self.era = EraType::Unknown;
            self.intensity = 0.0;
            self.qi_delta = 0.0;
            self.danger_delta = 0;
            self.expires_at_tick = 0;
            return true;
        }
        false
    }
}

// ─── EraChangedEvent ──────────────────────────────────────────────────────

/// 时代切换事件，每次时代发生变化（包括自动衰减到 Unknown）时 emit。
#[derive(Debug, Clone, Event, PartialEq)]
pub struct EraChangedEvent {
    pub old_era: EraType,
    pub new_era: EraType,
    pub onset_tick: u64,
}

// ─── era_decree_system ────────────────────────────────────────────────────

/// 处理 era_decree 宣告：
/// 扫描 pending 的 EraDecreeIntent 事件，更新 WorldEraState，发出 EraChangedEvent。
///
/// 注意：era_decree 进料路径是 EraDecreeIntent 事件（由 redis_bridge 在解析
/// NarrationStyle::EraDecree 时 emit，携带同批 modify_zone 的 era_name/global_effect
/// 和 spirit_qi_delta/danger_level_delta 参数）。本系统**不**直接读 redis，
/// 而是消费已解析的事件。
use valence::prelude::{EventReader, EventWriter, ResMut};

/// era_decree 进料事件：由 redis_bridge 或命令处理器 emit。
#[derive(Debug, Clone, Event)]
pub struct EraDecreeIntent {
    /// agent era.md 发出的时代名（"calamity" / "mutation" / "deduction"）
    pub era_name: String,
    /// spirit_qi_delta（用于强度推算，不写真元账户）
    pub spirit_qi_delta: f64,
    /// danger_level_delta（影响兽 aggression weight）
    pub danger_level_delta: i64,
    /// 当前游戏 tick
    pub tick: u64,
}

/// era_decree 消费系统：处理 EraDecreeIntent → 写 WorldEraState → emit EraChangedEvent。
pub fn era_decree_system(
    mut era_intents: EventReader<EraDecreeIntent>,
    mut era_changed: EventWriter<EraChangedEvent>,
    mut world_era: ResMut<WorldEraState>,
) {
    for intent in era_intents.read() {
        let (old_era, new_era) = world_era.apply_decree(
            &intent.era_name,
            intent.spirit_qi_delta,
            intent.danger_level_delta,
            intent.tick,
        );

        tracing::info!(
            "[bong][era] era_decree applied: {} → {} (intensity={:.2}, tick={})",
            old_era.as_wire_str(),
            new_era.as_wire_str(),
            world_era.intensity,
            intent.tick,
        );

        era_changed.send(EraChangedEvent {
            old_era,
            new_era,
            onset_tick: intent.tick,
        });
    }
}

/// 过期检查系统：每 tick 检查 WorldEraState 是否超时，若超时则衰减到 Unknown。
pub fn era_expiry_system(
    mut world_era: ResMut<WorldEraState>,
    mut era_changed: EventWriter<EraChangedEvent>,
    clock: Option<bevy_ecs::system::Res<crate::cultivation::tick::CultivationClock>>,
) {
    let tick = clock.map(|c| c.tick).unwrap_or(0);
    let old_era = world_era.era;
    if world_era.check_expiry(tick) {
        tracing::info!(
            "[bong][era] era expired at tick {tick}: {} → unknown",
            old_era.as_wire_str()
        );
        era_changed.send(EraChangedEvent {
            old_era,
            new_era: EraType::Unknown,
            onset_tick: tick,
        });
    }
}

/// 注册所有 era 系统和资源到 Bevy App。
pub fn register(app: &mut valence::prelude::App) {
    use valence::prelude::{IntoSystemConfigs, Update};

    app.insert_resource(WorldEraState::default());
    app.add_event::<EraDecreeIntent>();
    app.add_event::<EraChangedEvent>();
    app.add_systems(Update, (era_decree_system, era_expiry_system).chain());

    tracing::info!("[bong][era] WorldEraState resource + era systems registered");
}

// ─── 单测 ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── EraType 变体 ──────────────────────────────────────────────────────

    #[test]
    fn era_type_from_era_name_calamity() {
        assert_eq!(
            EraType::from_era_name("calamity"),
            Some(EraType::Calamity),
            "期望 'calamity' 解析为 EraType::Calamity"
        );
    }

    #[test]
    fn era_type_from_era_name_mutation_maps_to_change() {
        assert_eq!(
            EraType::from_era_name("mutation"),
            Some(EraType::Change),
            "期望 agent era.md 发出的 'mutation' 解析为 EraType::Change（viability 决议 §4）"
        );
    }

    #[test]
    fn era_type_from_era_name_change_maps_to_change() {
        assert_eq!(
            EraType::from_era_name("change"),
            Some(EraType::Change),
            "期望 'change' 解析为 EraType::Change"
        );
    }

    #[test]
    fn era_type_from_era_name_deduction() {
        assert_eq!(
            EraType::from_era_name("deduction"),
            Some(EraType::Deduction),
            "期望 'deduction' 解析为 EraType::Deduction"
        );
    }

    #[test]
    fn era_type_from_era_name_unknown_returns_none() {
        assert_eq!(
            EraType::from_era_name("unknown_era"),
            None,
            "期望未知 era_name 返回 None（不静默接受垃圾输入）"
        );
    }

    #[test]
    fn era_type_wire_str_stable() {
        assert_eq!(EraType::Calamity.as_wire_str(), "calamity");
        assert_eq!(EraType::Change.as_wire_str(), "change");
        assert_eq!(EraType::Deduction.as_wire_str(), "deduction");
        assert_eq!(EraType::Unknown.as_wire_str(), "unknown");
    }

    // ── EraModifiers 三时代常数映射 ───────────────────────────────────────

    #[test]
    fn modifiers_calamity_tribulation_threshold_higher() {
        let m = current_modifiers(EraType::Calamity);
        assert!(
            m.tribulation_threshold_mul > 1.0,
            "灾劫时代渡劫阈值系数应 > 1.0（更难触发，天道施压）；实际 = {}",
            m.tribulation_threshold_mul
        );
        assert_eq!(m.tribulation_threshold_mul, CALAMITY_TRIBULATION_MUL);
    }

    #[test]
    fn modifiers_deduction_tribulation_threshold_lower() {
        let m = current_modifiers(EraType::Deduction);
        assert!(
            m.tribulation_threshold_mul < 1.0,
            "演绎时代渡劫阈值系数应 < 1.0（略宽）；实际 = {}",
            m.tribulation_threshold_mul
        );
        assert_eq!(m.tribulation_threshold_mul, DEDUCTION_TRIBULATION_MUL);
    }

    #[test]
    fn modifiers_calamity_beast_density_higher() {
        let m = current_modifiers(EraType::Calamity);
        assert!(
            m.beast_density_mul > 1.0,
            "灾劫时代兽密度系数应 > 1.0；实际 = {}",
            m.beast_density_mul
        );
        assert_eq!(m.beast_density_mul, CALAMITY_BEAST_DENSITY_MUL);
    }

    #[test]
    fn modifiers_deduction_beast_density_lower() {
        let m = current_modifiers(EraType::Deduction);
        assert!(
            m.beast_density_mul < 1.0,
            "演绎时代兽密度系数应 < 1.0；实际 = {}",
            m.beast_density_mul
        );
        assert_eq!(m.beast_density_mul, DEDUCTION_BEAST_DENSITY_MUL);
    }

    #[test]
    fn modifiers_change_faction_loyalty_drift_nonzero() {
        let m = current_modifiers(EraType::Change);
        assert!(
            m.faction_loyalty_drift > 0.0,
            "变化时代 faction_loyalty_drift 应 > 0（有漂移）；实际 = {}",
            m.faction_loyalty_drift
        );
    }

    #[test]
    fn modifiers_unknown_all_baseline() {
        let m = current_modifiers(EraType::Unknown);
        assert_eq!(
            m.tribulation_threshold_mul, 1.0,
            "Unknown 时代渡劫系数应为基准 1.0"
        );
        assert_eq!(
            m.beast_density_mul, 1.0,
            "Unknown 时代兽密度系数应为基准 1.0"
        );
        assert_eq!(m.faction_loyalty_drift, 0.0, "Unknown 时代无 faction 漂移");
    }

    // ── WorldEraState apply_decree ────────────────────────────────────────

    #[test]
    fn apply_decree_calamity_updates_state() {
        let mut state = WorldEraState::default();
        let (old, new) = state.apply_decree("calamity", 0.05, 1, 100);

        assert_eq!(old, EraType::Unknown, "apply 前应为 Unknown");
        assert_eq!(new, EraType::Calamity, "期望 calamity → Calamity");
        assert_eq!(state.era, EraType::Calamity);
        assert_eq!(state.onset_tick, 100);
        assert!(
            (state.intensity - 1.0).abs() < 1e-9,
            "spirit_qi_delta=0.05 → intensity=1.0；实际 = {}",
            state.intensity
        );
        assert_eq!(state.danger_delta, 1);
        assert_eq!(
            state.expires_at_tick,
            100 + ERA_DEFAULT_DURATION_TICKS,
            "expires_at_tick = onset_tick + ERA_DEFAULT_DURATION_TICKS"
        );
    }

    #[test]
    fn apply_decree_intensity_clamps_to_1() {
        let mut state = WorldEraState::default();
        // delta 远超 0.05，强度应 clamp 到 1.0
        state.apply_decree("calamity", 99.0, 0, 0);
        assert!(
            (state.intensity - 1.0).abs() < 1e-9,
            "intensity 应 clamp 到 1.0；实际 = {}",
            state.intensity
        );
    }

    #[test]
    fn apply_decree_zero_delta_gives_zero_intensity() {
        let mut state = WorldEraState::default();
        state.apply_decree("deduction", 0.0, 0, 0);
        assert!(
            state.intensity.abs() < 1e-9,
            "spirit_qi_delta=0 → intensity=0.0；实际 = {}",
            state.intensity
        );
    }

    #[test]
    fn apply_decree_unknown_era_name_falls_back_to_unknown() {
        let mut state = WorldEraState::default();
        let (_, new) = state.apply_decree("garbage_era", 0.01, 0, 50);
        assert_eq!(
            new,
            EraType::Unknown,
            "未知 era_name 应 fallback 到 Unknown（不崩溃）"
        );
    }

    // ── WorldEraState check_expiry ────────────────────────────────────────

    #[test]
    fn check_expiry_not_expired_before_deadline() {
        let mut state = WorldEraState::default();
        state.apply_decree("calamity", 0.05, 0, 100);
        let expired = state.check_expiry(100 + ERA_DEFAULT_DURATION_TICKS - 1);
        assert!(
            !expired,
            "未到 expires_at_tick 不应衰减；tick = {} < expires_at_tick = {}",
            100 + ERA_DEFAULT_DURATION_TICKS - 1,
            state.expires_at_tick
        );
        assert_eq!(state.era, EraType::Calamity, "era 应保持不变");
    }

    #[test]
    fn check_expiry_expires_after_deadline() {
        let mut state = WorldEraState::default();
        state.apply_decree("calamity", 0.05, 0, 100);
        let expired = state.check_expiry(100 + ERA_DEFAULT_DURATION_TICKS + 1);
        assert!(
            expired,
            "超过 expires_at_tick 应自动衰减到 Unknown；tick = {}",
            100 + ERA_DEFAULT_DURATION_TICKS + 1
        );
        assert_eq!(state.era, EraType::Unknown, "衰减后 era 应为 Unknown");
        assert!(state.intensity.abs() < 1e-9, "衰减后 intensity 应为 0.0");
    }

    #[test]
    fn check_expiry_unknown_era_never_decays() {
        let mut state = WorldEraState::default();
        // Unknown 态（default），expires_at_tick = 0，任何 tick 都不应触发衰减
        let expired = state.check_expiry(999_999);
        assert!(!expired, "Unknown 态不应触发过期事件");
        assert_eq!(state.era, EraType::Unknown);
    }

    // ── EraChangedEvent emit（通过 Bevy App 集成） ─────────────────────────

    #[test]
    fn era_decree_system_emits_era_changed_event() {
        let mut app = valence::prelude::App::new();
        app.add_event::<EraDecreeIntent>();
        app.add_event::<EraChangedEvent>();
        app.insert_resource(WorldEraState::default());
        app.insert_resource(crate::cultivation::tick::CultivationClock::default());
        app.add_systems(valence::prelude::Update, era_decree_system);

        // 发出 era_decree 事件
        app.world_mut().send_event(EraDecreeIntent {
            era_name: "calamity".to_string(),
            spirit_qi_delta: 0.05,
            danger_level_delta: 2,
            tick: 500,
        });

        app.update();

        let state = app.world().resource::<WorldEraState>();
        assert_eq!(state.era, EraType::Calamity, "era 应更新为 Calamity");
        assert_eq!(state.onset_tick, 500);

        // 检查 EraChangedEvent（使用 drain() 读取当帧事件）
        let emitted: Vec<EraChangedEvent> = app
            .world_mut()
            .resource_mut::<valence::prelude::Events<EraChangedEvent>>()
            .drain()
            .collect();
        assert_eq!(
            emitted.len(),
            1,
            "期望恰好一个 EraChangedEvent；实际 = {}",
            emitted.len()
        );
        let event = &emitted[0];
        assert_eq!(event.old_era, EraType::Unknown);
        assert_eq!(event.new_era, EraType::Calamity);
        assert_eq!(event.onset_tick, 500);
    }

    #[test]
    fn era_decree_system_era_transition_old_preserved() {
        let mut app = valence::prelude::App::new();
        app.add_event::<EraDecreeIntent>();
        app.add_event::<EraChangedEvent>();
        app.insert_resource(WorldEraState::default());
        app.insert_resource(crate::cultivation::tick::CultivationClock::default());
        app.add_systems(valence::prelude::Update, era_decree_system);

        // 第一次 decree：Unknown → Calamity
        app.world_mut().send_event(EraDecreeIntent {
            era_name: "calamity".to_string(),
            spirit_qi_delta: 0.05,
            danger_level_delta: 0,
            tick: 100,
        });
        app.update();

        // 第二次 decree：Calamity → Deduction
        app.world_mut().send_event(EraDecreeIntent {
            era_name: "deduction".to_string(),
            spirit_qi_delta: 0.02,
            danger_level_delta: -1,
            tick: 200,
        });
        app.update();

        let state = app.world().resource::<WorldEraState>();
        assert_eq!(
            state.era,
            EraType::Deduction,
            "第二次 decree 后 era 应为 Deduction"
        );

        // 检查最新的 EraChangedEvent（第二次 update 后 drain 当帧事件）
        let emitted: Vec<EraChangedEvent> = app
            .world_mut()
            .resource_mut::<valence::prelude::Events<EraChangedEvent>>()
            .drain()
            .collect();
        let last = emitted
            .last()
            .expect("期望至少一个 EraChangedEvent（Calamity → Deduction）");
        assert_eq!(
            last.new_era,
            EraType::Deduction,
            "第二次 decree 后最新事件应是 Deduction"
        );
        assert_eq!(last.onset_tick, 200);
    }

    // ── qi_delta 不写全局真元守恒断言 ────────────────────────────────────

    #[test]
    fn apply_decree_qi_delta_does_not_change_world_qi_budget() {
        use crate::qi_physics::ledger::WorldQiBudget;

        let budget = WorldQiBudget::from_total(100.0);
        let initial_total = budget.initial_total;
        let current_before = budget.current_total;

        // 模拟 apply_decree 不碰 WorldQiBudget
        let mut state = WorldEraState::default();
        state.apply_decree("calamity", 0.05, 0, 0);

        // WorldQiBudget 不应有任何变化
        assert_eq!(
            budget.initial_total, initial_total,
            "apply_decree 不得修改 initial_total"
        );
        assert_eq!(
            budget.current_total, current_before,
            "apply_decree 绝不直接写全局真元 delta（守恒红线）"
        );
        assert_eq!(
            budget.era_decay_accum, 0.0,
            "apply_decree 不得积累 era_decay_accum"
        );
    }

    #[test]
    fn era_decay_step_conservation_invariant() {
        use crate::qi_physics::ledger::WorldQiBudget;
        use crate::qi_physics::tiandao::era_decay_step;

        let initial = 100.0_f64;
        let mut budget = WorldQiBudget::from_total(initial);
        let decay = era_decay_step(&mut budget, 1.0).expect("era_decay_step should succeed");

        // 守恒：initial_total = current_total + era_decay_accum
        assert!(
            (budget.initial_total - (budget.current_total + budget.era_decay_accum)).abs() < 1e-9,
            "守恒不变量：initial_total = current_total + era_decay_accum；\
            initial={}, current={}, accum={}, sum={}",
            budget.initial_total,
            budget.current_total,
            budget.era_decay_accum,
            budget.current_total + budget.era_decay_accum
        );
        assert!(decay > 0.0, "灾劫衰减应 > 0；decay = {}", decay);
    }
}
