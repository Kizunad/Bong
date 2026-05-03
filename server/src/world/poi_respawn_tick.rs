//! plan-poi-novice-v1 — 新手 POI 刷新周期。

use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use valence::prelude::{bevy_ecs, App, Res, ResMut, Resource, Update};

use crate::combat::CombatClock;

use super::poi_novice::{PoiNoviceKind, PoiNoviceRegistry};

pub const SERVER_TICKS_PER_SECOND: u64 = 20;
pub const MUTANT_NEST_RESPAWN_TICKS: u64 = 24 * 60 * 60 * SERVER_TICKS_PER_SECOND;
pub const ROGUE_NPC_RESPAWN_TICKS: u64 = MUTANT_NEST_RESPAWN_TICKS;
pub const SCROLL_REFRESH_SECONDS: u64 = 7 * 24 * 60 * 60;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PoiRespawnState {
    pub poi_id: String,
    pub kind: PoiNoviceKind,
    pub last_server_tick: u64,
    pub last_wall_clock_secs: u64,
}

impl PoiRespawnState {
    pub fn is_server_tick_ready(&self, now_tick: u64) -> bool {
        match self.kind {
            PoiNoviceKind::MutantNest => {
                now_tick.saturating_sub(self.last_server_tick) >= MUTANT_NEST_RESPAWN_TICKS
            }
            PoiNoviceKind::RogueVillage => {
                now_tick.saturating_sub(self.last_server_tick) >= ROGUE_NPC_RESPAWN_TICKS
            }
            _ => false,
        }
    }

    pub fn is_real_time_ready(&self, now_wall_clock_secs: u64) -> bool {
        self.kind == PoiNoviceKind::ScrollHidden
            && now_wall_clock_secs.saturating_sub(self.last_wall_clock_secs)
                >= SCROLL_REFRESH_SECONDS
    }
}

#[derive(Debug, Default, Resource)]
pub struct PoiRespawnStore {
    states: HashMap<String, PoiRespawnState>,
}

impl PoiRespawnStore {
    pub fn ensure_site(&mut self, poi_id: impl Into<String>, kind: PoiNoviceKind) {
        let poi_id = poi_id.into();
        self.states
            .entry(poi_id.clone())
            .or_insert(PoiRespawnState {
                poi_id,
                kind,
                last_server_tick: 0,
                last_wall_clock_secs: 0,
            });
    }

    pub fn get(&self, poi_id: &str) -> Option<&PoiRespawnState> {
        self.states.get(poi_id)
    }

    pub fn mark_refreshed(&mut self, poi_id: &str, now_tick: u64, now_wall_clock_secs: u64) {
        if let Some(state) = self.states.get_mut(poi_id) {
            state.last_server_tick = now_tick;
            state.last_wall_clock_secs = now_wall_clock_secs;
        }
    }

    pub fn ready_ids(&self, now_tick: u64, now_wall_clock_secs: u64) -> Vec<String> {
        let mut ids = self
            .states
            .values()
            .filter(|state| {
                state.is_server_tick_ready(now_tick)
                    || state.is_real_time_ready(now_wall_clock_secs)
            })
            .map(|state| state.poi_id.clone())
            .collect::<Vec<_>>();
        ids.sort();
        ids
    }
}

pub fn register(app: &mut App) {
    app.init_resource::<PoiRespawnStore>()
        .add_systems(Update, respawn_tick);
}

pub fn respawn_tick(
    registry: Option<Res<PoiNoviceRegistry>>,
    clock: Option<Res<CombatClock>>,
    mut store: ResMut<PoiRespawnStore>,
) {
    let Some(registry) = registry else {
        return;
    };
    for site in registry.sites() {
        store.ensure_site(site.id.clone(), site.kind);
    }
    let now_tick = clock.as_ref().map_or(0, |clock| clock.tick);
    let now_wall_clock_secs = current_wall_clock_secs();
    for poi_id in store.ready_ids(now_tick, now_wall_clock_secs) {
        if let Some(state) = store.get(&poi_id) {
            tracing::info!(
                "[bong][poi-novice] poi={} kind={:?} refresh ready",
                state.poi_id,
                state.kind
            );
        }
        store.mark_refreshed(&poi_id, now_tick, now_wall_clock_secs);
    }
}

fn current_wall_clock_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mutant_and_rogue_refresh_on_twenty_four_hours_of_server_ticks() {
        let mutant = PoiRespawnState {
            poi_id: "spawn:mutant_nest".to_string(),
            kind: PoiNoviceKind::MutantNest,
            last_server_tick: 100,
            last_wall_clock_secs: 0,
        };
        assert!(!mutant.is_server_tick_ready(100 + MUTANT_NEST_RESPAWN_TICKS - 1));
        assert!(mutant.is_server_tick_ready(100 + MUTANT_NEST_RESPAWN_TICKS));

        let rogue = PoiRespawnState {
            kind: PoiNoviceKind::RogueVillage,
            ..mutant.clone()
        };
        assert!(rogue.is_server_tick_ready(100 + ROGUE_NPC_RESPAWN_TICKS));
    }

    #[test]
    fn scroll_cache_refreshes_on_one_week_real_time() {
        let scroll = PoiRespawnState {
            poi_id: "spawn:scroll_hidden".to_string(),
            kind: PoiNoviceKind::ScrollHidden,
            last_server_tick: 0,
            last_wall_clock_secs: 10,
        };
        assert!(!scroll.is_real_time_ready(10 + SCROLL_REFRESH_SECONDS - 1));
        assert!(scroll.is_real_time_ready(10 + SCROLL_REFRESH_SECONDS));
    }

    #[test]
    fn store_reports_ready_ids_by_kind_clock() {
        let mut store = PoiRespawnStore::default();
        store.ensure_site("spawn:mutant_nest", PoiNoviceKind::MutantNest);
        store.ensure_site("spawn:scroll_hidden", PoiNoviceKind::ScrollHidden);
        let ready = store.ready_ids(MUTANT_NEST_RESPAWN_TICKS, SCROLL_REFRESH_SECONDS);
        assert_eq!(ready, vec!["spawn:mutant_nest", "spawn:scroll_hidden"]);
    }
}
