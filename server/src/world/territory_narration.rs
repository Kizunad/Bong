//! plan-territory-v1 P3 — 天道 narration 双路 emit + 三时刻变动检测。
//!
//! ## 三时刻检测
//! 在 territory_tick(P0) 和 territory_pvp_influence_system(P2) 两处 dominance 重算点
//! 通过 EventWriter<DominanceChangedEvent> emit 变动事件。本模块提供：
//!
//! 1. `DominanceChangedEvent` — 新增事件，携带 zone/kind/realm_band/observed_at_tick。
//! 2. `territory_narration_system` — 读 DominanceChangedEvent →
//!    (a) push_zone(PendingGameplayNarrations, 模板文本) 兜底（不依赖 agent 即 in-game 可见）
//!    (b) tx_outbound.send(RedisOutbound::TerritoryDominanceNarration) 留契约频道供 agent。
//! 3. `territory_public_reveal_system` — 延迟 PUBLIC_KNOWN_DELAY_TICKS 后翻 public_known=true。
//!
//! ## 防孤岛
//! - push_zone 路径不依赖 agent，是真闭环。
//! - Redis 路径留契约供未来 agent territory-narration runtime 接入。
//! - 两路同时 emit，互不干扰。
//!
//! ## 天道视角（即时）vs 江湖传话（延迟+public_known）
//! - narration（天道旁白）：天道全知，霸主确立即评论（不 gate public_known）。
//! - NPC 传话：江湖消息，延迟 1-3hr 且 gate public_known=true。
//!
//! ## DEFER
//! 环境痕迹 client 视觉（GroundDecal/Sprite 影响力三档粒子痕迹）DEFER 到独立 client 资产 PR。

use valence::prelude::{bevy_ecs, App, Event, EventReader, Res, ResMut, Update};

use crate::cultivation::tick::CultivationClock;
use crate::network::redis_bridge::RedisOutbound;
use crate::network::RedisBridgeResource;
use crate::player::gameplay::PendingGameplayNarrations;
use crate::schema::common::NarrationStyle;
use crate::schema::territory_narration::{
    TerritoryDominanceNarrationRequestV1, TerritoryEventKindV1,
};
use crate::world::territory::ZoneInfluenceMap;
use crate::world::territory_rumor::RealmBand;
use crate::world::zone::ZoneRegistry;

// ─── 常量 ─────────────────────────────────────────────────────────────────────

/// public_known 翻 true 延迟（约 1hr game time）。
/// 语义：霸主确立后延迟传播，"霸主刚确立时无人知"。
pub const PUBLIC_KNOWN_DELAY_TICKS: u64 = 72_000; // 1hr = 72_000 ticks

/// 灵气耗尽阈值（spirit_qi ≤ 0 即废地，worldview §十）。
const QI_DEPLETED_THRESHOLD: f64 = 0.0;

// ─── DominanceChangedEvent ─────────────────────────────────────────────────────

/// 三时刻变动类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DominanceEventKind {
    /// 新霸主确立（旧主=None 或换主）。
    Established,
    /// 霸主被驱逐（跌破 DOMINANCE_DROP 且无继任）。
    Ousted,
    /// 区域灵气耗尽（spirit_qi 从 >阈值 跌至 ≤阈值）。
    QiDepleted,
}

/// 领地霸主变动事件（Bevy Event）。
///
/// 由 territory.rs 的两处重算点（territory_tick + territory_pvp_influence_system）emit。
/// 本模块的 territory_narration_system 和 territory_rumor.rs 的 enqueue_system 消费。
#[derive(Debug, Clone, Event)]
pub struct DominanceChangedEvent {
    pub zone_name: String,
    pub kind: DominanceEventKind,
    /// 匿名境界段（仅 Low/Mid/High，不含 char_id）。
    pub realm_band: RealmBand,
    /// 事件触发时的游戏 tick（测试 / agent 消费用）。
    #[allow(dead_code)]
    pub observed_at_tick: u64,
}

// ─── 灵气水位追踪 ─────────────────────────────────────────────────────────────

/// 区域灵气水位 Resource（边沿检测，防 QiDepleted 每 tick 刷屏）。
#[derive(Debug, Default)]
pub struct ZoneQiWatermark {
    /// key = zone_name，value = 上一 tick 的 spirit_qi
    pub last_qi: std::collections::HashMap<String, f64>,
}

impl valence::prelude::Resource for ZoneQiWatermark {}

// ─── public_known 延迟传播系统 ────────────────────────────────────────────────

/// 延迟 PUBLIC_KNOWN_DELAY_TICKS 后将霸主的 public_known 翻转为 true。
///
/// 语义：霸主确立后传播延迟——"刚确立时无人知，1hr 后才公开"。
/// 天道 narration 不 gate（天道全知即时评论）。
/// NPC 传话 gate 在 public_known=true 之后（江湖消息有延迟）。
pub fn territory_public_reveal_system(
    clock: Option<Res<CultivationClock>>,
    mut influence_map: ResMut<ZoneInfluenceMap>,
) {
    let Some(clock) = clock else {
        return;
    };
    let now_tick = clock.tick;

    for entry in influence_map.zones.values_mut() {
        if let Some(dominant) = entry.dominant.as_mut() {
            if !dominant.public_known
                && now_tick >= dominant.established_tick + PUBLIC_KNOWN_DELAY_TICKS
            {
                dominant.public_known = true;
            }
        }
    }
}

// ─── narration system ─────────────────────────────────────────────────────────

/// 天道 narration 双路 emit system。
///
/// 读 DominanceChangedEvent → 两路 emit：
/// (a) push_zone(PendingGameplayNarrations) — 模板文本，不依赖 agent 即 in-game 可见（兜底）。
/// (b) tx_outbound.send(TerritoryDominanceNarration) — Redis 契约频道，供未来 agent runtime。
///
/// 三时刻模板文本：
/// - Established: 「{zone}上，不知何时多了一位常客。灵草因此疏了。」
/// - Ousted: 「{zone}那{band}修士，已无人见到。」
/// - QiDepleted: 「{zone}灵气已尽，地气沉寂。」
pub fn territory_narration_system(
    mut ev_reader: EventReader<DominanceChangedEvent>,
    mut narrations: Option<ResMut<PendingGameplayNarrations>>,
    redis: Option<Res<RedisBridgeResource>>,
    zones: Option<Res<ZoneRegistry>>,
) {
    for ev in ev_reader.read() {
        // 获取当前 spirit_qi（用于 Redis payload）
        let spirit_qi = zones
            .as_deref()
            .and_then(|registry| registry.find_zone_by_name(&ev.zone_name))
            .map(|zone| zone.spirit_qi)
            .unwrap_or(0.0);

        // ── (a) push_zone 兜底（不依赖 agent）──────────────────────────────
        let template_text = narration_template(&ev.zone_name, ev.kind, ev.realm_band);
        if let Some(ref mut n) = narrations {
            n.push_zone(
                &ev.zone_name,
                template_text.clone(),
                NarrationStyle::PoliticalJianghu,
            );
        }

        // ── (b) Redis 契约频道（供未来 agent territory-narration runtime）──
        let event_kind = match ev.kind {
            DominanceEventKind::Established => TerritoryEventKindV1::Established,
            DominanceEventKind::Ousted => TerritoryEventKindV1::Ousted,
            DominanceEventKind::QiDepleted => TerritoryEventKindV1::QiDepleted,
        };
        let realm_band_str = match ev.realm_band {
            RealmBand::Low => "low",
            RealmBand::Mid => "mid",
            RealmBand::High => "high",
        };
        let req = TerritoryDominanceNarrationRequestV1::new(
            &ev.zone_name,
            event_kind,
            realm_band_str,
            spirit_qi,
        );
        if let Some(ref r) = redis {
            let _ = r
                .tx_outbound
                .send(RedisOutbound::TerritoryDominanceNarration(req));
        }
    }
}

/// 三时刻 narration 模板文本。
fn narration_template(zone: &str, kind: DominanceEventKind, band: RealmBand) -> String {
    match kind {
        DominanceEventKind::Established => {
            format!("{zone}上，不知何时多了一位常客。灵草因此疏了。")
        }
        DominanceEventKind::Ousted => {
            let band_word = match band {
                RealmBand::Low => "低段",
                RealmBand::Mid => "中段",
                RealmBand::High => "高段",
            };
            format!("{zone}那{band_word}修士，已无人见到。")
        }
        DominanceEventKind::QiDepleted => {
            format!("{zone}灵气已尽，地气沉寂。")
        }
    }
}

/// 向 territory.rs 注册新增的事件和系统。
///
/// 调用方（territory.rs::register）应调用本函数将三时刻系统接入 Bevy schedule。
pub fn register(app: &mut App) {
    app.add_event::<DominanceChangedEvent>();
    app.init_resource::<ZoneQiWatermark>();
    app.add_systems(Update, territory_public_reveal_system);
    app.add_systems(Update, territory_narration_system);
}

// ─── 供 territory.rs 使用的辅助函数 ────────────────────────────────────────

/// 从 spirit_qi 变化检测 QiDepleted 边沿（纯函数，可单测）。
pub fn detect_qi_depleted_edge(last_qi: f64, new_qi: f64) -> bool {
    last_qi > QI_DEPLETED_THRESHOLD && new_qi <= QI_DEPLETED_THRESHOLD
}

// ─── 单测 ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod narration_tests {
    use super::*;
    use crate::schema::channels::CH_TERRITORY_NARRATION_REQUEST;
    use crate::world::territory::ZoneDominance;
    use crate::world::territory_rumor::RealmBand;

    // ── 1. public_known 延迟前为 false ──────────────────────────────────────

    #[test]
    fn public_known_remains_false_before_delay() {
        use crate::cultivation::tick::CultivationClock;
        use valence::prelude::{App, Update};

        let mut app = App::new();
        app.init_resource::<ZoneInfluenceMap>();
        app.add_systems(Update, territory_public_reveal_system);

        let established_tick = 1000u64;
        // clock = established + DELAY - 1（未到延迟）
        app.insert_resource(CultivationClock {
            tick: established_tick + PUBLIC_KNOWN_DELAY_TICKS - 1,
        });

        // 写入霸主 established_tick=1000, public_known=false
        {
            let mut map = app.world_mut().resource_mut::<ZoneInfluenceMap>();
            let entry = map.zones.entry("spawn".to_string()).or_default();
            entry.dominant = Some(ZoneDominance {
                char_id: "p1".to_string(),
                influence: 50.0,
                established_tick,
                public_known: false,
                realm_band: Some(RealmBand::Mid),
            });
        }

        app.update();

        let map = app.world().resource::<ZoneInfluenceMap>();
        let public_known = map
            .zones
            .get("spawn")
            .and_then(|e| e.dominant.as_ref())
            .map(|d| d.public_known)
            .unwrap_or(true);
        assert!(
            !public_known,
            "延迟 {PUBLIC_KNOWN_DELAY_TICKS} tick 未到前 public_known 应为 false，实际=true"
        );
    }

    // ── 2. public_known 延迟后翻 true ────────────────────────────────────────

    #[test]
    fn public_known_becomes_true_at_delay() {
        use crate::cultivation::tick::CultivationClock;
        use valence::prelude::{App, Update};

        let mut app = App::new();
        app.init_resource::<ZoneInfluenceMap>();
        app.add_systems(Update, territory_public_reveal_system);

        let established_tick = 1000u64;
        // clock = established + DELAY（恰好到期）
        app.insert_resource(CultivationClock {
            tick: established_tick + PUBLIC_KNOWN_DELAY_TICKS,
        });

        {
            let mut map = app.world_mut().resource_mut::<ZoneInfluenceMap>();
            let entry = map.zones.entry("qingyun".to_string()).or_default();
            entry.dominant = Some(ZoneDominance {
                char_id: "p2".to_string(),
                influence: 60.0,
                established_tick,
                public_known: false,
                realm_band: Some(RealmBand::High),
            });
        }

        app.update();

        let map = app.world().resource::<ZoneInfluenceMap>();
        let public_known = map
            .zones
            .get("qingyun")
            .and_then(|e| e.dominant.as_ref())
            .map(|d| d.public_known)
            .unwrap_or(false);
        assert!(
            public_known,
            "延迟 {PUBLIC_KNOWN_DELAY_TICKS} tick 到期后 public_known 应翻 true，实际=false"
        );
    }

    // ── 3. Established narration push_zone 真入队 ───────────────────────────

    #[test]
    fn narration_established_push_zone_enqueued() {
        use valence::prelude::{App, Update};

        let mut app = App::new();
        app.add_event::<DominanceChangedEvent>();
        app.init_resource::<ZoneInfluenceMap>();
        app.insert_resource(PendingGameplayNarrations::default());
        app.add_systems(Update, territory_narration_system);

        // emit Established event
        app.world_mut()
            .resource_mut::<valence::prelude::bevy_ecs::event::Events<DominanceChangedEvent>>()
            .send(DominanceChangedEvent {
                zone_name: "spawn".to_string(),
                kind: DominanceEventKind::Established,
                realm_band: RealmBand::Low,
                observed_at_tick: 1000,
            });

        app.update();

        let narrations = app
            .world_mut()
            .resource_mut::<PendingGameplayNarrations>()
            .drain();
        assert_eq!(
            narrations.len(),
            1,
            "Established 应触发 1 条 push_zone，实际={}",
            narrations.len()
        );
        let n = &narrations[0];
        assert!(
            n.text.contains("spawn"),
            "narration 文本应含 zone 名 spawn，实际={}",
            n.text
        );
        assert_eq!(
            n.style,
            NarrationStyle::PoliticalJianghu,
            "narration style 应为 PoliticalJianghu"
        );
    }

    // ── 4. Ousted narration ──────────────────────────────────────────────────

    #[test]
    fn narration_ousted_push_zone_enqueued() {
        use valence::prelude::{App, Update};

        let mut app = App::new();
        app.add_event::<DominanceChangedEvent>();
        app.init_resource::<ZoneInfluenceMap>();
        app.insert_resource(PendingGameplayNarrations::default());
        app.add_systems(Update, territory_narration_system);

        app.world_mut()
            .resource_mut::<valence::prelude::bevy_ecs::event::Events<DominanceChangedEvent>>()
            .send(DominanceChangedEvent {
                zone_name: "blood_valley".to_string(),
                kind: DominanceEventKind::Ousted,
                realm_band: RealmBand::High,
                observed_at_tick: 2000,
            });

        app.update();

        let narrations = app
            .world_mut()
            .resource_mut::<PendingGameplayNarrations>()
            .drain();
        assert_eq!(narrations.len(), 1, "Ousted 应触发 1 条 push_zone");
        let n = &narrations[0];
        assert!(
            n.text.contains("blood_valley"),
            "narration 含 zone 名，实际={}",
            n.text
        );
        assert!(
            n.text.contains("高段"),
            "Ousted narration 应含境界段词，实际={}",
            n.text
        );
    }

    // ── 5. QiDepleted narration ──────────────────────────────────────────────

    #[test]
    fn narration_qi_depleted_push_zone_enqueued() {
        use valence::prelude::{App, Update};

        let mut app = App::new();
        app.add_event::<DominanceChangedEvent>();
        app.init_resource::<ZoneInfluenceMap>();
        app.insert_resource(PendingGameplayNarrations::default());
        app.add_systems(Update, territory_narration_system);

        app.world_mut()
            .resource_mut::<valence::prelude::bevy_ecs::event::Events<DominanceChangedEvent>>()
            .send(DominanceChangedEvent {
                zone_name: "north_wastes".to_string(),
                kind: DominanceEventKind::QiDepleted,
                realm_band: RealmBand::Low,
                observed_at_tick: 3000,
            });

        app.update();

        let narrations = app
            .world_mut()
            .resource_mut::<PendingGameplayNarrations>()
            .drain();
        assert_eq!(narrations.len(), 1, "QiDepleted 应触发 1 条 push_zone");
        let n = &narrations[0];
        assert!(
            n.text.contains("north_wastes"),
            "narration 含 zone 名，实际={}",
            n.text
        );
        assert!(
            n.text.contains("灵气"),
            "QiDepleted narration 应含「灵气」，实际={}",
            n.text
        );
    }

    // ── 6. Redis 路径：tx_outbound 收到 TerritoryDominanceNarration ──────────

    #[test]
    fn narration_redis_path_sends_to_tx_outbound() {
        use crate::network::redis_bridge::RedisOutbound;
        use crate::network::RedisBridgeResource;
        use crate::schema::territory_narration::TerritoryEventKindV1;
        use crate::world::dimension::DimensionKind;
        use crate::world::zone::{Zone, ZoneRegistry};
        use crossbeam_channel::unbounded;
        use valence::prelude::{App, DVec3, Update};

        let (tx, rx) = unbounded::<RedisOutbound>();
        let (_, rx_inbound) = unbounded::<crate::network::redis_bridge::RedisInbound>();

        let mut app = App::new();
        app.add_event::<DominanceChangedEvent>();
        app.init_resource::<ZoneInfluenceMap>();
        app.insert_resource(PendingGameplayNarrations::default());
        app.insert_resource(RedisBridgeResource {
            tx_outbound: tx,
            rx_inbound,
        });
        app.insert_resource(ZoneRegistry {
            zones: vec![Zone {
                name: "qingyun_peaks".to_string(),
                dimension: DimensionKind::Overworld,
                bounds: (DVec3::new(0.0, 60.0, 0.0), DVec3::new(200.0, 80.0, 200.0)),
                spirit_qi: 0.73,
                danger_level: 0,
                active_events: vec![],
                patrol_anchors: vec![DVec3::new(100.0, 66.0, 100.0)],
                blocked_tiles: vec![],
            }],
        });
        app.add_systems(Update, territory_narration_system);

        app.world_mut()
            .resource_mut::<valence::prelude::bevy_ecs::event::Events<DominanceChangedEvent>>()
            .send(DominanceChangedEvent {
                zone_name: "qingyun_peaks".to_string(),
                kind: DominanceEventKind::Established,
                realm_band: RealmBand::Mid,
                observed_at_tick: 5000,
            });

        app.update();

        // rx 应收到一条 TerritoryDominanceNarration
        let received = rx
            .try_recv()
            .expect("tx_outbound 应收到 TerritoryDominanceNarration");
        match received {
            RedisOutbound::TerritoryDominanceNarration(req) => {
                assert_eq!(req.zone, "qingyun_peaks", "zone 字段应正确");
                assert_eq!(
                    req.event_kind,
                    TerritoryEventKindV1::Established,
                    "event_kind 应为 Established"
                );
                assert_eq!(req.realm_band, "mid", "realm_band 应为 mid");
                assert_eq!(req.v, 1, "v 字段应为 1");
                assert!(
                    (req.spirit_qi - 0.73).abs() < f64::EPSILON,
                    "Redis payload spirit_qi 应来自 ZoneRegistry 当前值，实际={}",
                    req.spirit_qi
                );
            }
            other => panic!("期望 TerritoryDominanceNarration，实际={other:?}"),
        }
    }

    #[test]
    fn narration_redis_path_defaults_spirit_qi_to_zero_when_zone_missing() {
        use crate::network::redis_bridge::RedisOutbound;
        use crate::network::RedisBridgeResource;
        use crossbeam_channel::unbounded;
        use valence::prelude::{App, Update};

        let (tx, rx) = unbounded::<RedisOutbound>();
        let (_, rx_inbound) = unbounded::<crate::network::redis_bridge::RedisInbound>();

        let mut app = App::new();
        app.add_event::<DominanceChangedEvent>();
        app.init_resource::<ZoneInfluenceMap>();
        app.insert_resource(RedisBridgeResource {
            tx_outbound: tx,
            rx_inbound,
        });
        app.add_systems(Update, territory_narration_system);

        app.world_mut()
            .resource_mut::<valence::prelude::bevy_ecs::event::Events<DominanceChangedEvent>>()
            .send(DominanceChangedEvent {
                zone_name: "missing_zone".to_string(),
                kind: DominanceEventKind::QiDepleted,
                realm_band: RealmBand::Low,
                observed_at_tick: 6000,
            });

        app.update();

        let received = rx
            .try_recv()
            .expect("tx_outbound 应收到缺失 zone 的 TerritoryDominanceNarration");
        match received {
            RedisOutbound::TerritoryDominanceNarration(req) => {
                assert_eq!(
                    req.spirit_qi, 0.0,
                    "缺失 ZoneRegistry/zone 时应使用 0.0 默认值而非旧的 0.5"
                );
            }
            other => panic!("期望 TerritoryDominanceNarration，实际={other:?}"),
        }
    }

    // ── 7. CH_TERRITORY_NARRATION_REQUEST 频道值已固定 ──────────────────────

    #[test]
    fn channel_territory_narration_request_frozen() {
        assert_eq!(
            CH_TERRITORY_NARRATION_REQUEST, "bong:territory_narration_request",
            "频道名应固定为 bong:territory_narration_request"
        );
    }

    // ── 8. detect_qi_depleted_edge 边沿检测 ────────────────────────────────

    #[test]
    fn qi_depleted_edge_detected_when_crossing_threshold() {
        assert!(
            detect_qi_depleted_edge(0.5, 0.0),
            "spirit_qi 从 0.5 跌到 0.0 应检测到 QiDepleted 边沿"
        );
        assert!(
            detect_qi_depleted_edge(0.1, -0.1),
            "spirit_qi 从正跌到负应检测到边沿"
        );
    }

    #[test]
    fn qi_depleted_edge_not_detected_when_already_zero() {
        // 连续两 tick spirit_qi=0，不重复触发
        assert!(
            !detect_qi_depleted_edge(0.0, 0.0),
            "spirit_qi 已为 0 维持不变不应重复触发边沿"
        );
        assert!(
            !detect_qi_depleted_edge(-0.1, 0.0),
            "spirit_qi 从负变 0 不应触发（已在耗尽状态）"
        );
    }

    #[test]
    fn qi_depleted_edge_not_detected_when_still_positive() {
        assert!(
            !detect_qi_depleted_edge(0.5, 0.3),
            "spirit_qi 仍为正不应触发 QiDepleted"
        );
    }

    // ── 9. narration_template 三时刻模板文本覆盖 ────────────────────────────

    #[test]
    fn narration_template_established_contains_zone() {
        let text = narration_template("spawn", DominanceEventKind::Established, RealmBand::Mid);
        assert!(text.contains("spawn"), "Established 模板应含 zone 名");
        assert!(text.contains("常客"), "Established 模板应含「常客」");
    }

    #[test]
    fn narration_template_ousted_contains_band_word() {
        let text_low = narration_template("spawn", DominanceEventKind::Ousted, RealmBand::Low);
        assert!(text_low.contains("低段"), "Ousted Low 模板应含「低段」");

        let text_mid = narration_template("spawn", DominanceEventKind::Ousted, RealmBand::Mid);
        assert!(text_mid.contains("中段"), "Ousted Mid 模板应含「中段」");

        let text_high = narration_template("spawn", DominanceEventKind::Ousted, RealmBand::High);
        assert!(text_high.contains("高段"), "Ousted High 模板应含「高段」");
    }

    #[test]
    fn narration_template_qi_depleted_contains_qi_word() {
        let text = narration_template(
            "north_wastes",
            DominanceEventKind::QiDepleted,
            RealmBand::Low,
        );
        assert!(text.contains("灵气"), "QiDepleted 模板应含「灵气」");
        assert!(text.contains("north_wastes"), "QiDepleted 模板应含 zone 名");
    }
}
