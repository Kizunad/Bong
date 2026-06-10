//! plan-territory-v1 P3 — NPC 传话系统：匿名境界段 + 1-3hr 延迟 + 相邻 zone gate。
//!
//! ## 数据流
//! 1. `territory_rumor_enqueue_system` 读 DominanceChangedEvent(Established) +
//!    zone.public_known==true → 入队 PendingZoneRumors（附 deliver_at_tick = now + 1-3hr）。
//! 2. `territory_rumor_spread_system` 到期后写入 home_zone 匹配（或相邻 zone）的
//!    NpcPatrol NPC 的 NpcRumorMemory.rumors（可观测状态）。
//!
//! ## 匿名约束
//! ZoneDominionRumor **不含任何 char_id / 玩家名**，只携带 zone + realm_band + tick。
//! 由编译期结构保证（struct 无玩家标识字段）。
//!
//! ## 范围声明
//! - NPC 传话主体为「任何带 NpcPatrol 的 NPC」（非散修 ScatteredCultivator，理由见蓝图）。
//! - 跨 zone 扩散：1 跳，相邻 zone gate（zones_are_adjacent margin=150）。
//! - 环境痕迹 client 视觉（GroundDecal/Sprite）DEFER 到独立 client 资产 PR。

use valence::prelude::{
    bevy_ecs, App, Component, Entity, EventReader, Position, Query, Res, ResMut, Resource, Update,
    With,
};

use crate::cultivation::components::Realm;
use crate::npc::patrol::NpcPatrol;
use crate::world::territory_narration::DominanceChangedEvent;
use crate::world::territory_narration::DominanceEventKind;
use crate::world::zone::ZoneRegistry;
use crate::{cultivation::tick::CultivationClock, world::territory::ZoneInfluenceMap};

// ─── 常量 ─────────────────────────────────────────────────────────────────────

/// 1 小时游戏内时间对应的 tick 数（1hr = 72000 ticks）。
pub const TICKS_PER_GAME_HOUR: u64 = 72_000;

/// NPC 传话最短延迟（1hr game time）。
pub const RUMOR_DELIVER_MIN_TICKS: u64 = TICKS_PER_GAME_HOUR; // 72_000

/// NPC 传话最长延迟（3hr game time）。
pub const RUMOR_DELIVER_MAX_TICKS: u64 = 3 * TICKS_PER_GAME_HOUR; // 216_000

/// zones_are_adjacent 判定用 margin（格）。
pub const ADJACENT_ZONE_MARGIN: f64 = 150.0;

/// 相邻扩散 eval 节流间隔（避免每 tick 都遍历）。
pub const RUMOR_SPREAD_EVAL_TICKS: u64 = 600; // ~30s

/// rumor 自然过期 TTL（10hr game time = 720_000 ticks）。
pub const RUMOR_TTL_TICKS: u64 = 10 * TICKS_PER_GAME_HOUR;

// ─── 境界段 ────────────────────────────────────────────────────────────────────

/// 匿名境界段（Low / Mid / High）。
///
/// 映射：Awaken/Induce → Low, Condense/Solidify → Mid, Spirit/Void → High。
/// **不传 char_id / 玩家名**，只传境界段（匿名约束）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RealmBand {
    Low,
    Mid,
    High,
}

impl RealmBand {
    /// 从 Realm 映射到匿名段位。
    pub fn from_realm(realm: Realm) -> Self {
        match realm {
            Realm::Awaken | Realm::Induce => RealmBand::Low,
            Realm::Condense | Realm::Solidify => RealmBand::Mid,
            Realm::Spirit | Realm::Void => RealmBand::High,
        }
    }

    /// 段位描述文字（用于 narration 模板，仅段位词不含玩家名）。
    #[allow(dead_code)]
    pub fn as_str(self) -> &'static str {
        match self {
            RealmBand::Low => "低段",
            RealmBand::Mid => "中段",
            RealmBand::High => "高段",
        }
    }
}

// ─── 数据结构 ─────────────────────────────────────────────────────────────────

/// 匿名领地 rumor 条目。
///
/// **结构层面不含任何 char_id / 玩家名**，编译期保证匿名。
#[derive(Debug, Clone, PartialEq)]
pub struct ZoneDominionRumor {
    /// 事件所在 zone 名称。
    pub zone_name: String,
    /// 匿名境界段（Low / Mid / High）。
    pub realm_band: RealmBand,
    /// 事件发生的游戏 tick（「大致时间」，匿名化不精确）。
    pub observed_at_tick: u64,
    /// 预计传达 tick（到期后 spread system 写入 NPC memory）。
    pub deliver_at_tick: u64,
}

/// NPC rumor 记忆组件。
///
/// 挂在有 NpcPatrol 的 NPC entity 上。
/// `upsert` 同 zone 取最新（覆盖旧条目），`prune_expired` 清理过期 rumor。
#[derive(Debug, Default, Component)]
pub struct NpcRumorMemory {
    pub rumors: Vec<ZoneDominionRumor>,
}

impl NpcRumorMemory {
    /// 写入或更新 rumor（同 zone_name 的旧条目被覆盖，只保留最新）。
    pub fn upsert(&mut self, rumor: ZoneDominionRumor) {
        if let Some(existing) = self
            .rumors
            .iter_mut()
            .find(|r| r.zone_name == rumor.zone_name)
        {
            // 新 rumor observed_at_tick 更新才覆盖
            if rumor.observed_at_tick >= existing.observed_at_tick {
                *existing = rumor;
            }
        } else {
            self.rumors.push(rumor);
        }
    }

    /// 清理已过期 rumor（observed_at_tick + TTL < now）。
    pub fn prune_expired(&mut self, now_tick: u64) {
        self.rumors
            .retain(|r| r.observed_at_tick.saturating_add(RUMOR_TTL_TICKS) >= now_tick);
    }
}

/// 待传播 rumor 队列（Resource）。
///
/// `territory_rumor_enqueue_system` 往这里推入，
/// `territory_rumor_spread_system` 到期后从这里取出并写入 NPC memory。
#[derive(Debug, Default)]
pub struct PendingZoneRumors {
    pub queue: Vec<ZoneDominionRumor>,
}

impl Resource for PendingZoneRumors {}

// ─── 节流状态 ─────────────────────────────────────────────────────────────────

#[derive(Debug, Default)]
pub struct RumorSpreadThrottle {
    pub last_eval_tick: u64,
}

impl Resource for RumorSpreadThrottle {}

// ─── 伪随机延迟 ───────────────────────────────────────────────────────────────

/// 伪随机延迟计算（无 rand crate）。
///
/// 用 entity bits XOR now_tick 生成 0..range 范围的偏移量（复用 gossip.rs 同模式）。
pub fn pseudo_rand_deliver_offset(entity: Entity, now_tick: u64) -> u64 {
    let bits = entity.to_bits();
    let range = RUMOR_DELIVER_MAX_TICKS - RUMOR_DELIVER_MIN_TICKS;
    RUMOR_DELIVER_MIN_TICKS + (bits ^ now_tick) % range.max(1)
}

// ─── System: 入队 ─────────────────────────────────────────────────────────────

type EnqueueSystemNpcQuery<'w, 's> = Query<'w, 's, (Entity, &'static NpcPatrol), With<NpcPatrol>>;

/// 读 DominanceChangedEvent(Established + public_known=true) → 推 PendingZoneRumors。
///
/// - 只处理 Established 类型（新霸主确立才传话）。
/// - 只处理 public_known=true 的 zone（传话延迟 gate）。
/// - deliver_at_tick = now + pseudo_rand(1-3hr)。
/// - **不含 char_id**，只取 realm_band + zone_name。
pub fn territory_rumor_enqueue_system(
    mut ev_reader: EventReader<DominanceChangedEvent>,
    mut pending: ResMut<PendingZoneRumors>,
    influence_map: Option<Res<ZoneInfluenceMap>>,
    clock: Option<Res<CultivationClock>>,
    npc_query: EnqueueSystemNpcQuery<'_, '_>,
) {
    let Some(clock) = clock else {
        return;
    };
    let now_tick = clock.tick;

    for ev in ev_reader.read() {
        // 只处理新确立（Established）
        if ev.kind != DominanceEventKind::Established {
            continue;
        }

        // gate: public_known 必须为 true
        let is_public = influence_map
            .as_deref()
            .and_then(|m| m.zones.get(&ev.zone_name))
            .and_then(|e| e.dominant.as_ref())
            .map(|d| d.public_known)
            .unwrap_or(false);

        if !is_public {
            continue;
        }

        // 取任意 NPC entity 做伪随机种子（无 rand crate）
        // Entity::from_raw(1) 作为无 NPC 时的伪随机种子（合法 entity，generation=1）
        let seed_entity = npc_query
            .iter()
            .next()
            .map(|(e, _)| e)
            .unwrap_or(Entity::from_raw(1));
        let deliver_at_tick = now_tick + pseudo_rand_deliver_offset(seed_entity, now_tick);

        let rumor = ZoneDominionRumor {
            zone_name: ev.zone_name.clone(),
            realm_band: ev.realm_band,
            observed_at_tick: now_tick,
            deliver_at_tick,
        };

        // 防重复：同 zone_name 只保留最新（覆盖旧的）
        if let Some(existing) = pending
            .queue
            .iter_mut()
            .find(|r| r.zone_name == rumor.zone_name)
        {
            if rumor.observed_at_tick >= existing.observed_at_tick {
                *existing = rumor;
            }
        } else {
            pending.queue.push(rumor);
        }
    }
}

// ─── System: 传播 ─────────────────────────────────────────────────────────────

type SpreadSystemNpcQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static NpcPatrol,
        &'static Position,
        &'static mut NpcRumorMemory,
    ),
    With<NpcPatrol>,
>;

/// 到期 rumor 写入 home_zone 匹配（及相邻 zone）的 NPC NpcRumorMemory。
///
/// - 节流：每 RUMOR_SPREAD_EVAL_TICKS 执行一次（~30s）。
/// - 相邻判定：zones_are_adjacent(ADJACENT_ZONE_MARGIN=150)。
/// - 过期清理：每次执行前调 prune_expired。
/// - **1-跳扩散**：持有 rumor 的 NPC 若 home_zone 不在 rumor.zone，且 rumor.zone 与
///   NPC home_zone 相邻，则也接收 rumor（跨 zone 1 跳传播）。
pub fn territory_rumor_spread_system(
    clock: Option<Res<CultivationClock>>,
    zones: Option<Res<ZoneRegistry>>,
    mut throttle: ResMut<RumorSpreadThrottle>,
    mut pending: ResMut<PendingZoneRumors>,
    mut npc_query: SpreadSystemNpcQuery<'_, '_>,
) {
    let Some(clock) = clock else {
        return;
    };
    let now_tick = clock.tick;

    // 节流
    if now_tick < throttle.last_eval_tick + RUMOR_SPREAD_EVAL_TICKS && throttle.last_eval_tick > 0 {
        return;
    }
    throttle.last_eval_tick = now_tick;

    // 分离 ready（到期）和 not_ready（未到期）
    let (ready, not_ready): (Vec<_>, Vec<_>) = std::mem::take(&mut pending.queue)
        .into_iter()
        .partition(|r| now_tick >= r.deliver_at_tick);

    // 未到期的放回
    pending.queue = not_ready;

    if ready.is_empty() {
        // 即使没有到期 rumor，也清理 NPC memory 中过期条目
        for (_, _, _, mut memory) in npc_query.iter_mut() {
            memory.prune_expired(now_tick);
        }
        return;
    }

    // 对每个到期 rumor，找匹配 NPC
    for rumor in ready {
        for (_, patrol, _pos, mut memory) in npc_query.iter_mut() {
            let home = &patrol.home_zone;
            // 1. home_zone 直接匹配
            // 2. home_zone 与 rumor.zone_name 相邻（1 跳）
            let should_receive = home == &rumor.zone_name
                || zones
                    .as_deref()
                    .map(|z| z.zones_are_adjacent(home, &rumor.zone_name, ADJACENT_ZONE_MARGIN))
                    .unwrap_or(false);

            if should_receive {
                memory.upsert(rumor.clone());
            }
        }
    }

    // 清理过期条目
    for (_, _, _, mut memory) in npc_query.iter_mut() {
        memory.prune_expired(now_tick);
    }
}

/// 注册 territory rumor 系统和资源到 Bevy App。
pub fn register(app: &mut App) {
    app.init_resource::<PendingZoneRumors>();
    app.init_resource::<RumorSpreadThrottle>();
    app.add_systems(Update, territory_rumor_enqueue_system);
    app.add_systems(Update, territory_rumor_spread_system);
}

// ─── 单测 ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod rumor_tests {
    use super::*;

    // ── 1. RealmBand 映射 ────────────────────────────────────────────────────

    #[test]
    fn realm_band_low_awaken() {
        assert_eq!(
            RealmBand::from_realm(Realm::Awaken),
            RealmBand::Low,
            "Awaken → Low"
        );
    }

    #[test]
    fn realm_band_low_induce() {
        assert_eq!(
            RealmBand::from_realm(Realm::Induce),
            RealmBand::Low,
            "Induce → Low"
        );
    }

    #[test]
    fn realm_band_mid_condense() {
        assert_eq!(
            RealmBand::from_realm(Realm::Condense),
            RealmBand::Mid,
            "Condense → Mid"
        );
    }

    #[test]
    fn realm_band_mid_solidify() {
        assert_eq!(
            RealmBand::from_realm(Realm::Solidify),
            RealmBand::Mid,
            "Solidify → Mid"
        );
    }

    #[test]
    fn realm_band_high_spirit() {
        assert_eq!(
            RealmBand::from_realm(Realm::Spirit),
            RealmBand::High,
            "Spirit → High"
        );
    }

    #[test]
    fn realm_band_high_void() {
        assert_eq!(
            RealmBand::from_realm(Realm::Void),
            RealmBand::High,
            "Void → High"
        );
    }

    // ── 2. ZoneDominionRumor 无 char_id 字段（匿名约束，编译期）────────────

    #[test]
    fn zone_dominion_rumor_has_no_player_identity_field() {
        // 构造 rumor，断言字段只有 zone_name/realm_band/observed_at_tick/deliver_at_tick
        let rumor = ZoneDominionRumor {
            zone_name: "spawn".to_string(),
            realm_band: RealmBand::Mid,
            observed_at_tick: 1000,
            deliver_at_tick: 73000,
        };
        // 通过 format debug 确认无 char_id 字段（只要能编译就已由结构保证）
        let debug_str = format!("{rumor:?}");
        assert!(
            !debug_str.contains("char_id"),
            "ZoneDominionRumor 不应含 char_id 字段，actual debug={debug_str}"
        );
        assert!(
            !debug_str.contains("player"),
            "ZoneDominionRumor 不应含 player 字段，actual debug={debug_str}"
        );
        assert_eq!(rumor.zone_name, "spawn");
        assert_eq!(rumor.realm_band, RealmBand::Mid);
        assert_eq!(rumor.observed_at_tick, 1000);
        assert_eq!(rumor.deliver_at_tick, 73000);
    }

    // ── 3. NpcRumorMemory.upsert 同 zone 覆盖 ──────────────────────────────

    #[test]
    fn npc_rumor_memory_upsert_overwrites_same_zone() {
        let mut memory = NpcRumorMemory::default();
        memory.upsert(ZoneDominionRumor {
            zone_name: "spawn".to_string(),
            realm_band: RealmBand::Low,
            observed_at_tick: 100,
            deliver_at_tick: 72100,
        });
        assert_eq!(memory.rumors.len(), 1);

        // 更新（newer tick）
        memory.upsert(ZoneDominionRumor {
            zone_name: "spawn".to_string(),
            realm_band: RealmBand::High,
            observed_at_tick: 200,
            deliver_at_tick: 72200,
        });
        assert_eq!(memory.rumors.len(), 1, "同 zone 应覆盖旧条目，总数应为 1");
        assert_eq!(
            memory.rumors[0].realm_band,
            RealmBand::High,
            "覆盖后 realm_band 应为最新值 High"
        );
    }

    #[test]
    fn npc_rumor_memory_upsert_keeps_older_tick_if_not_newer() {
        let mut memory = NpcRumorMemory::default();
        memory.upsert(ZoneDominionRumor {
            zone_name: "spawn".to_string(),
            realm_band: RealmBand::Mid,
            observed_at_tick: 500,
            deliver_at_tick: 72500,
        });
        // 旧 tick 的 rumor 不应覆盖更新的
        memory.upsert(ZoneDominionRumor {
            zone_name: "spawn".to_string(),
            realm_band: RealmBand::Low,
            observed_at_tick: 100, // 旧 tick
            deliver_at_tick: 72100,
        });
        assert_eq!(
            memory.rumors[0].realm_band,
            RealmBand::Mid,
            "旧 tick rumor 不应覆盖更新的条目"
        );
    }

    // ── 4. NpcRumorMemory.prune_expired ────────────────────────────────────

    #[test]
    fn npc_rumor_memory_prune_expired_removes_old() {
        let mut memory = NpcRumorMemory::default();
        // 旧 rumor：observed_at=100，ttl=720_000 → 过期 at 720_100
        memory.upsert(ZoneDominionRumor {
            zone_name: "old_zone".to_string(),
            realm_band: RealmBand::Low,
            observed_at_tick: 100,
            deliver_at_tick: 72100,
        });
        // 新 rumor：observed_at=500，未过期
        memory.upsert(ZoneDominionRumor {
            zone_name: "new_zone".to_string(),
            realm_band: RealmBand::High,
            observed_at_tick: 800_000,
            deliver_at_tick: 872000,
        });

        // now=820_000，old_zone 的 observed+TTL=720_100 < 820_000 → 过期
        // new_zone 的 observed+TTL=800_000+720_000=1_520_000 > 820_000 → 未过期
        memory.prune_expired(820_000);

        assert_eq!(memory.rumors.len(), 1, "过期 rumor 应被清除");
        assert_eq!(
            memory.rumors[0].zone_name, "new_zone",
            "剩余 rumor 应为 new_zone"
        );
    }

    // ── 5. 延迟范围 1-3hr ────────────────────────────────────────────────────

    #[test]
    fn deliver_offset_in_range() {
        let entity = Entity::from_raw(42);
        let offset = pseudo_rand_deliver_offset(entity, 1000);
        assert!(
            offset >= RUMOR_DELIVER_MIN_TICKS,
            "deliver offset 应 >= {RUMOR_DELIVER_MIN_TICKS}, 实际={offset}"
        );
        assert!(
            offset < RUMOR_DELIVER_MAX_TICKS,
            "deliver offset 应 < {RUMOR_DELIVER_MAX_TICKS}, 实际={offset}"
        );
    }

    // ── 6. Bevy App 集成：enqueue → spread → NpcRumorMemory 写入 ──────────

    use crate::world::territory::{ZoneDominance, ZoneInfluenceMap};
    use crate::world::territory_narration::DominanceChangedEvent;
    use crate::world::territory_narration::DominanceEventKind;
    use valence::prelude::{App, DVec3, Update};

    fn make_spread_app_with_clock_and_zone(
        now_tick: u64,
        zone_name: &str,
        public_known: bool,
    ) -> App {
        use crate::world::zone::{Zone, ZoneRegistry};

        let mut app = App::new();
        app.add_event::<DominanceChangedEvent>();
        app.init_resource::<PendingZoneRumors>();
        app.init_resource::<RumorSpreadThrottle>();
        app.init_resource::<ZoneInfluenceMap>();

        let zone = Zone {
            name: zone_name.to_string(),
            dimension: crate::world::dimension::DimensionKind::Overworld,
            bounds: (DVec3::new(0.0, 60.0, 0.0), DVec3::new(200.0, 80.0, 200.0)),
            spirit_qi: 0.8,
            danger_level: 0,
            active_events: vec![],
            patrol_anchors: vec![DVec3::new(100.0, 66.0, 100.0)],
            blocked_tiles: vec![],
        };
        app.insert_resource(ZoneRegistry { zones: vec![zone] });
        app.insert_resource(CultivationClock { tick: now_tick });

        // 写入 influence map + public_known
        {
            let mut map = app.world_mut().resource_mut::<ZoneInfluenceMap>();
            let entry = map.zones.entry(zone_name.to_string()).or_default();
            entry.dominant = Some(ZoneDominance {
                char_id: "test_player".to_string(),
                influence: 50.0,
                established_tick: now_tick.saturating_sub(100),
                public_known,
                realm_band: Some(RealmBand::Mid),
            });
        }

        app.add_systems(Update, territory_rumor_enqueue_system);
        app.add_systems(Update, territory_rumor_spread_system);
        app
    }

    #[test]
    fn enqueue_only_when_public_known_true() {
        let now = 100_000u64;
        let mut app = make_spread_app_with_clock_and_zone(now, "spawn", true);

        // emit Established event
        app.world_mut()
            .resource_mut::<bevy_ecs::event::Events<DominanceChangedEvent>>()
            .send(DominanceChangedEvent {
                zone_name: "spawn".to_string(),
                kind: DominanceEventKind::Established,
                realm_band: RealmBand::Mid,
                observed_at_tick: now,
            });

        app.update();

        let pending = app.world().resource::<PendingZoneRumors>();
        assert_eq!(
            pending.queue.len(),
            1,
            "public_known=true 时应入队 1 条 rumor，实际={}",
            pending.queue.len()
        );
        assert_eq!(pending.queue[0].zone_name, "spawn");
        assert_eq!(pending.queue[0].realm_band, RealmBand::Mid);
    }

    #[test]
    fn enqueue_blocked_when_public_known_false() {
        let now = 100_000u64;
        let mut app = make_spread_app_with_clock_and_zone(now, "spawn", false);

        app.world_mut()
            .resource_mut::<bevy_ecs::event::Events<DominanceChangedEvent>>()
            .send(DominanceChangedEvent {
                zone_name: "spawn".to_string(),
                kind: DominanceEventKind::Established,
                realm_band: RealmBand::Mid,
                observed_at_tick: now,
            });

        app.update();

        let pending = app.world().resource::<PendingZoneRumors>();
        assert_eq!(
            pending.queue.len(),
            0,
            "public_known=false 时不应入队 rumor，实际={}",
            pending.queue.len()
        );
    }

    #[test]
    fn spread_delivers_rumor_to_home_zone_npc() {
        use valence::testing::create_mock_client;

        let now = 1_000_000u64; // 大 tick 确保延迟已到期
        let zone_name = "spawn";

        let mut app = App::new();
        app.add_event::<DominanceChangedEvent>();
        app.init_resource::<PendingZoneRumors>();
        app.init_resource::<RumorSpreadThrottle>();
        app.init_resource::<ZoneInfluenceMap>();

        use crate::world::zone::{Zone, ZoneRegistry};
        let zone = Zone {
            name: zone_name.to_string(),
            dimension: crate::world::dimension::DimensionKind::Overworld,
            bounds: (DVec3::new(0.0, 60.0, 0.0), DVec3::new(200.0, 80.0, 200.0)),
            spirit_qi: 0.8,
            danger_level: 0,
            active_events: vec![],
            patrol_anchors: vec![DVec3::new(100.0, 66.0, 100.0)],
            blocked_tiles: vec![],
        };
        app.insert_resource(ZoneRegistry { zones: vec![zone] });
        app.insert_resource(CultivationClock { tick: now });
        app.add_systems(Update, territory_rumor_spread_system);

        // 预置一条已到期的 rumor
        {
            let mut pending = app.world_mut().resource_mut::<PendingZoneRumors>();
            pending.queue.push(ZoneDominionRumor {
                zone_name: zone_name.to_string(),
                realm_band: RealmBand::Mid,
                observed_at_tick: now - 80_000,
                deliver_at_tick: now - 1, // 已到期
            });
        }

        // spawn NPC with NpcPatrol + NpcRumorMemory + Position
        use crate::npc::patrol::NpcPatrol;
        use crate::npc::spawn::NpcMarker;
        use valence::prelude::Position;
        let (client_bundle, _) = create_mock_client("npc_guard");
        // ClientBundle 已含 Position；额外插入以设置真实坐标（不重复 bundle，用 insert 覆写）
        let npc_entity = app
            .world_mut()
            .spawn((
                client_bundle,
                NpcPatrol::new(zone_name, DVec3::new(50.0, 66.0, 50.0)),
                NpcRumorMemory::default(),
                NpcMarker,
            ))
            .id();
        app.world_mut()
            .entity_mut(npc_entity)
            .insert(Position::new([50.0, 66.0, 50.0]));

        app.update();

        let memory = app
            .world()
            .entity(npc_entity)
            .get::<NpcRumorMemory>()
            .expect("NPC 应有 NpcRumorMemory 组件");
        assert_eq!(
            memory.rumors.len(),
            1,
            "到期 rumor 应写入 home_zone NPC memory，实际 rumors={:?}",
            memory.rumors
        );
        assert_eq!(memory.rumors[0].zone_name, zone_name);
        assert_eq!(memory.rumors[0].realm_band, RealmBand::Mid);
        let _ = npc_entity;
    }

    #[test]
    fn spread_not_delivered_before_due_tick() {
        let now = 50_000u64;
        let zone_name = "spawn";

        let mut app = App::new();
        app.add_event::<DominanceChangedEvent>();
        app.init_resource::<PendingZoneRumors>();
        app.init_resource::<RumorSpreadThrottle>();
        app.init_resource::<ZoneInfluenceMap>();

        use crate::world::zone::{Zone, ZoneRegistry};
        let zone = Zone {
            name: zone_name.to_string(),
            dimension: crate::world::dimension::DimensionKind::Overworld,
            bounds: (DVec3::new(0.0, 60.0, 0.0), DVec3::new(200.0, 80.0, 200.0)),
            spirit_qi: 0.8,
            danger_level: 0,
            active_events: vec![],
            patrol_anchors: vec![DVec3::new(100.0, 66.0, 100.0)],
            blocked_tiles: vec![],
        };
        app.insert_resource(ZoneRegistry { zones: vec![zone] });
        app.insert_resource(CultivationClock { tick: now });
        app.add_systems(Update, territory_rumor_spread_system);

        // 预置一条未到期的 rumor
        {
            let mut pending = app.world_mut().resource_mut::<PendingZoneRumors>();
            pending.queue.push(ZoneDominionRumor {
                zone_name: zone_name.to_string(),
                realm_band: RealmBand::Low,
                observed_at_tick: now,
                deliver_at_tick: now + 100_000, // 未到期
            });
        }

        use crate::npc::patrol::NpcPatrol;
        use crate::npc::spawn::NpcMarker;
        use valence::prelude::Position;
        use valence::testing::create_mock_client;
        let (client_bundle, _) = create_mock_client("npc_early");
        // ClientBundle 已含 Position；单独 insert 覆写坐标
        let npc_entity = app
            .world_mut()
            .spawn((
                client_bundle,
                NpcPatrol::new(zone_name, DVec3::new(50.0, 66.0, 50.0)),
                NpcRumorMemory::default(),
                NpcMarker,
            ))
            .id();
        app.world_mut()
            .entity_mut(npc_entity)
            .insert(Position::new([50.0, 66.0, 50.0]));

        app.update();

        let memory = app
            .world()
            .entity(npc_entity)
            .get::<NpcRumorMemory>()
            .expect("NPC 应有 NpcRumorMemory");
        assert_eq!(
            memory.rumors.len(),
            0,
            "未到期 rumor 不应写入 NPC memory，实际={}",
            memory.rumors.len()
        );
        let _ = npc_entity;
    }

    // ── 7. 跨 zone 相邻传播：NPC home=B，rumor.zone=A，A 与 B 相邻 → B 的 NPC 收到 ──
    //       同时断言非相邻 far zone 的 NPC **不**收到（正负向一体测试）。
    #[test]
    fn spread_adjacent_zone_npc_receives_and_far_zone_npc_does_not() {
        use crate::npc::patrol::NpcPatrol;
        use crate::npc::spawn::NpcMarker;
        use crate::world::zone::{Zone, ZoneRegistry};
        use valence::prelude::Position;
        use valence::testing::create_mock_client;

        let now = 2_000_000u64;

        // zone_a: x [0, 200], z [0, 200]
        // zone_b: x [201, 400], z [0, 200]  — gap_x = 1, within ADJACENT_ZONE_MARGIN(150)
        // zone_far: x [2000, 2200], z [0, 200] — gap_x = 1800, far outside margin
        let zone_a = Zone {
            name: "zone_a".to_string(),
            dimension: crate::world::dimension::DimensionKind::Overworld,
            bounds: (DVec3::new(0.0, 60.0, 0.0), DVec3::new(200.0, 80.0, 200.0)),
            spirit_qi: 0.8,
            danger_level: 0,
            active_events: vec![],
            patrol_anchors: vec![DVec3::new(100.0, 66.0, 100.0)],
            blocked_tiles: vec![],
        };
        let zone_b = Zone {
            name: "zone_b".to_string(),
            dimension: crate::world::dimension::DimensionKind::Overworld,
            bounds: (DVec3::new(201.0, 60.0, 0.0), DVec3::new(400.0, 80.0, 200.0)),
            spirit_qi: 0.6,
            danger_level: 0,
            active_events: vec![],
            patrol_anchors: vec![DVec3::new(300.0, 66.0, 100.0)],
            blocked_tiles: vec![],
        };
        let zone_far = Zone {
            name: "zone_far".to_string(),
            dimension: crate::world::dimension::DimensionKind::Overworld,
            bounds: (
                DVec3::new(2000.0, 60.0, 0.0),
                DVec3::new(2200.0, 80.0, 200.0),
            ),
            spirit_qi: 0.4,
            danger_level: 0,
            active_events: vec![],
            patrol_anchors: vec![DVec3::new(2100.0, 66.0, 100.0)],
            blocked_tiles: vec![],
        };

        let mut app = App::new();
        app.add_event::<DominanceChangedEvent>();
        app.init_resource::<PendingZoneRumors>();
        app.init_resource::<RumorSpreadThrottle>();
        app.init_resource::<ZoneInfluenceMap>();
        app.insert_resource(ZoneRegistry {
            zones: vec![zone_a, zone_b, zone_far],
        });
        app.insert_resource(CultivationClock { tick: now });
        app.add_systems(Update, territory_rumor_spread_system);

        // 预置一条已到期的 rumor for zone_a
        {
            let mut pending = app.world_mut().resource_mut::<PendingZoneRumors>();
            pending.queue.push(ZoneDominionRumor {
                zone_name: "zone_a".to_string(),
                realm_band: RealmBand::High,
                observed_at_tick: now - 100_000,
                deliver_at_tick: now - 1, // 已到期
            });
        }

        // NPC 甲: home_zone=zone_b（与 zone_a 相邻，应收到 rumor）
        let (client_bundle_b, _) = create_mock_client("npc_adjacent");
        let npc_adjacent = app
            .world_mut()
            .spawn((
                client_bundle_b,
                NpcPatrol::new("zone_b", DVec3::new(300.0, 66.0, 100.0)),
                NpcRumorMemory::default(),
                NpcMarker,
            ))
            .id();
        app.world_mut()
            .entity_mut(npc_adjacent)
            .insert(Position::new([300.0, 66.0, 100.0]));

        // NPC 乙: home_zone=zone_far（与 zone_a 不相邻，不应收到 rumor）
        let (client_bundle_far, _) = create_mock_client("npc_far");
        let npc_far = app
            .world_mut()
            .spawn((
                client_bundle_far,
                NpcPatrol::new("zone_far", DVec3::new(2100.0, 66.0, 100.0)),
                NpcRumorMemory::default(),
                NpcMarker,
            ))
            .id();
        app.world_mut()
            .entity_mut(npc_far)
            .insert(Position::new([2100.0, 66.0, 100.0]));

        app.update();

        // 断言：相邻 zone NPC 收到 rumor
        let adj_memory = app
            .world()
            .entity(npc_adjacent)
            .get::<NpcRumorMemory>()
            .expect("zone_b NPC 应有 NpcRumorMemory");
        assert_eq!(
            adj_memory.rumors.len(),
            1,
            "zone_b NPC（与 zone_a 相邻）应收到 1 条 rumor，\
            实际 rumors={:?}。检查 should_receive 相邻分支是否有效",
            adj_memory.rumors
        );
        assert_eq!(
            adj_memory.rumors[0].zone_name, "zone_a",
            "相邻 NPC 收到的 rumor zone_name 应为 zone_a"
        );
        assert_eq!(
            adj_memory.rumors[0].realm_band,
            RealmBand::High,
            "相邻 NPC 收到的 rumor realm_band 应为 High"
        );

        // 断言：非相邻 zone NPC 不收到 rumor
        let far_memory = app
            .world()
            .entity(npc_far)
            .get::<NpcRumorMemory>()
            .expect("zone_far NPC 应有 NpcRumorMemory");
        assert_eq!(
            far_memory.rumors.len(),
            0,
            "zone_far NPC（与 zone_a 不相邻，gap=1800）不应收到 rumor，\
            实际 rumors={:?}。检查 should_receive 是否被短路成全员广播",
            far_memory.rumors
        );

        let _ = (npc_adjacent, npc_far);
    }
}
