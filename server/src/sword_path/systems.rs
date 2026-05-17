//! plan-sword-path-v2 — 剑道 v1 数据结构 + 纯函数的 Bevy 运行时接线。
//!
//! 涵盖 P1.1（绑定追踪）+ P1.3（碎裂反噬）+ P2.2（盲区 tick）。其他 ECS-side
//! 行为（招式 cast / 真元注入 / 化虚 runtime）走 `super::skill_register`，因为
//! 它们都是 `SkillFn` 入口而不是独立 Bevy system。

use valence::prelude::{Commands, Entity, EventReader, EventWriter, Events, Query, Res, ResMut};

use crate::combat::events::{AttackSource, CombatEvent};
use crate::combat::weapon::{Weapon, WeaponKind};
use crate::combat::CombatClock;
use crate::cultivation::components::Cultivation;
use crate::qi_physics::{QiAccountId, QiTransfer, QiTransferReason};
use crate::world::zone::DEFAULT_SPAWN_ZONE_NAME;

use super::bond::{
    SwordBondComponent, SwordBondFormedEvent, SwordBondProgress, SwordShatterEvent,
    BOND_TRIGGER_USES,
};
use super::heaven_gate::TiandaoBlindZoneRegistry;
use super::shatter::compute_shatter_outcome;

/// P1.1 — 追踪玩家连续使用剑术次数：≥ `BOND_TRIGGER_USES` 时挂载
/// `SwordBondComponent`。换剑 / 不持剑 / 已绑定 → 重置或拒绝。
///
/// 触发条件：玩家 (`attacker`) 持 `WeaponKind::Sword`，且本次 `CombatEvent.source`
/// 属于 sword_basics 的 SwordCleave / SwordThrust 系列。剑道 v2 五招本身不参与
/// "积累 → 自然绑定"（高阶招式默认已经在绑定语境下使用）。
pub fn sword_bond_tracking_system(
    mut commands: Commands,
    mut combat_events: EventReader<CombatEvent>,
    weapons: Query<&Weapon>,
    mut progress: Query<&mut SwordBondProgress>,
    bonds: Query<&SwordBondComponent>,
    mut bond_events: EventWriter<SwordBondFormedEvent>,
) {
    for event in combat_events.read() {
        let attacker = event.attacker;
        let Ok(weapon) = weapons.get(attacker) else {
            // 攻击者没有 Weapon component（赤手空拳、NPC 等）→ 不参与剑道追踪。
            continue;
        };
        if weapon.weapon_kind != WeaponKind::Sword {
            continue;
        }
        if !matches!(
            event.source,
            AttackSource::SwordCleave | AttackSource::SwordThrust
        ) {
            // 仅 sword_basics 直接命中算"连续使用剑术"；其他来源（远袭/AoE）不增计数。
            continue;
        }

        // 已绑定：拒绝第二绑定（plan §P1.1 决策）。仍允许积累 progress 不变。
        if bonds.get(attacker).is_ok() {
            continue;
        }

        // 命中目标无意义，但伤害必须 > 0（避免 parry / 无效命中也涨计数）。
        if event.damage <= 0.0 && event.physical_damage <= 0.0 {
            continue;
        }

        let weapon_entity = Entity::from_raw(weapon.instance_id as u32);
        if let Ok(mut prog) = progress.get_mut(attacker) {
            // 换剑 → 重置计数。仍记新的 tracked_weapon_entity。
            if prog.tracked_weapon_entity != weapon_entity {
                prog.tracked_weapon_entity = weapon_entity;
                prog.consecutive_uses = 1;
            } else {
                prog.consecutive_uses = prog.consecutive_uses.saturating_add(1);
            }
            if prog.consecutive_uses >= BOND_TRIGGER_USES {
                let bond = SwordBondComponent::new(weapon_entity);
                commands
                    .entity(attacker)
                    .insert(bond)
                    .remove::<SwordBondProgress>();
                bond_events.send(SwordBondFormedEvent {
                    player: attacker,
                    weapon: weapon_entity,
                });
            }
        } else {
            commands.entity(attacker).insert(SwordBondProgress {
                consecutive_uses: 1,
                tracked_weapon_entity: weapon_entity,
            });
        }
    }
}

/// P1.3 — 监听 `SwordShatterEvent`：扣减玩家 `Cultivation.qi_current` + 永久衰减
/// `qi_max`，剩余真元走 ledger 释放回所在 zone。
///
/// 守恒律：`stored_qi = backlash_qi_current + qi_released_to_zone`，由
/// `shatter::compute_shatter_outcome` 锁定。
pub fn sword_shatter_system(
    mut shatter_events: EventReader<SwordShatterEvent>,
    mut players: Query<&mut Cultivation>,
    mut qi_transfers: Option<ResMut<Events<QiTransfer>>>,
) {
    for event in shatter_events.read() {
        // 0.5 是确定性占位 roll —— "10% 概率结晶剑魂" 由独立的 spawn system 抽样决定，
        // 这里只关心真元守恒。
        let outcome = compute_shatter_outcome(event, 0.5);
        if let Ok(mut cultivation) = players.get_mut(event.player) {
            cultivation.qi_current =
                (cultivation.qi_current - outcome.backlash_qi_current).max(0.0);
            cultivation.qi_max = (cultivation.qi_max - outcome.backlash_qi_max_permanent).max(0.0);
            cultivation.qi_current = cultivation.qi_current.min(cultivation.qi_max);
        }
        if outcome.qi_released_to_zone > f64::EPSILON {
            if let Some(events) = qi_transfers.as_deref_mut() {
                if let Ok(transfer) = QiTransfer::new(
                    QiAccountId::container(format!("sword_bond:{:?}", event.weapon)),
                    QiAccountId::zone(DEFAULT_SPAWN_ZONE_NAME),
                    outcome.qi_released_to_zone,
                    QiTransferReason::ReleaseToZone,
                ) {
                    events.send(transfer);
                }
            }
        }
    }
}

/// P2.2 — 每 server tick 调用 `registry.tick_expire(current_tick)`，把过期盲区从
/// registry 中移除。盲区随化虚一剑开天产生，TTL 由 plan §techniques::effects
/// 锚定为 5 min（5 × 60 × 20 tick）。
pub fn tiandao_blind_zone_tick_system(
    clock: Res<CombatClock>,
    mut registry: ResMut<TiandaoBlindZoneRegistry>,
) {
    registry.tick_expire(clock.tick);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::components::{BodyPart, WoundKind};
    use crate::combat::events::AttackSource;
    use crate::combat::weapon::EquipSlot;
    use crate::cultivation::components::{Cultivation, Realm};
    use crate::sword_path::bond::{SwordBondComponent, SwordBondFormedEvent, SwordBondProgress};
    use crate::sword_path::grade::SwordGrade;
    use crate::sword_path::heaven_gate::TiandaoBlindZoneRegistry;
    use crate::sword_path::tiandao_blind::TiandaoBlindZone;
    use valence::prelude::{App, DVec3, Events, Update};

    fn spawn_sword_player(app: &mut App) -> Entity {
        app.world_mut()
            .spawn((Weapon {
                slot: EquipSlot::MainHand,
                instance_id: 42,
                template_id: "sword_iron".into(),
                weapon_kind: WeaponKind::Sword,
                base_attack: 10.0,
                quality_tier: 0,
                durability: 100.0,
                durability_max: 100.0,
            },))
            .id()
    }

    fn cleave_hit(attacker: Entity, target: Entity) -> CombatEvent {
        CombatEvent {
            attacker,
            target,
            resolved_at_tick: 1,
            body_part: BodyPart::Chest,
            wound_kind: WoundKind::Cut,
            source: AttackSource::SwordCleave,
            debug_command: false,
            physical_damage: 3.0,
            damage: 0.0,
            contam_delta: 0.0,
            description: "hit".into(),
            defense_kind: None,
            defense_effectiveness: None,
            defense_contam_reduced: None,
            defense_wound_severity: None,
        }
    }

    /// P1.1 happy-path: 连续 20 次 sword_basics 命中 → 挂上 SwordBondComponent，
    /// 同 tick 删除 SwordBondProgress 并发 SwordBondFormedEvent。
    #[test]
    fn bond_tracking_attaches_component_after_20_consecutive_hits() {
        let mut app = App::new();
        app.add_event::<CombatEvent>();
        app.add_event::<SwordBondFormedEvent>();
        app.add_systems(Update, sword_bond_tracking_system);

        let attacker = spawn_sword_player(&mut app);
        let target = app.world_mut().spawn_empty().id();

        for _ in 0..BOND_TRIGGER_USES {
            app.world_mut().send_event(cleave_hit(attacker, target));
            app.update();
        }

        let bond = app
            .world()
            .get::<SwordBondComponent>(attacker)
            .expect("bond should be inserted after BOND_TRIGGER_USES hits");
        assert_eq!(
            bond.grade,
            SwordGrade::Mortal,
            "新绑定默认 Mortal (plan §grade)"
        );
        assert!(
            app.world().get::<SwordBondProgress>(attacker).is_none(),
            "成功绑定后 progress 应被移除，避免无限计数"
        );
        let events = app.world().resource::<Events<SwordBondFormedEvent>>();
        assert!(
            !events.is_empty(),
            "至少应发 1 个 SwordBondFormedEvent，否则 narration/HUD 抓不到"
        );
    }

    /// P1.1 边界：19 次不绑定，不到阈值。
    #[test]
    fn bond_tracking_does_not_trigger_below_threshold() {
        let mut app = App::new();
        app.add_event::<CombatEvent>();
        app.add_event::<SwordBondFormedEvent>();
        app.add_systems(Update, sword_bond_tracking_system);

        let attacker = spawn_sword_player(&mut app);
        let target = app.world_mut().spawn_empty().id();

        for _ in 0..(BOND_TRIGGER_USES - 1) {
            app.world_mut().send_event(cleave_hit(attacker, target));
            app.update();
        }

        assert!(app.world().get::<SwordBondComponent>(attacker).is_none());
        let progress = app
            .world()
            .get::<SwordBondProgress>(attacker)
            .expect("progress 仍在累积");
        assert_eq!(progress.consecutive_uses, BOND_TRIGGER_USES - 1);
    }

    /// P1.1 拒绝第二绑定：已挂 SwordBondComponent 时不再积累 progress。
    #[test]
    fn bond_tracking_refuses_second_bond() {
        let mut app = App::new();
        app.add_event::<CombatEvent>();
        app.add_event::<SwordBondFormedEvent>();
        app.add_systems(Update, sword_bond_tracking_system);

        let attacker = spawn_sword_player(&mut app);
        let weapon_entity = Entity::from_raw(42);
        app.world_mut()
            .entity_mut(attacker)
            .insert(SwordBondComponent::new(weapon_entity));
        let target = app.world_mut().spawn_empty().id();

        for _ in 0..BOND_TRIGGER_USES {
            app.world_mut().send_event(cleave_hit(attacker, target));
            app.update();
        }

        // 仍然只有一个 bond，没新 progress 出现
        assert!(app.world().get::<SwordBondProgress>(attacker).is_none());
        let events = app.world().resource::<Events<SwordBondFormedEvent>>();
        assert!(events.is_empty(), "已绑定时不应再发 SwordBondFormedEvent");
    }

    /// P1.1 换剑重置：tracked_weapon_entity 不同 → 重新从 1 计数。
    #[test]
    fn bond_tracking_resets_on_weapon_swap() {
        let mut app = App::new();
        app.add_event::<CombatEvent>();
        app.add_event::<SwordBondFormedEvent>();
        app.add_systems(Update, sword_bond_tracking_system);

        let attacker = spawn_sword_player(&mut app);
        let target = app.world_mut().spawn_empty().id();
        // 先攻 5 次
        for _ in 0..5 {
            app.world_mut().send_event(cleave_hit(attacker, target));
            app.update();
        }
        let prog = app
            .world()
            .get::<SwordBondProgress>(attacker)
            .expect("progress exists");
        assert_eq!(prog.consecutive_uses, 5);

        // 换武器 instance_id（模拟玩家拔出另一把剑）
        if let Some(mut weapon) = app.world_mut().get_mut::<Weapon>(attacker) {
            weapon.instance_id = 99;
        }
        app.world_mut().send_event(cleave_hit(attacker, target));
        app.update();
        let prog = app
            .world()
            .get::<SwordBondProgress>(attacker)
            .expect("progress reset");
        assert_eq!(
            prog.consecutive_uses, 1,
            "换剑后应重置为 1，否则会用旧剑积累的次数绑定新剑（破 plan §P1.1 决策）"
        );
        assert_eq!(prog.tracked_weapon_entity, Entity::from_raw(99));
    }

    /// P1.1 非剑武器 / 非剑道招式来源 → 不累积。
    #[test]
    fn bond_tracking_ignores_non_sword_sources() {
        let mut app = App::new();
        app.add_event::<CombatEvent>();
        app.add_event::<SwordBondFormedEvent>();
        app.add_systems(Update, sword_bond_tracking_system);

        // 持剑但来源是 Melee（赤手）→ 不应累积
        let attacker = spawn_sword_player(&mut app);
        let target = app.world_mut().spawn_empty().id();
        let mut event = cleave_hit(attacker, target);
        event.source = AttackSource::Melee;
        app.world_mut().send_event(event);
        app.update();
        assert!(app.world().get::<SwordBondProgress>(attacker).is_none());

        // 持非剑武器 + SwordCleave 来源 → 也不应累积
        if let Some(mut weapon) = app.world_mut().get_mut::<Weapon>(attacker) {
            weapon.weapon_kind = WeaponKind::Spear;
        }
        app.world_mut().send_event(cleave_hit(attacker, target));
        app.update();
        assert!(app.world().get::<SwordBondProgress>(attacker).is_none());
    }

    /// P1.3 — shatter 守恒：stored_qi 总额 = qi_current 扣减 + zone 释放。
    #[test]
    fn shatter_system_conserves_qi_and_drops_qi_max() {
        let mut app = App::new();
        app.add_event::<SwordShatterEvent>();
        app.add_event::<QiTransfer>();
        app.add_systems(Update, sword_shatter_system);

        let player = app
            .world_mut()
            .spawn(Cultivation {
                realm: Realm::Solidify,
                qi_current: 200.0,
                qi_max: 200.0,
                ..Cultivation::default()
            })
            .id();
        app.world_mut().send_event(SwordShatterEvent {
            player,
            weapon: Entity::from_raw(7),
            stored_qi: 100.0,
            grade: SwordGrade::Solidified,
        });
        app.update();

        let cultivation = app.world().get::<Cultivation>(player).unwrap();
        // backlash_qi_current = 100 * 0.6 = 60 → qi_current 200 - 60 = 140
        assert!(
            (cultivation.qi_current - 140.0).abs() < 1e-6,
            "qi_current 应扣 60 反噬，实际：{}",
            cultivation.qi_current
        );
        // qi_max 永久衰减 100 * 0.05 = 5 → 195
        assert!(
            (cultivation.qi_max - 195.0).abs() < 1e-6,
            "qi_max 应永久 -5，实际：{}",
            cultivation.qi_max
        );

        let transfers = app.world().resource::<Events<QiTransfer>>();
        let transfer_count = transfers.iter_current_update_events().count();
        assert!(
            transfer_count >= 1,
            "释放回 zone 必须走 QiTransfer ledger（守恒律），实际 events 数={transfer_count}"
        );
    }

    /// P1.3 边界：stored_qi=0 → 不发 QiTransfer（避免噪声）。
    #[test]
    fn shatter_with_zero_stored_qi_does_not_emit_transfer() {
        let mut app = App::new();
        app.add_event::<SwordShatterEvent>();
        app.add_event::<QiTransfer>();
        app.add_systems(Update, sword_shatter_system);

        let player = app
            .world_mut()
            .spawn(Cultivation {
                realm: Realm::Condense,
                qi_current: 50.0,
                qi_max: 50.0,
                ..Cultivation::default()
            })
            .id();
        app.world_mut().send_event(SwordShatterEvent {
            player,
            weapon: Entity::from_raw(7),
            stored_qi: 0.0,
            grade: SwordGrade::Condensed,
        });
        app.update();

        let transfers = app.world().resource::<Events<QiTransfer>>();
        assert_eq!(
            transfers.iter_current_update_events().count(),
            0,
            "stored_qi=0 不应走 ledger transfer"
        );
        let c = app.world().get::<Cultivation>(player).unwrap();
        assert_eq!(c.qi_current, 50.0, "无反噬");
        assert_eq!(c.qi_max, 50.0, "无 qi_max 衰减");
    }

    /// P2.2 — tick system 清理过期盲区。
    #[test]
    fn blind_zone_tick_expires_old_zones() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 0 });
        app.insert_resource(TiandaoBlindZoneRegistry::default());
        app.add_systems(Update, tiandao_blind_zone_tick_system);

        {
            let mut registry = app.world_mut().resource_mut::<TiandaoBlindZoneRegistry>();
            registry.add(TiandaoBlindZone {
                center: DVec3::ZERO,
                radius: 50.0,
                ttl_ticks: 100,
                created_tick: 0,
            });
            registry.add(TiandaoBlindZone {
                center: DVec3::new(1000.0, 0.0, 0.0),
                radius: 50.0,
                ttl_ticks: 1000,
                created_tick: 0,
            });
        }

        app.world_mut().resource_mut::<CombatClock>().tick = 150;
        app.update();

        let registry = app.world().resource::<TiandaoBlindZoneRegistry>();
        assert_eq!(
            registry.active_count(),
            1,
            "tick=150 时 ttl=100 的盲区应过期，ttl=1000 的盲区保留"
        );
    }

    /// P2.2 — tick=0 时不应清理任何盲区（边界）。
    #[test]
    fn blind_zone_tick_keeps_fresh_zones() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 0 });
        app.insert_resource(TiandaoBlindZoneRegistry::default());
        app.add_systems(Update, tiandao_blind_zone_tick_system);

        app.world_mut()
            .resource_mut::<TiandaoBlindZoneRegistry>()
            .add(TiandaoBlindZone {
                center: DVec3::ZERO,
                radius: 50.0,
                ttl_ticks: 1000,
                created_tick: 0,
            });

        app.update();
        assert_eq!(
            app.world()
                .resource::<TiandaoBlindZoneRegistry>()
                .active_count(),
            1
        );
    }
}
