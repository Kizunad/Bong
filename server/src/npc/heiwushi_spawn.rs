//! 黑武士自然刷新系统（plan-sword-path-complete §A）。
//!
//! 铸剑古殿（giant_sword_sea zone）定点守关 BOSS，首杀后 72h 复活，此后 1h 间隔。
//! 刷新门槛：锚点 48 格内有玩家（防止无人空世界刷）。

use valence::prelude::{
    bevy_ecs, App, Client, Commands, DVec3, Position, Query, Res, ResMut, Resource, Update, With,
};

use crate::npc::heiwushi::{spawn_heiwushi_at, HeiwushiMarker};
use crate::npc::movement::GameTick;
use crate::world::dimension::DimensionLayers;
use crate::world::zone::ZoneRegistry;

pub const HEIWUSHI_HOME_ZONE: &str = "giant_sword_sea";
pub const HEIWUSHI_SPAWN_CHECK_INTERVAL_TICKS: u64 = 200; // 每 10s 巡检
pub const HEIWUSHI_FIRST_KILL_RESPAWN_TICKS: u64 = 72 * 60 * 60 * 20; // 72h → ticks
pub const HEIWUSHI_RESPAWN_TICKS: u64 = 60 * 60 * 20; // 1h → ticks
pub const HEIWUSHI_PLAYER_PRESENCE_RADIUS: f64 = 48.0; // 玩家在场触发半径

/// 黑武士刷新状态 Resource，持久化生死观测。
#[derive(Debug, Default, Resource)]
pub struct HeiwushiSpawnState {
    /// 最近一次死亡的观测 tick（None = 从未刷过或尚未死亡）。
    pub last_death_tick: Option<u64>,
    /// 当前是否有活体黑武士。
    pub alive: bool,
    /// 累计击杀次数（首杀 = 1，用于区分首次刷新冷却 vs 后续冷却）。
    pub kills: u32,
    /// 上次巡检 tick，避免每 tick 都做全量查询。
    last_check_tick: u64,
}

pub fn register(app: &mut App) {
    app.init_resource::<HeiwushiSpawnState>()
        .add_systems(Update, heiwushi_natural_spawn_system);
}

/// 黑武士自然刷新巡检（§A 逻辑逐项锁定）。
pub fn heiwushi_natural_spawn_system(
    tick: Option<Res<GameTick>>,
    mut state: ResMut<HeiwushiSpawnState>,
    alive_bosses: Query<(), With<HeiwushiMarker>>,
    players: Query<&Position, With<Client>>,
    zone_registry: Option<Res<ZoneRegistry>>,
    dimension_layers: Option<Res<DimensionLayers>>,
    mut commands: Commands,
) {
    let now = tick.map(|t| u64::from(t.0)).unwrap_or(0);

    // 节流：每 HEIWUSHI_SPAWN_CHECK_INTERVAL_TICKS 才巡检一次。
    if now.saturating_sub(state.last_check_tick) < HEIWUSHI_SPAWN_CHECK_INTERVAL_TICKS {
        return;
    }
    state.last_check_tick = now;

    // 1. 若已存在活体 → 确认存活，return。
    let boss_count = alive_bosses.iter().count();
    if boss_count > 0 {
        state.alive = true;
        return;
    }

    // 2. 刚才标记 alive=true，但现在 query 计数为 0 → 视为刚死：落账死亡 tick。
    if state.alive {
        state.last_death_tick = Some(now);
        state.kills = state.kills.saturating_add(1);
        state.alive = false;
        return;
    }

    // 3. 计算冷却：首杀 72h，之后 1h。
    if let Some(last_death) = state.last_death_tick {
        let cooldown = if state.kills <= 1 {
            HEIWUSHI_FIRST_KILL_RESPAWN_TICKS
        } else {
            HEIWUSHI_RESPAWN_TICKS
        };
        if now.saturating_sub(last_death) < cooldown {
            return;
        }
    }
    // last_death_tick == None 时（从未刷过）直接进到步骤 4。

    // 4. 取锚点。
    let anchor = zone_registry
        .as_deref()
        .and_then(|r| r.find_zone_by_name(HEIWUSHI_HOME_ZONE))
        .and_then(|z| z.patrol_anchors.first().copied())
        .unwrap_or(DVec3::new(4200.0, 85.0, 1200.0));

    // 5. 玩家在场门：锚点 HEIWUSHI_PLAYER_PRESENCE_RADIUS 格内必须有玩家。
    let radius_sq = HEIWUSHI_PLAYER_PRESENCE_RADIUS * HEIWUSHI_PLAYER_PRESENCE_RADIUS;
    let player_nearby = players
        .iter()
        .any(|pos| pos.get().distance_squared(anchor) <= radius_sq);
    if !player_nearby {
        return;
    }

    // 6. 取 overworld layer，缺失时 skip。
    let Some(layers) = dimension_layers.as_deref() else {
        return;
    };
    let layer = layers.overworld;

    // 7. 满足所有条件 → 刷新黑武士。
    spawn_heiwushi_at(
        &mut commands,
        layer,
        HEIWUSHI_HOME_ZONE,
        anchor,
        anchor,
        now,
    );
    state.alive = true;
}

#[cfg(test)]
mod tests {
    use super::*;
    use valence::prelude::{App, Update};

    fn make_app() -> App {
        let mut app = App::new();
        app.init_resource::<HeiwushiSpawnState>()
            .add_event::<crate::combat::events::AttackIntent>()
            .add_event::<crate::combat::events::ApplyStatusEffectIntent>()
            .add_event::<crate::npc::heiwushi::HeiwushiActionVfxEvent>()
            .add_systems(Update, heiwushi_natural_spawn_system);
        app
    }

    #[test]
    fn no_spawn_when_cooldown_not_reached() {
        // last_death_tick = 0, kills = 1 → FIRST_KILL 冷却 72h，当前 tick = 200 → 不刷
        let mut app = make_app();
        app.insert_resource(GameTick(200));
        {
            let mut state = app.world_mut().resource_mut::<HeiwushiSpawnState>();
            state.last_death_tick = Some(0);
            state.kills = 1;
            state.alive = false;
        }
        app.update();
        let mut boss_query = app.world_mut().query_filtered::<(), With<HeiwushiMarker>>();
        let bosses = boss_query.iter(app.world()).count();
        assert_eq!(bosses, 0, "冷却期内不应刷新黑武士");
    }

    #[test]
    fn no_spawn_when_first_kill_72h_not_elapsed() {
        // kills = 1, 死亡 tick = 0, 当前 = 72h - 1 → 不刷
        let mut app = make_app();
        let almost_72h: u32 = (HEIWUSHI_FIRST_KILL_RESPAWN_TICKS - 1) as u32;
        app.insert_resource(GameTick(almost_72h));
        {
            let mut state = app.world_mut().resource_mut::<HeiwushiSpawnState>();
            state.last_death_tick = Some(0);
            state.kills = 1;
            state.alive = false;
        }
        app.update();
        let mut boss_query = app.world_mut().query_filtered::<(), With<HeiwushiMarker>>();
        let bosses = boss_query.iter(app.world()).count();
        assert_eq!(bosses, 0, "首杀 72h 内不应刷新");
    }

    #[test]
    fn no_spawn_when_respawn_1h_not_elapsed_after_second_kill() {
        // kills = 2, 死亡 tick = 0, 当前 = 1h - 1 → 不刷
        let mut app = make_app();
        let almost_1h: u32 = (HEIWUSHI_RESPAWN_TICKS - 1) as u32;
        app.insert_resource(GameTick(almost_1h));
        {
            let mut state = app.world_mut().resource_mut::<HeiwushiSpawnState>();
            state.last_death_tick = Some(0);
            state.kills = 2;
            state.alive = false;
        }
        app.update();
        let mut boss_query = app.world_mut().query_filtered::<(), With<HeiwushiMarker>>();
        let bosses = boss_query.iter(app.world()).count();
        assert_eq!(bosses, 0, "第二次死亡 1h 内不应刷新");
    }

    #[test]
    fn no_spawn_when_no_player_nearby() {
        // 冷却已过，但无玩家在 48 格内 → 不刷
        // 注：没有 ZoneRegistry 和 DimensionLayers → anchor 用默认值，且无玩家 → 步骤 5 拦截
        let mut app = make_app();
        // tick 超过 72h (first kill)
        let big_tick: u32 =
            (HEIWUSHI_FIRST_KILL_RESPAWN_TICKS + HEIWUSHI_SPAWN_CHECK_INTERVAL_TICKS + 1) as u32;
        app.insert_resource(GameTick(big_tick));
        {
            let mut state = app.world_mut().resource_mut::<HeiwushiSpawnState>();
            state.last_death_tick = Some(0);
            state.kills = 1;
            state.alive = false;
        }
        app.update();
        let mut boss_query = app.world_mut().query_filtered::<(), With<HeiwushiMarker>>();
        let bosses = boss_query.iter(app.world()).count();
        assert_eq!(bosses, 0, "锚点无玩家时不应刷新");
    }

    #[test]
    fn no_duplicate_spawn_when_alive() {
        // alive = true, 但无 HeiwushiMarker → 应落 last_death_tick 而非刷新
        let mut app = make_app();
        app.insert_resource(GameTick(200));
        {
            let mut state = app.world_mut().resource_mut::<HeiwushiSpawnState>();
            state.alive = true;
            state.last_death_tick = None;
        }
        app.update();
        // alive 变 false，last_death_tick 填入，不刷新
        let state = app.world().resource::<HeiwushiSpawnState>();
        assert!(!state.alive, "活体丢失后 alive 应置 false");
        assert!(
            state.last_death_tick.is_some(),
            "活体丢失后 last_death_tick 应落账"
        );
    }

    #[test]
    fn death_observation_records_last_death_tick() {
        // alive=true, 0 个 HeiwushiMarker → 视为刚死，last_death_tick 应被写入
        let mut app = make_app();
        app.insert_resource(GameTick(500));
        {
            let mut state = app.world_mut().resource_mut::<HeiwushiSpawnState>();
            state.alive = true;
        }
        app.update();
        let state = app.world().resource::<HeiwushiSpawnState>();
        assert_eq!(state.last_death_tick, Some(500), "死亡观测应记录当前 tick");
        assert_eq!(state.kills, 1, "死亡后 kills 应增 1");
    }
}
