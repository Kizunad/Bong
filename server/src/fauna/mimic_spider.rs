//! 拟态灰烬蛛 — plan-fauna-mimic-spider-v1 P0/P3
//!
//! 三态状态机 Disguised / Ambush / Retreat，伏击型妖兽。
//! P0 实装：
//!   - SpiderDisguiseState enum（三态）
//!   - MimicSpiderBlackboard component（含 drained_qi 字段）
//!   - 感知 / 几何阈值常数（留本模块，qi_physics 速率常数归 constants.rs）
//!   - Disguised 期 qi 吸收系统（走 qi_physics::regen_from_zone + QiTransfer，守恒）
//!   - 死亡 qi 归还系统（mirror fauna/rat_phase.rs release_drained_qi_on_death_system）
//!
//! P3 实装：
//!   - `SpiderTrapPotential` component（陷阱归属 + 放置时间戳）
//!   - `SPIDER_TRAP_TIMEOUT_TICKS` 常数（72 游戏内天 = 72 × 24_000 tick）
//!   - `SPIDER_CAGE_TEMPLATE_ID` 常数（item.spider_cage 模板 ID，与 fauna.toml 对齐）
//!   - `spider_trap_timeout_system`（超时自动释放陷阱蛛 → Retreat）
//!
//! item.spider_cage 的配方不在本 plan 范围，由 anqi-v2 引用。

use serde::{Deserialize, Serialize};
use valence::prelude::{
    bevy_ecs, Component, DVec3, Entity, Event, EventReader, IntoSystemConfigs, Position, Query,
    Res, ResMut, With,
};

use crate::combat::events::DeathEvent;
use crate::cultivation::tick::CultivationClock;
use crate::npc::spawn::NpcMarker;
use crate::qi_physics::constants::SPIDER_DISGUISE_REGEN_RATE;
use crate::qi_physics::excretion::regen_from_zone;
use crate::qi_physics::ledger::{QiAccountId, QiTransfer, QiTransferReason, WorldQiAccount};
use crate::world::dimension::CurrentDimension;
use crate::world::zone::ZoneRegistry;

// ── 几何 / 感知阈值常数（归本模块；qi 速率常数归 qi_physics::constants）────────────

/// worldview §七：感知触发阈值 = qi_max * 此比例。
/// 引气期玩家（qi_max ≈ 10~200）均高于此值，醒灵期（趋近 0）不触发。
pub const SPIDER_QI_SENSE_THRESHOLD: f64 = 0.1;

/// 感知半径（方块数）：蛛在此范围内检测玩家真元并决定是否暴起。
pub const SPIDER_SENSE_RADIUS: f64 = 8.0;

/// 撤退判定半径（方块数）：退出此距离视为完成撤退。
pub const SPIDER_RETREAT_RADIUS: f64 = 32.0;

// ── P3 陷阱常数 ───────────────────────────────────────────────────────────────

/// 陷阱笼物品模板 ID（与 server/assets/items/fauna.toml 中的 `id = "spider_cage"` 对齐）。
/// anqi-v2 引用此常数注册配方，本 plan 不实装配方。
pub const SPIDER_CAGE_TEMPLATE_ID: &str = "spider_cage";

/// 陷阱超时 tick 数：72 游戏内天（72 × 24_000 tick）。
/// 超过此时间，被捕获的蛛自动进入 Retreat 并解除 trap 归属。
pub const SPIDER_TRAP_TIMEOUT_TICKS: u64 = 72 * 24_000;

// ──────────────────────────────────────────────────────────────────────────────

/// 拟态灰烬蛛三态状态机。
///
/// - `Disguised`：静止伪装为灰烬方块，持续吸收 zone spirit_qi（正守恒）。
/// - `Ambush`：暴起追击，停止吸收。
/// - `Retreat`：危险时向低灵气区撤退，完成后回 `Disguised`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Component)]
#[serde(rename_all = "snake_case")]
pub enum SpiderDisguiseState {
    Disguised,
    Ambush,
    Retreat,
}

impl Default for SpiderDisguiseState {
    fn default() -> Self {
        Self::Disguised
    }
}

/// 拟态灰烬蛛个体 blackboard。
///
/// - `drained_qi`：Disguised 期累计从 zone 吸走的真元量，死亡时归还（等比衰减）。
/// - `home_zone`：孵化区域名称，用于 zone 查找。
/// - `home_pos`：出生位置，Retreat 阶段的方向参考。
/// - `trapped_by`：P3 陷阱归属（None = 野生）。
#[derive(Debug, Clone, PartialEq, Component)]
pub struct MimicSpiderBlackboard {
    pub home_zone: String,
    pub home_pos: DVec3,
    pub drained_qi: f64,
    /// P3 陷阱归属；P0/P1/P2 保持 None。
    pub trapped_by: Option<Entity>,
}

impl MimicSpiderBlackboard {
    pub fn new(home_zone: &str, home_pos: DVec3) -> Self {
        Self {
            home_zone: home_zone.to_string(),
            home_pos,
            drained_qi: 0.0,
            trapped_by: None,
        }
    }
}

/// P3 陷阱归属 component。
///
/// 当玩家以 `item.spider_cage` 对处于 `Disguised` 状态的蛛使用时，
/// server 端添加此 component，标记陷阱归属和放置时间戳。
///
/// # 语义约束
///
/// - `trap_owner`：放置陷阱的玩家 Entity；陷阱触发时蛛不攻击此玩家。
/// - `placed_at`：放置世界坐标（供后续 P-future 地图渲染使用）。
/// - `placed_tick`：放置时的 `CultivationClock::tick`；超过
///   `placed_tick + SPIDER_TRAP_TIMEOUT_TICKS` 时 `spider_trap_timeout_system` 自动释放。
///
/// 释放方式：从蛛身上移除此 component + 清空 `blackboard.trapped_by` + 转为 Retreat。
#[derive(Debug, Clone, Component)]
pub struct SpiderTrapPotential {
    /// 部署陷阱的玩家；陷阱激活时不攻击此玩家（仅攻击第三方）。
    pub trap_owner: Entity,
    /// 放置坐标（语义参考，不影响 AI 决策）。
    pub placed_at: DVec3,
    /// 放置时的 `CultivationClock::tick`。
    pub placed_tick: u64,
}

/// P3 陷阱超时释放系统。
///
/// 每 tick 扫描带有 `SpiderTrapPotential` 的蛛：
///   - 若 `current_tick >= placed_tick + SPIDER_TRAP_TIMEOUT_TICKS`，
///     则移除 `SpiderTrapPotential`、清空 `blackboard.trapped_by`、
///     强制切换到 `SpiderDisguiseState::Retreat`（让 big-brain 正常接手撤退路径）。
///
/// # 设计说明
/// 超时不立即 Despawn 蛛，而是进入 Retreat——符合 worldview §七"蛛自主游走，不被永久禁锢"。
pub fn spider_trap_timeout_system(
    mut commands: Commands,
    clock: Option<Res<CultivationClock>>,
    mut spiders: Query<
        (
            Entity,
            &SpiderTrapPotential,
            &mut MimicSpiderBlackboard,
            &mut SpiderDisguiseState,
        ),
        With<NpcMarker>,
    >,
) {
    let current_tick = clock.map(|c| c.tick).unwrap_or(0);

    for (entity, trap, mut blackboard, mut state) in &mut spiders {
        if current_tick >= trap.placed_tick + SPIDER_TRAP_TIMEOUT_TICKS {
            // 超时：解除陷阱归属，强制进入 Retreat（自然撤退回出生地后变回 Disguised）
            blackboard.trapped_by = None;
            *state = SpiderDisguiseState::Retreat;
            commands.entity(entity).remove::<SpiderTrapPotential>();
        }
    }
}

use valence::prelude::Commands;

/// P0 Disguised 期 qi 吸收系统。
///
/// 每 tick 以 `SPIDER_DISGUISE_REGEN_RATE` 从所在 zone spirit_qi 吸收真元：
///   - 调用 `regen_from_zone(zone_qi, rate=1.0, integrity=1.0, room)`
///   - 走 `WorldQiAccount::transfer` 保证双账户守恒
///   - 无 Cultivation component（蛛不修炼），用 `blackboard.drained_qi` 记账
///
/// # 守恒约束
/// zone.spirit_qi 减少量 == blackboard.drained_qi 累计增量（除精度误差）。
type SpiderAbsorbQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Position,
        Option<&'static CurrentDimension>,
        &'static SpiderDisguiseState,
        &'static mut MimicSpiderBlackboard,
    ),
    With<NpcMarker>,
>;

pub fn spider_disguised_qi_absorb_system(
    mut spiders: SpiderAbsorbQuery<'_, '_>,
    mut zones: Option<ResMut<ZoneRegistry>>,
    mut ledger: Option<ResMut<WorldQiAccount>>,
) {
    let (Some(zones), Some(ledger)) = (zones.as_deref_mut(), ledger.as_deref_mut()) else {
        return;
    };

    for (entity, pos, dim, state, mut blackboard) in &mut spiders {
        // 只在 Disguised 状态吸收
        if *state != SpiderDisguiseState::Disguised {
            continue;
        }

        let dim_kind = dim
            .map(|d| d.0)
            .unwrap_or(crate::world::dimension::DimensionKind::Overworld);

        let Some(zone_name) = zones.find_zone(dim_kind, pos.get()).map(|z| z.name.clone()) else {
            continue;
        };
        let Some(zone) = zones.find_zone_mut(&zone_name) else {
            continue;
        };

        // 负灵域 spirit_qi <= 0 时不吸收
        if zone.spirit_qi <= 0.0 {
            continue;
        }

        // qi 上限：蛛吸收额度上限 = BeastKind::Spider 的 qi_max（25.0 × 0.1 = 2.5）
        // 这里用 qi_max 类比：drained_qi 无上限（会随死亡衰减回区域），room = 饱和上限
        let qi_spider_cap = crate::fauna::components::BeastKind::Spider.health_max() as f64;
        let room = (qi_spider_cap - blackboard.drained_qi).max(0.0);
        if room <= 0.0 {
            continue;
        }

        // regen_from_zone(zone_qi, rate, integrity, room) 返回 (gain, drain)
        // rate 参数用 SPIDER_DISGUISE_REGEN_RATE（已乘入 QI_CULTIVATION_REGEN_RATE 基底）
        let (gain, drain) = regen_from_zone(
            zone.spirit_qi,
            SPIDER_DISGUISE_REGEN_RATE,
            1.0, // 蛛无经脉，integrity=1.0 全效
            room,
        );

        if gain <= 0.0 || drain <= 0.0 {
            continue;
        }

        // 走 ledger 双账户守恒
        let zone_account = QiAccountId::zone(zone.name.clone());
        let spider_account = QiAccountId::npc(format!("mimic_spider:{}", entity.index()));

        // set_balance 对齐账户
        if ledger
            .set_balance(
                zone_account.clone(),
                zone.spirit_qi.max(0.0) * crate::qi_physics::constants::QI_ZONE_UNIT_CAPACITY,
            )
            .is_err()
        {
            continue;
        }
        if ledger
            .set_balance(spider_account.clone(), blackboard.drained_qi.max(0.0))
            .is_err()
        {
            continue;
        }

        let Ok(transfer) = QiTransfer::new(
            zone_account,
            spider_account.clone(),
            gain,
            QiTransferReason::CultivationRegen,
        ) else {
            continue;
        };

        if ledger.transfer(transfer).is_ok() {
            blackboard.drained_qi = ledger.balance(&spider_account);
            zone.spirit_qi = (zone.spirit_qi - drain).max(0.0);
        }
    }
}

/// 死亡时将 drained_qi 的 1% 归还给所在 zone（与 rat_phase 等比策略一致）。
///
/// 蛛死亡时散逸：真元随蛛尸分解慢慢回归环境，而非全量即时释放（防止刷怪刷灵气）。
pub fn spider_release_qi_on_death_system(
    mut deaths: EventReader<DeathEvent>,
    spiders: Query<(&Position, Option<&CurrentDimension>, &MimicSpiderBlackboard), With<NpcMarker>>,
    mut zones: Option<ResMut<ZoneRegistry>>,
) {
    let Some(zones) = zones.as_deref_mut() else {
        for _ in deaths.read() {}
        return;
    };

    for death in deaths.read() {
        let Ok((position, dimension, blackboard)) = spiders.get(death.target) else {
            continue;
        };
        if blackboard.drained_qi <= 0.0 {
            continue;
        }
        let dim = dimension
            .map(|d| d.0)
            .unwrap_or(crate::world::dimension::DimensionKind::Overworld);
        let Some(zone_name) = zones.find_zone(dim, position.get()).map(|z| z.name.clone()) else {
            continue;
        };
        if let Some(zone) = zones.find_zone_mut(zone_name.as_str()) {
            // 1% 即时散逸，与 rat_phase 相同策略，守恒（剩余 99% 随时间蒸发进 excretion）
            zone.spirit_qi = (zone.spirit_qi + blackboard.drained_qi * 0.01).clamp(-1.0, 1.0);
        }
    }
}

/// P0 内部：判断某位置玩家真元是否超过感知阈值（用于 P1 SpiderAmbushScorer）。
///
/// 该函数纯粹用于测试和 P1 scorer 接入，不属于 ECS system。
pub fn exceeds_qi_sense_threshold(qi_current: f64, qi_max: f64) -> bool {
    qi_current > qi_max * SPIDER_QI_SENSE_THRESHOLD
}

/// P0 内部：判断目标是否在感知半径内。
pub fn within_sense_radius(spider_pos: DVec3, target_pos: DVec3) -> bool {
    spider_pos.distance(target_pos) <= SPIDER_SENSE_RADIUS
}

/// P0 内部：判断蛛是否达到撤退完成条件（距离触发者超过 RETREAT_RADIUS）。
pub fn retreat_complete(spider_pos: DVec3, threat_pos: DVec3) -> bool {
    spider_pos.distance(threat_pos) >= SPIDER_RETREAT_RADIUS
}

/// Bevy 事件：拟态蛛状态转变（供 P1 系统读取 + 测试断言）。
#[derive(Debug, Clone, Event, PartialEq, Serialize, Deserialize)]
pub struct SpiderStateChangeEvent {
    pub spider: u32,
    pub from: SpiderDisguiseState,
    pub to: SpiderDisguiseState,
    pub tick: u64,
}

/// P0/P3 事件 + 系统注册入口（由 fauna::register 调用）。
pub fn register(app: &mut valence::prelude::App) {
    app.add_event::<SpiderStateChangeEvent>();
    app.add_systems(
        valence::prelude::Update,
        (
            spider_disguised_qi_absorb_system,
            spider_release_qi_on_death_system.before(crate::fauna::drop::fauna_drop_system),
            spider_trap_timeout_system,
        ),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use valence::prelude::{App, DVec3, Update};

    use crate::combat::events::DeathEvent;
    use crate::cultivation::tick::CultivationClock;
    use crate::npc::spawn::NpcMarker;
    use crate::world::zone::ZoneRegistry;

    fn make_blackboard(zone: &str, pos: DVec3) -> MimicSpiderBlackboard {
        MimicSpiderBlackboard::new(zone, pos)
    }

    /// 构建带 spawn zone 的 ZoneRegistry，用指定 spirit_qi 覆盖默认值。
    fn zone_registry_with_qi(spirit_qi: f64) -> ZoneRegistry {
        let mut registry = ZoneRegistry::fallback();
        registry
            .find_zone_mut("spawn")
            .expect("fallback ZoneRegistry must have spawn zone")
            .spirit_qi = spirit_qi;
        registry
    }

    // ── enum / 默认值 pin 测试 ─────────────────────────────────────────

    #[test]
    fn spider_disguise_state_default_is_disguised() {
        assert_eq!(
            SpiderDisguiseState::default(),
            SpiderDisguiseState::Disguised,
            "默认状态必须是 Disguised（伪装），蛛出生即隐身"
        );
    }

    #[test]
    fn spider_disguise_state_all_variants_serialize_round_trip() {
        let variants = [
            SpiderDisguiseState::Disguised,
            SpiderDisguiseState::Ambush,
            SpiderDisguiseState::Retreat,
        ];
        for v in variants {
            let json = serde_json::to_string(&v).expect("serialize must succeed");
            let back: SpiderDisguiseState =
                serde_json::from_str(&json).expect("deserialize must succeed");
            assert_eq!(v, back, "状态 {v:?} 序列化往返必须等价");
        }
    }

    #[test]
    fn spider_disguise_state_wire_names_locked() {
        // wire 名稳定性：改变量名时此测试必须失败，提示同步更新 wire 契约。
        assert_eq!(
            serde_json::to_string(&SpiderDisguiseState::Disguised).unwrap(),
            "\"disguised\""
        );
        assert_eq!(
            serde_json::to_string(&SpiderDisguiseState::Ambush).unwrap(),
            "\"ambush\""
        );
        assert_eq!(
            serde_json::to_string(&SpiderDisguiseState::Retreat).unwrap(),
            "\"retreat\""
        );
    }

    // ── 常数 pin 测试 ──────────────────────────────────────────────────

    #[test]
    fn spider_qi_sense_threshold_pin() {
        // 设计决议 §8.1：0.1 × qi_max，醒灵期（qi_current→0）不触发
        assert!(
            (SPIDER_QI_SENSE_THRESHOLD - 0.1).abs() < 1e-12,
            "期望 SPIDER_QI_SENSE_THRESHOLD == 0.1，实际 {SPIDER_QI_SENSE_THRESHOLD}"
        );
    }

    #[test]
    fn spider_sense_radius_pin() {
        assert!(
            (SPIDER_SENSE_RADIUS - 8.0).abs() < 1e-12,
            "期望 SPIDER_SENSE_RADIUS == 8.0 方块，实际 {SPIDER_SENSE_RADIUS}"
        );
    }

    #[test]
    fn spider_retreat_radius_pin() {
        assert!(
            (SPIDER_RETREAT_RADIUS - 32.0).abs() < 1e-12,
            "期望 SPIDER_RETREAT_RADIUS == 32.0 方块，实际 {SPIDER_RETREAT_RADIUS}"
        );
    }

    // ── 感知阈值边界测试 ───────────────────────────────────────────────

    #[test]
    fn qi_sense_threshold_not_triggered_at_zero_qi() {
        // 醒灵期零真元——不应触发
        assert!(
            !exceeds_qi_sense_threshold(0.0, 10.0),
            "qi_current=0 不应触发感知（醒灵期安全）"
        );
    }

    #[test]
    fn qi_sense_threshold_triggered_above_threshold() {
        // qi_current = 0.11 × qi_max > 0.1 × qi_max，应触发
        assert!(
            exceeds_qi_sense_threshold(1.1, 10.0),
            "qi_current > 10% qi_max 应触发感知"
        );
    }

    #[test]
    fn qi_sense_threshold_exactly_at_boundary_not_triggered() {
        // qi_current = 0.1 × qi_max，严格大于才触发
        assert!(
            !exceeds_qi_sense_threshold(1.0, 10.0),
            "qi_current == 10% qi_max 不应触发（严格大于）"
        );
    }

    #[test]
    fn qi_sense_threshold_high_realm_player_always_triggers() {
        // 通灵期玩家 qi_max ≈ 500，qi_current ≈ 400 >> 50（0.1×500）
        assert!(
            exceeds_qi_sense_threshold(400.0, 500.0),
            "高境界玩家在感知半径内必触发"
        );
    }

    // ── 几何判断测试 ───────────────────────────────────────────────────

    #[test]
    fn within_sense_radius_exact_boundary() {
        let spider = DVec3::new(0.0, 64.0, 0.0);
        let target = DVec3::new(8.0, 64.0, 0.0); // distance == 8.0 == SPIDER_SENSE_RADIUS
        assert!(
            within_sense_radius(spider, target),
            "距离 == 感知半径时应在范围内（含边界）"
        );
    }

    #[test]
    fn within_sense_radius_outside() {
        let spider = DVec3::new(0.0, 64.0, 0.0);
        let target = DVec3::new(8.1, 64.0, 0.0); // distance > 8.0
        assert!(
            !within_sense_radius(spider, target),
            "距离 > 感知半径时不在范围内"
        );
    }

    #[test]
    fn retreat_complete_at_boundary() {
        let spider = DVec3::new(32.0, 64.0, 0.0);
        let threat = DVec3::new(0.0, 64.0, 0.0); // distance == 32.0 == SPIDER_RETREAT_RADIUS
        assert!(
            retreat_complete(spider, threat),
            "距离 >= 撤退半径时撤退完成"
        );
    }

    #[test]
    fn retreat_complete_too_close() {
        let spider = DVec3::new(10.0, 64.0, 0.0);
        let threat = DVec3::new(0.0, 64.0, 0.0); // distance == 10.0 < 32.0
        assert!(
            !retreat_complete(spider, threat),
            "距离 < 撤退半径时撤退未完成"
        );
    }

    // ── qi 吸收守恒系统测试 ────────────────────────────────────────────

    #[test]
    fn disguised_spider_absorbs_qi_from_zone() {
        let mut app = App::new();
        app.add_event::<DeathEvent>();
        app.insert_resource(WorldQiAccount::default());
        app.insert_resource(zone_registry_with_qi(0.8));
        app.add_systems(Update, spider_disguised_qi_absorb_system);

        // spawn zone bounds: min=[-128,64,-128], 位置 [0,64,0] 在其内
        let pos = DVec3::new(0.0, 64.0, 0.0);
        let spider = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([pos.x, pos.y, pos.z]),
                SpiderDisguiseState::Disguised,
                make_blackboard("spawn", pos),
            ))
            .id();

        app.update();

        let blackboard = app
            .world()
            .get::<MimicSpiderBlackboard>(spider)
            .expect("blackboard must exist after update");
        assert!(
            blackboard.drained_qi > 0.0,
            "Disguised 蛛一 tick 后 drained_qi 应 > 0，实际 {}（说明 qi 吸收系统未执行）",
            blackboard.drained_qi
        );
    }

    #[test]
    fn ambush_spider_does_not_absorb_qi() {
        let mut app = App::new();
        app.add_event::<DeathEvent>();
        app.insert_resource(WorldQiAccount::default());
        app.insert_resource(zone_registry_with_qi(0.8));
        app.add_systems(Update, spider_disguised_qi_absorb_system);

        let pos = DVec3::new(0.0, 64.0, 0.0);
        let spider = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([pos.x, pos.y, pos.z]),
                SpiderDisguiseState::Ambush, // 暴起中，不吸收
                make_blackboard("spawn", pos),
            ))
            .id();

        app.update();

        let blackboard = app.world().get::<MimicSpiderBlackboard>(spider).unwrap();
        assert_eq!(
            blackboard.drained_qi, 0.0,
            "Ambush 状态下不应吸收 qi，期望 drained_qi == 0"
        );
    }

    #[test]
    fn retreat_spider_does_not_absorb_qi() {
        let mut app = App::new();
        app.add_event::<DeathEvent>();
        app.insert_resource(WorldQiAccount::default());
        app.insert_resource(zone_registry_with_qi(0.8));
        app.add_systems(Update, spider_disguised_qi_absorb_system);

        let pos = DVec3::new(0.0, 64.0, 0.0);
        let spider = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([pos.x, pos.y, pos.z]),
                SpiderDisguiseState::Retreat,
                make_blackboard("spawn", pos),
            ))
            .id();

        app.update();

        let blackboard = app.world().get::<MimicSpiderBlackboard>(spider).unwrap();
        assert_eq!(
            blackboard.drained_qi, 0.0,
            "Retreat 状态下不应吸收 qi，期望 drained_qi == 0"
        );
    }

    #[test]
    fn disguised_spider_does_not_absorb_from_negative_zone() {
        // 负灵域 spirit_qi <= 0 时蛛不吸收（正吸收方向被守恒保护）
        let mut app = App::new();
        app.add_event::<DeathEvent>();
        app.insert_resource(WorldQiAccount::default());
        app.insert_resource(zone_registry_with_qi(-0.5)); // 负灵域
        app.add_systems(Update, spider_disguised_qi_absorb_system);

        let pos = DVec3::new(0.0, 64.0, 0.0);
        let spider = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([pos.x, pos.y, pos.z]),
                SpiderDisguiseState::Disguised,
                make_blackboard("spawn", pos),
            ))
            .id();

        app.update();

        let blackboard = app.world().get::<MimicSpiderBlackboard>(spider).unwrap();
        assert_eq!(
            blackboard.drained_qi, 0.0,
            "负灵域中 Disguised 蛛不应吸收 qi（zone spirit_qi <= 0）"
        );
    }

    #[test]
    fn spider_death_releases_qi_to_zone() {
        // 有 drained_qi 的蛛死亡后 zone spirit_qi 应有小幅回升
        let mut app = App::new();
        app.add_event::<DeathEvent>();
        app.insert_resource(WorldQiAccount::default());

        let initial_spirit_qi = 0.5_f64;
        app.insert_resource(zone_registry_with_qi(initial_spirit_qi));
        app.add_systems(Update, spider_release_qi_on_death_system);

        let pos = DVec3::new(0.0, 64.0, 0.0);
        let mut blackboard = make_blackboard("spawn", pos);
        blackboard.drained_qi = 100.0; // 已累计吸收 100 真元

        let spider = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([pos.x, pos.y, pos.z]),
                SpiderDisguiseState::Disguised,
                blackboard,
            ))
            .id();

        app.world_mut().send_event(DeathEvent {
            target: spider,
            cause: "test".to_string(),
            attacker: None,
            attacker_player_id: None,
            at_tick: 0,
        });
        app.update();

        let new_spirit_qi = app
            .world()
            .resource::<ZoneRegistry>()
            .find_zone_by_name("spawn")
            .expect("spawn zone must still exist")
            .spirit_qi;
        assert!(
            new_spirit_qi > initial_spirit_qi,
            "蛛死亡后 zone spirit_qi 应回升（期望 > {initial_spirit_qi}，实际 {new_spirit_qi}）"
        );
        // 验证 1% 策略：回升量 = 100 * 0.01 = 1.0，但 zone 上限 1.0，实际取 clamp
        let expected = (initial_spirit_qi + 100.0 * 0.01).clamp(-1.0, 1.0);
        assert!(
            (new_spirit_qi - expected).abs() < 1e-9,
            "死亡归还应 = drained_qi × 0.01 clamp(-1,1)，期望 {expected}，实际 {new_spirit_qi}"
        );
    }

    #[test]
    fn spider_death_with_zero_drained_qi_no_zone_change() {
        // drained_qi=0 时死亡不影响 zone（避免零值写入影响守恒审计）
        let mut app = App::new();
        app.add_event::<DeathEvent>();
        app.insert_resource(WorldQiAccount::default());

        let initial_spirit_qi = 0.6_f64;
        app.insert_resource(zone_registry_with_qi(initial_spirit_qi));
        app.add_systems(Update, spider_release_qi_on_death_system);

        let pos = DVec3::new(0.0, 64.0, 0.0);
        let spider = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([pos.x, pos.y, pos.z]),
                SpiderDisguiseState::Disguised,
                make_blackboard("spawn", pos), // drained_qi == 0
            ))
            .id();

        app.world_mut().send_event(DeathEvent {
            target: spider,
            cause: "test".to_string(),
            attacker: None,
            attacker_player_id: None,
            at_tick: 0,
        });
        app.update();

        let new_spirit_qi = app
            .world()
            .resource::<ZoneRegistry>()
            .find_zone_by_name("spawn")
            .expect("spawn zone must still exist")
            .spirit_qi;
        assert_eq!(
            new_spirit_qi, initial_spirit_qi,
            "drained_qi=0 的蛛死亡不应改变 zone spirit_qi"
        );
    }

    #[test]
    fn blackboard_new_trapped_by_is_none() {
        let b = MimicSpiderBlackboard::new("spawn", DVec3::ZERO);
        assert!(
            b.trapped_by.is_none(),
            "新建 blackboard trapped_by 应为 None（野生蛛不属于任何陷阱主）"
        );
    }

    // ── P3 SpiderTrapPotential + 超时系统测试 ────────────────────────────────

    #[test]
    fn spider_cage_template_id_pin() {
        // wire 名稳定性：fauna.toml id = "spider_cage" 必须与常数一致
        assert_eq!(
            SPIDER_CAGE_TEMPLATE_ID, "spider_cage",
            "SPIDER_CAGE_TEMPLATE_ID 应与 fauna.toml 中 item id 一致（实际 {SPIDER_CAGE_TEMPLATE_ID}）"
        );
    }

    #[test]
    fn spider_trap_timeout_ticks_pin() {
        // 72 游戏内天 = 72 × 24_000 = 1_728_000 tick
        assert_eq!(
            SPIDER_TRAP_TIMEOUT_TICKS,
            72 * 24_000,
            "陷阱超时应为 72 游戏天（期望 {}，实际 {SPIDER_TRAP_TIMEOUT_TICKS}）",
            72 * 24_000
        );
    }

    #[test]
    fn trap_potential_component_stores_owner_and_tick() {
        // SpiderTrapPotential 存储正确的归属和时间戳
        let mut app = App::new();
        let owner = app.world_mut().spawn_empty().id();
        let placed_pos = DVec3::new(10.0, 64.0, 20.0);
        let placed_tick = 5000_u64;

        let spider = app
            .world_mut()
            .spawn(SpiderTrapPotential {
                trap_owner: owner,
                placed_at: placed_pos,
                placed_tick,
            })
            .id();

        let trap = app
            .world()
            .get::<SpiderTrapPotential>(spider)
            .expect("SpiderTrapPotential should be present");
        assert_eq!(trap.trap_owner, owner, "trap_owner 应与放置者 Entity 一致");
        assert_eq!(
            trap.placed_tick, placed_tick,
            "placed_tick 应与放置时刻一致"
        );
        assert_eq!(trap.placed_at, placed_pos, "placed_at 应与放置坐标一致");
    }

    #[test]
    fn trap_timeout_system_releases_spider_after_timeout() {
        // current_tick >= placed_tick + TIMEOUT → SpiderTrapPotential 被移除，状态变 Retreat
        let mut app = App::new();
        app.add_event::<DeathEvent>();
        app.insert_resource(WorldQiAccount::default());
        app.insert_resource(CultivationClock {
            tick: SPIDER_TRAP_TIMEOUT_TICKS + 100,
        });
        app.add_systems(Update, spider_trap_timeout_system);

        let owner = app.world_mut().spawn_empty().id();
        let pos = DVec3::new(0.0, 64.0, 0.0);
        let mut blackboard = make_blackboard("spawn", pos);
        blackboard.trapped_by = Some(owner);

        let spider = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([pos.x, pos.y, pos.z]),
                SpiderDisguiseState::Disguised,
                blackboard,
                SpiderTrapPotential {
                    trap_owner: owner,
                    placed_at: pos,
                    placed_tick: 0, // placed at tick 0，current = TIMEOUT+100 → 超时
                },
            ))
            .id();

        app.update();

        // SpiderTrapPotential 应被移除
        assert!(
            app.world().get::<SpiderTrapPotential>(spider).is_none(),
            "超时后 SpiderTrapPotential 应被移除（蛛已释放）"
        );

        // 状态应变 Retreat
        let state = app
            .world()
            .get::<SpiderDisguiseState>(spider)
            .expect("SpiderDisguiseState must still exist");
        assert_eq!(
            *state,
            SpiderDisguiseState::Retreat,
            "超时后蛛状态应变 Retreat（期望 Retreat，实际 {state:?}）"
        );

        // blackboard.trapped_by 应被清空
        let bb = app
            .world()
            .get::<MimicSpiderBlackboard>(spider)
            .expect("blackboard must still exist");
        assert!(
            bb.trapped_by.is_none(),
            "超时后 blackboard.trapped_by 应为 None（期望 None，说明归属已解除）"
        );
    }

    #[test]
    fn trap_timeout_system_does_not_release_before_timeout() {
        // current_tick < placed_tick + TIMEOUT → 不释放
        let mut app = App::new();
        app.add_event::<DeathEvent>();
        app.insert_resource(WorldQiAccount::default());
        // 设时钟为超时前一刻
        app.insert_resource(CultivationClock {
            tick: SPIDER_TRAP_TIMEOUT_TICKS - 1,
        });
        app.add_systems(Update, spider_trap_timeout_system);

        let owner = app.world_mut().spawn_empty().id();
        let pos = DVec3::new(0.0, 64.0, 0.0);
        let mut blackboard = make_blackboard("spawn", pos);
        blackboard.trapped_by = Some(owner);

        let spider = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([pos.x, pos.y, pos.z]),
                SpiderDisguiseState::Disguised,
                blackboard,
                SpiderTrapPotential {
                    trap_owner: owner,
                    placed_at: pos,
                    placed_tick: 0, // placed_tick=0, current=TIMEOUT-1 → 未超时
                },
            ))
            .id();

        app.update();

        // SpiderTrapPotential 应还在
        assert!(
            app.world().get::<SpiderTrapPotential>(spider).is_some(),
            "未超时时 SpiderTrapPotential 不应被移除（蛛仍受控）"
        );

        // 状态应保持 Disguised
        let state = app.world().get::<SpiderDisguiseState>(spider).unwrap();
        assert_eq!(
            *state,
            SpiderDisguiseState::Disguised,
            "未超时时蛛应保持 Disguised（实际 {state:?}）"
        );
    }

    #[test]
    fn trap_timeout_system_exact_boundary_releases() {
        // current_tick == placed_tick + TIMEOUT → 恰好超时（边界 off-by-one：>=）
        let mut app = App::new();
        app.add_event::<DeathEvent>();
        app.insert_resource(WorldQiAccount::default());
        app.insert_resource(CultivationClock {
            tick: SPIDER_TRAP_TIMEOUT_TICKS, // 恰好等于超时
        });
        app.add_systems(Update, spider_trap_timeout_system);

        let owner = app.world_mut().spawn_empty().id();
        let pos = DVec3::new(0.0, 64.0, 0.0);
        let mut blackboard = make_blackboard("spawn", pos);
        blackboard.trapped_by = Some(owner);

        let spider = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([pos.x, pos.y, pos.z]),
                SpiderDisguiseState::Disguised,
                blackboard,
                SpiderTrapPotential {
                    trap_owner: owner,
                    placed_at: pos,
                    placed_tick: 0,
                },
            ))
            .id();

        app.update();

        // 恰好等于超时边界，应释放
        assert!(
            app.world().get::<SpiderTrapPotential>(spider).is_none(),
            "恰好到达超时边界时应释放（期望 SpiderTrapPotential=None，current=TIMEOUT，placed=0）"
        );
        let state = app.world().get::<SpiderDisguiseState>(spider).unwrap();
        assert_eq!(
            *state,
            SpiderDisguiseState::Retreat,
            "边界超时应切换到 Retreat（实际 {state:?}）"
        );
    }

    #[test]
    fn wild_spider_not_affected_by_trap_timeout_system() {
        // 没有 SpiderTrapPotential 的野生蛛，timeout 系统不影响其状态
        let mut app = App::new();
        app.add_event::<DeathEvent>();
        app.insert_resource(WorldQiAccount::default());
        app.insert_resource(CultivationClock {
            tick: SPIDER_TRAP_TIMEOUT_TICKS * 10, // 极大 tick，野生蛛不应受影响
        });
        app.add_systems(Update, spider_trap_timeout_system);

        let pos = DVec3::new(0.0, 64.0, 0.0);
        let spider = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([pos.x, pos.y, pos.z]),
                SpiderDisguiseState::Disguised,
                make_blackboard("spawn", pos),
                // 无 SpiderTrapPotential
            ))
            .id();

        app.update();

        // 状态应保持 Disguised
        let state = app.world().get::<SpiderDisguiseState>(spider).unwrap();
        assert_eq!(
            *state,
            SpiderDisguiseState::Disguised,
            "野生蛛（无 SpiderTrapPotential）不应被 timeout 系统修改（实际 {state:?}）"
        );
    }

    #[test]
    fn trap_potential_different_owners_are_independent() {
        // 两只蛛各有不同 owner，超时后各自独立释放，不互相干扰
        let mut app = App::new();
        app.add_event::<DeathEvent>();
        app.insert_resource(WorldQiAccount::default());
        // 只让 spider_a 超时（placed_tick=0, timeout），spider_b 放置在未来（placed_tick=TIMEOUT+100）
        app.insert_resource(CultivationClock {
            tick: SPIDER_TRAP_TIMEOUT_TICKS + 50,
        });
        app.add_systems(Update, spider_trap_timeout_system);

        let owner_a = app.world_mut().spawn_empty().id();
        let owner_b = app.world_mut().spawn_empty().id();
        let pos = DVec3::new(0.0, 64.0, 0.0);

        let mut bb_a = make_blackboard("spawn", pos);
        bb_a.trapped_by = Some(owner_a);
        let spider_a = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([pos.x, pos.y, pos.z]),
                SpiderDisguiseState::Disguised,
                bb_a,
                SpiderTrapPotential {
                    trap_owner: owner_a,
                    placed_at: pos,
                    placed_tick: 0, // 超时
                },
            ))
            .id();

        let mut bb_b = make_blackboard("spawn", pos);
        bb_b.trapped_by = Some(owner_b);
        let spider_b = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([pos.x, pos.y, pos.z]),
                SpiderDisguiseState::Disguised,
                bb_b,
                SpiderTrapPotential {
                    trap_owner: owner_b,
                    placed_at: pos,
                    // 放置在 TIMEOUT + 100 tick 时，current = TIMEOUT + 50 → 未超时
                    placed_tick: SPIDER_TRAP_TIMEOUT_TICKS + 100,
                },
            ))
            .id();

        app.update();

        // spider_a 应超时释放
        assert!(
            app.world().get::<SpiderTrapPotential>(spider_a).is_none(),
            "spider_a 应超时后释放（placed=0, current=TIMEOUT+50）"
        );
        assert_eq!(
            *app.world().get::<SpiderDisguiseState>(spider_a).unwrap(),
            SpiderDisguiseState::Retreat,
            "spider_a 超时后应 Retreat"
        );

        // spider_b 应保持 Disguised（未超时）
        assert!(
            app.world().get::<SpiderTrapPotential>(spider_b).is_some(),
            "spider_b 未超时不应释放（placed=TIMEOUT+100, current=TIMEOUT+50）"
        );
        assert_eq!(
            *app.world().get::<SpiderDisguiseState>(spider_b).unwrap(),
            SpiderDisguiseState::Disguised,
            "spider_b 未超时应保持 Disguised"
        );
    }
}
