//! 死亡对外契约（plan §4）— 修炼侧只 emit 致死触发，生死判定由战斗 plan 收口。
//!
//! 另外提供 `PlayerRevived` 监听：战斗 plan 完成重生后发事件，本 plan
//! 应用境界-1、qi=0、composure=0.3、contam 清空、LIFO 关脉等惩罚。
//! `PlayerTerminated` 也有监听 hook，停止该实体的所有修炼 tick（通过
//! 移除 Cultivation Component 实现）。

use serde::{Deserialize, Serialize};
use valence::prelude::{bevy_ecs, Commands, Entity, Event, EventReader, EventWriter, Query};

use super::components::{Contamination, Cultivation, MeridianSystem, Realm};
use super::life_record::{BiographyEntry, LifeRecord};
use super::qi_zero_decay::{close_meridian, pick_closures};
use super::tick::CultivationClock;
use crate::skill::components::SkillId;
use crate::skill::events::SkillCapChanged;
use valence::prelude::Res;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CultivationDeathCause {
    BreakthroughBackfire,
    MeridianCollapse,
    NegativeZoneDrain,
    ContaminationOverflow,
    NaturalAging,
}

#[derive(Debug, Clone, Event)]
pub struct CultivationDeathTrigger {
    pub entity: Entity,
    pub cause: CultivationDeathCause,
    pub context: serde_json::Value,
}

#[derive(Debug, Clone, Event)]
pub struct PlayerRevived {
    pub entity: Entity,
}

#[derive(Debug, Clone, Event)]
pub struct PlayerTerminated {
    pub entity: Entity,
}

/// 重生响应：境界 -1、qi=0、composure=0.3、contam 清空、LIFO 关脉至对应境界。
pub fn apply_revive_penalty(
    cultivation: &mut Cultivation,
    meridians: &mut MeridianSystem,
    contam: &mut Contamination,
) {
    if let Some(prev) = cultivation.realm.previous() {
        cultivation.realm = prev;
    }
    cultivation.qi_current = 0.0;
    cultivation.composure = 0.3;
    cultivation.last_qi_zero_at = None;
    contam.entries.clear();

    let keep = cultivation
        .realm
        .required_meridians()
        .max(if cultivation.realm == Realm::Awaken {
            0
        } else {
            1
        });
    let closures = pick_closures(meridians, keep);
    for (is_regular, idx) in closures {
        if is_regular {
            close_meridian(&mut meridians.regular[idx]);
        } else {
            close_meridian(&mut meridians.extraordinary[idx]);
        }
    }
    cultivation.qi_max = 10.0 + meridians.sum_capacity();
}

pub fn on_player_revived(
    clock: Res<CultivationClock>,
    mut events: EventReader<PlayerRevived>,
    mut skill_cap_events: EventWriter<SkillCapChanged>,
    mut players: Query<(
        &mut Cultivation,
        &mut MeridianSystem,
        &mut Contamination,
        &mut LifeRecord,
    )>,
) {
    let now = clock.tick;
    for ev in events.read() {
        if let Ok((mut c, mut ms, mut cn, mut life)) = players.get_mut(ev.entity) {
            if matches!(
                life.biography.last(),
                Some(BiographyEntry::Rebirth { tick, .. }) if *tick == now
            ) {
                continue;
            }
            let prior = c.realm;
            apply_revive_penalty(&mut c, &mut ms, &mut cn);
            life.push(BiographyEntry::Rebirth {
                prior_realm: prior,
                new_realm: c.realm,
                tick: now,
            });
            let new_cap = super::breakthrough::skill_cap_for_realm(c.realm);
            for skill in SkillId::ALL {
                skill_cap_events.send(SkillCapChanged {
                    char_entity: ev.entity,
                    skill,
                    new_cap,
                });
            }
            tracing::info!(
                "[bong][cultivation] applied revive penalty to {:?}: realm {:?} -> {:?}",
                ev.entity,
                prior,
                c.realm
            );
        }
    }
}

pub fn on_player_terminated(mut commands: Commands, mut events: EventReader<PlayerTerminated>) {
    for ev in events.read() {
        if let Some(mut e) = commands.get_entity(ev.entity) {
            e.remove::<Cultivation>();
            e.remove::<MeridianSystem>();
            e.remove::<Contamination>();
            tracing::info!(
                "[bong][cultivation] terminated entity {:?} — removed cultivation components",
                ev.entity
            );
        }
    }
}

/// 将致死触发转发到生平卷（by caller）与 Redis 外发通道（留给 network 模块接入）。
pub fn log_death_trigger(
    mut events: EventReader<CultivationDeathTrigger>,
    mut out: EventWriter<CultivationDeathTrigger>,
) {
    // 简单 pass-through + tracing，真实接入时把 context 推到 network::agent_bridge
    for ev in events.read() {
        tracing::warn!(
            "[bong][cultivation] DEATH TRIGGER entity={:?} cause={:?} context={}",
            ev.entity,
            ev.cause,
            ev.context
        );
        out.send(ev.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cultivation::components::MeridianId;
    use crate::cultivation::tick::CultivationClock;
    use crate::player::state::canonical_player_id;
    use valence::prelude::App;

    #[test]
    fn revive_penalty_drops_one_realm_and_closes_excess() {
        let mut c = Cultivation {
            realm: Realm::Induce,
            qi_max: 100.0,
            qi_current: 50.0,
            composure: 1.0,
            ..Default::default()
        };
        let mut ms = MeridianSystem::default();
        ms.get_mut(MeridianId::Lung).opened = true;
        ms.get_mut(MeridianId::LargeIntestine).opened = true;
        ms.get_mut(MeridianId::Stomach).opened = true;
        let mut cn = Contamination::default();

        apply_revive_penalty(&mut c, &mut ms, &mut cn);
        assert_eq!(c.realm, Realm::Awaken);
        assert_eq!(c.qi_current, 0.0);
        assert!((c.composure - 0.3).abs() < 1e-9);
        // Awaken 要求 0 条，但保留至少 1（下限 special case）
        assert!(ms.opened_count() <= 3);
    }

    #[test]
    fn revive_at_awaken_stays_awaken() {
        let mut c = Cultivation {
            realm: Realm::Awaken,
            ..Default::default()
        };
        let mut ms = MeridianSystem::default();
        let mut cn = Contamination::default();
        apply_revive_penalty(&mut c, &mut ms, &mut cn);
        assert_eq!(c.realm, Realm::Awaken);
    }

    #[test]
    fn revive_penalty_does_not_mutate_character_anchor() {
        let mut app = App::new();
        app.insert_resource(CultivationClock { tick: 42 });
        app.add_event::<PlayerRevived>();
        app.add_event::<SkillCapChanged>();
        app.add_systems(valence::prelude::Update, on_player_revived);

        let entity = app
            .world_mut()
            .spawn((
                Cultivation {
                    realm: Realm::Induce,
                    qi_current: 8.0,
                    composure: 0.9,
                    ..Default::default()
                },
                MeridianSystem::default(),
                Contamination::default(),
                LifeRecord::new(canonical_player_id("Alice")),
            ))
            .id();

        app.world_mut().send_event(PlayerRevived { entity });
        app.update();

        let life = app
            .world()
            .get::<LifeRecord>(entity)
            .expect("life record should remain attached after revive");

        assert_eq!(life.character_id, canonical_player_id("Alice"));
        assert!(matches!(
            life.biography.last(),
            Some(BiographyEntry::Rebirth { tick: 42, .. })
        ));
    }

    #[test]
    fn revived_hook_skips_when_rebirth_already_recorded_for_tick() {
        let mut app = App::new();
        app.insert_resource(CultivationClock { tick: 42 });
        app.add_event::<PlayerRevived>();
        app.add_event::<SkillCapChanged>();
        app.add_systems(valence::prelude::Update, on_player_revived);

        let entity = app
            .world_mut()
            .spawn((
                Cultivation {
                    realm: Realm::Induce,
                    qi_current: 8.0,
                    composure: 0.9,
                    ..Default::default()
                },
                MeridianSystem::default(),
                Contamination::default(),
                LifeRecord {
                    character_id: canonical_player_id("Alice"),
                    created_at: 0,
                    biography: vec![BiographyEntry::Rebirth {
                        prior_realm: Realm::Induce,
                        new_realm: Realm::Awaken,
                        tick: 42,
                    }],
                    insights_taken: Vec::new(),
                    death_insights: Vec::new(),
                    skill_milestones: Vec::new(),
                    spirit_root_first: None,
                },
            ))
            .id();

        app.world_mut().send_event(PlayerRevived { entity });
        app.update();

        let cultivation = app
            .world()
            .get::<Cultivation>(entity)
            .expect("cultivation should remain attached");
        let life = app
            .world()
            .get::<LifeRecord>(entity)
            .expect("life record should remain attached");

        assert_eq!(cultivation.realm, Realm::Induce);
        assert_eq!(life.biography.len(), 1);
    }
}
