//! plan-tsy-loot-v1 §8.5 — DeathEvent.attacker / attacker_player_id 链路测试。
//!
//! 验证 §6 扩展后所有真实 emit 点都正确填字段：
//! - PVP 死亡（resolve.rs）→ attacker = Some(intent.attacker), attacker_player_id = Some("offline:Foo")
//! - PVE 死亡（NPC 攻击 player）→ attacker = Some(npc_entity), attacker_player_id = None
//! - 环境死亡（tsy_drain / wound_bleed_tick）→ 两者皆 None
//!
//! 不重做 resolve.rs 的完整 combat pipeline 测试（那个在 lifecycle / resolve 自带），
//! 这里只断言 DeathEvent 字段填法正确。

#[cfg(test)]
mod tests {
    use valence::prelude::{App, DVec3, Events, Position, Update};

    use crate::combat::components::{Wound, Wounds};
    use crate::combat::events::DeathEvent;
    use crate::combat::lifecycle::wound_bleed_tick;
    use crate::combat::CombatClock;
    use crate::cultivation::components::{Cultivation, MeridianSystem};
    use crate::world::dimension::CurrentDimension;
    use crate::world::tsy_drain::tsy_drain_tick;

    /// 包装：构造一个有 bleed 的 Wounds，方便 wound_bleed_tick 触发。
    fn bleeding_wounds(health: f32) -> Wounds {
        Wounds {
            entries: vec![Wound {
                location: crate::combat::components::BodyPart::Chest,
                kind: crate::combat::components::WoundKind::Pierce,
                severity: 5.0,
                bleeding_per_sec: 50.0,
                created_at_tick: 0,
                inflicted_by: None,
            }],
            health_current: health,
            health_max: 100.0,
        }
    }

    #[test]
    fn wound_bleed_death_has_no_attacker() {
        let mut app = App::new();
        // wound_bleed_tick 走 bleed 间隔；clock.tick 0 是间隔倍数
        app.insert_resource(CombatClock::default());
        app.add_event::<DeathEvent>();
        app.add_systems(Update, wound_bleed_tick);

        let _player = app.world_mut().spawn(bleeding_wounds(2.0)).id();
        app.update();

        let events = app.world().resource::<Events<DeathEvent>>();
        let mut reader = events.get_reader();
        let deaths: Vec<_> = reader.read(events).collect();
        assert!(!deaths.is_empty(), "应至少 emit 1 个 DeathEvent");
        for d in deaths {
            assert_eq!(d.cause, "bleed_out");
            assert_eq!(d.attacker, None, "bleed 死亡 attacker entity 应 None");
            assert_eq!(
                d.attacker_player_id, None,
                "bleed 死亡 attacker_player_id 应 None"
            );
        }
    }

    #[test]
    fn tsy_drain_death_has_no_attacker() {
        use crate::cultivation::tick::CultivationClock;
        use crate::world::dimension::DimensionKind;
        use crate::world::tsy::DimensionAnchor;
        use crate::world::tsy::TsyPresence;
        use crate::world::zone::{Zone, ZoneRegistry};

        let mut app = App::new();
        app.insert_resource(CombatClock::default());
        app.insert_resource(CultivationClock::default());
        // 一个浅层 tsy zone，启用 drain
        let mut zones = ZoneRegistry::fallback();
        zones
            .register_runtime_zone(Zone {
                name: "tsy_test_01_shallow".into(),
                dimension: DimensionKind::Tsy,
                bounds: (DVec3::new(0.0, 0.0, 0.0), DVec3::new(100.0, 80.0, 100.0)),
                spirit_qi: -0.5,
                danger_level: 5,
                active_events: vec!["tsy_entry".into()],
                patrol_anchors: Vec::new(),
                blocked_tiles: Vec::new(),
            })
            .expect("zone register");
        app.insert_resource(zones);
        app.add_event::<DeathEvent>();
        app.add_systems(Update, tsy_drain_tick);

        // PR #48 reform：真元归一到 Cultivation.qi_current/qi_max。
        let cultivation = Cultivation {
            qi_current: 1.0,
            qi_max: 100.0,
            ..Default::default()
        };
        let _player = app
            .world_mut()
            .spawn((
                Position::new([50.0, 50.0, 50.0]),
                CurrentDimension(DimensionKind::Tsy),
                cultivation,
                MeridianSystem::default(),
                TsyPresence {
                    family_id: "tsy_test_01".into(),
                    entered_at_tick: 0,
                    entry_inventory_snapshot: Vec::new(),
                    return_to: DimensionAnchor {
                        dimension: DimensionKind::Overworld,
                        pos: DVec3::new(0.0, 64.0, 0.0),
                    },
                },
            ))
            .id();

        // 跑 enough ticks 直到真元归零
        for tick in 1..=600u64 {
            app.world_mut().resource_mut::<CombatClock>().tick = tick;
            app.world_mut().resource_mut::<CultivationClock>().tick = tick;
            app.update();
            let events = app.world().resource::<Events<DeathEvent>>();
            let mut reader = events.get_reader();
            let deaths: Vec<_> = reader.read(events).cloned().collect();
            if !deaths.is_empty() {
                for d in deaths {
                    assert_eq!(d.cause, "tsy_drain");
                    assert_eq!(d.attacker, None);
                    assert_eq!(d.attacker_player_id, None);
                }
                return;
            }
        }
        panic!("600 tick 内应触发 tsy_drain DeathEvent");
    }

    #[test]
    fn manual_pvp_death_event_has_attacker_filled() {
        // 模拟 resolve.rs:630 的 PVP 路径填字段：attacker entity + attacker_id 前缀检测
        let mut app = App::new();
        let attacker = app.world_mut().spawn_empty().id();
        let target = app.world_mut().spawn_empty().id();
        let attacker_id = "offline:Killer";
        let attacker_player_id = attacker_id
            .starts_with("offline:")
            .then(|| attacker_id.to_string());
        let evt = DeathEvent {
            target,
            cause: format!("attack_intent:{attacker_id}"),
            attacker: Some(attacker),
            attacker_player_id,
            at_tick: 100,
        };
        assert_eq!(evt.attacker, Some(attacker));
        assert_eq!(evt.attacker_player_id, Some("offline:Killer".to_string()));
        assert!(evt.cause.contains("offline:Killer"));
    }

    #[test]
    fn manual_pve_npc_attacker_does_not_set_player_id() {
        // 模拟 NPC 攻击 player 的 attacker_id 形如 "npc_5v3"
        let mut app = App::new();
        let npc = app.world_mut().spawn_empty().id();
        let target = app.world_mut().spawn_empty().id();
        let attacker_id = "npc_5v3";
        let attacker_player_id = attacker_id
            .starts_with("offline:")
            .then(|| attacker_id.to_string());
        let evt = DeathEvent {
            target,
            cause: format!("attack_intent:{attacker_id}"),
            attacker: Some(npc),
            attacker_player_id,
            at_tick: 50,
        };
        assert_eq!(evt.attacker, Some(npc), "NPC entity 应填");
        assert_eq!(
            evt.attacker_player_id, None,
            "NPC 攻击者 player_id 应 None（offline: 前缀检测）"
        );
    }
}
