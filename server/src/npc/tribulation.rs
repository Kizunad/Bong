//! NPC 渡虚劫自动化（plan §3.2 Phase 3）。
//!
//! 玩家渡劫依赖 calamity agent 脚本化多波次 + 战斗 plan 施加伤害。NPC 简化版：
//!   1. `AscensionQuotaStore`：全服并发名额上限，避免同时一窝 NPC 化虚
//!   2. `NpcTribulationPacing`：每 NPC 的波次节奏器（定时 fire `TribulationWaveCleared`）
//!   3. 心魔劫自动走"坚心"默认（plan §8 已决定）—— 本文件对此不做额外判定，
//!      波次自动推进即等价于"坚心未被撼动"
//!
//! 出入口：
//!   * `try_reserve_npc_tribulation(entity)` — `StartDuXuAction` 在起劫前调用
//!   * `npc_tribulation_auto_wave_tick` — 每 tick 推进波次
//!   * `release_quota_for_ended_tribulations` — 结束后释放名额（成功/失败/取消）

use std::collections::HashSet;

use valence::prelude::{
    bevy_ecs, App, Component, Entity, EventWriter, IntoSystemConfigs, Query, ResMut, Resource,
    Update, With,
};

use crate::cultivation::tribulation::{TribulationState, TribulationWaveCleared};
use crate::npc::spawn::NpcMarker;

/// 全服并发渡虚劫名额。首版 4，可被 config/agent 调整。
#[derive(Debug, Clone, Resource)]
pub struct AscensionQuotaStore {
    pub max_concurrent: u8,
    pub active: HashSet<Entity>,
}

impl Default for AscensionQuotaStore {
    fn default() -> Self {
        Self {
            max_concurrent: 4,
            active: HashSet::new(),
        }
    }
}

impl AscensionQuotaStore {
    pub fn try_reserve(&mut self, entity: Entity) -> bool {
        if self.active.contains(&entity) {
            return true; // 已占用
        }
        if self.active.len() >= self.max_concurrent as usize {
            return false;
        }
        self.active.insert(entity);
        true
    }

    pub fn release(&mut self, entity: Entity) {
        self.active.remove(&entity);
    }

    #[allow(dead_code)]
    pub fn is_reserved(&self, entity: Entity) -> bool {
        self.active.contains(&entity)
    }

    #[allow(dead_code)]
    pub fn active_count(&self) -> usize {
        self.active.len()
    }
}

/// 每 NPC 的波次节拍器：`ticks_per_wave` 控制"天劫密度"。
#[derive(Debug, Clone, Copy, Component)]
pub struct NpcTribulationPacing {
    pub ticks_per_wave: u32,
    pub ticks_since_last_wave: u32,
}

impl Default for NpcTribulationPacing {
    fn default() -> Self {
        Self {
            ticks_per_wave: 100,
            ticks_since_last_wave: 0,
        }
    }
}

pub fn register(app: &mut App) {
    app.insert_resource(AscensionQuotaStore::default())
        .add_systems(
            Update,
            (
                npc_tribulation_auto_wave_tick,
                release_quota_for_ended_tribulations.after(npc_tribulation_auto_wave_tick),
            ),
        );
}

#[allow(clippy::type_complexity)]
pub(crate) fn npc_tribulation_auto_wave_tick(
    mut npcs: Query<(Entity, &TribulationState, &mut NpcTribulationPacing), With<NpcMarker>>,
    mut cleared: EventWriter<TribulationWaveCleared>,
) {
    for (entity, state, mut pacing) in &mut npcs {
        pacing.ticks_since_last_wave = pacing.ticks_since_last_wave.saturating_add(1);
        if pacing.ticks_since_last_wave < pacing.ticks_per_wave {
            continue;
        }
        pacing.ticks_since_last_wave = 0;
        // 下一波次号：从 1 起（plan §3.2 "wave_current.max(ev.wave)"）。
        let next_wave = state.wave_current.saturating_add(1);
        cleared.send(TribulationWaveCleared {
            entity,
            wave: next_wave,
        });
    }
}

/// 结束后释放配额：任何挂在 `AscensionQuotaStore.active` 但已没有
/// `TribulationState` 的 NPC 都会被释放（涵盖成功进 Void / 失败 / 被取消）。
pub(crate) fn release_quota_for_ended_tribulations(
    mut store: ResMut<AscensionQuotaStore>,
    ongoing: Query<Entity, (With<NpcMarker>, With<TribulationState>)>,
) {
    if store.active.is_empty() {
        return;
    }
    let still_tribulating: HashSet<Entity> = ongoing.iter().collect();
    let to_release: Vec<Entity> = store
        .active
        .iter()
        .copied()
        .filter(|entity| !still_tribulating.contains(entity))
        .collect();
    for entity in to_release {
        store.release(entity);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use valence::prelude::{App, Update};

    fn mk_entity(app: &mut App) -> Entity {
        app.world_mut().spawn(NpcMarker).id()
    }

    #[test]
    fn quota_reserves_up_to_max_then_rejects() {
        let mut store = AscensionQuotaStore {
            max_concurrent: 2,
            ..AscensionQuotaStore::default()
        };
        let mut app = App::new();
        let e1 = mk_entity(&mut app);
        let e2 = mk_entity(&mut app);
        let e3 = mk_entity(&mut app);
        assert!(store.try_reserve(e1));
        assert!(store.try_reserve(e2));
        assert!(!store.try_reserve(e3));
        assert_eq!(store.active_count(), 2);
    }

    #[test]
    fn quota_reserving_same_entity_is_idempotent() {
        let mut store = AscensionQuotaStore::default();
        let mut app = App::new();
        let e = mk_entity(&mut app);
        assert!(store.try_reserve(e));
        assert!(store.try_reserve(e));
        assert_eq!(store.active_count(), 1);
    }

    #[test]
    fn quota_release_frees_capacity() {
        let mut store = AscensionQuotaStore {
            max_concurrent: 1,
            ..AscensionQuotaStore::default()
        };
        let mut app = App::new();
        let e1 = mk_entity(&mut app);
        let e2 = mk_entity(&mut app);
        assert!(store.try_reserve(e1));
        assert!(!store.try_reserve(e2));
        store.release(e1);
        assert!(store.try_reserve(e2));
    }

    #[test]
    fn npc_tribulation_auto_wave_tick_fires_at_interval() {
        use crate::cultivation::tribulation::TribulationState;

        let mut app = App::new();
        app.add_event::<TribulationWaveCleared>();
        app.add_systems(Update, npc_tribulation_auto_wave_tick);

        let entity = app
            .world_mut()
            .spawn((
                NpcMarker,
                TribulationState::restored(0, 3, 0),
                NpcTribulationPacing {
                    ticks_per_wave: 3,
                    ticks_since_last_wave: 0,
                },
            ))
            .id();

        // 前 2 tick 不 fire
        app.update();
        app.update();
        {
            let events = app
                .world()
                .resource::<bevy_ecs::event::Events<TribulationWaveCleared>>();
            assert_eq!(events.iter_current_update_events().count(), 0);
        }
        // 第 3 tick fire 第一波
        app.update();
        {
            let events = app
                .world()
                .resource::<bevy_ecs::event::Events<TribulationWaveCleared>>();
            let all: Vec<_> = events.iter_current_update_events().cloned().collect();
            assert_eq!(all.len(), 1);
            assert_eq!(all[0].entity, entity);
            assert_eq!(all[0].wave, 1);
        }
    }

    #[test]
    fn release_quota_when_tribulation_state_gone() {
        let mut app = App::new();
        app.insert_resource(AscensionQuotaStore::default());
        app.add_systems(Update, release_quota_for_ended_tribulations);

        let entity = app.world_mut().spawn(NpcMarker).id();
        app.world_mut()
            .resource_mut::<AscensionQuotaStore>()
            .active
            .insert(entity);

        app.update();

        assert_eq!(
            app.world().resource::<AscensionQuotaStore>().active_count(),
            0,
            "NPC without TribulationState should be released"
        );
    }
}
