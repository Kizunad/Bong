//! 采集 HUD 与临时手持外观场景；计时、移动/受击打断均走正式采集系统。

use std::collections::HashMap;

use valence::prelude::bevy_ecs::system::SystemParam;
use valence::prelude::{
    bevy_ecs, Client, DetectChanges, Entity, Events, Local, Position, Query, Ref, Res, ResMut,
    Resource, With,
};

use crate::botany::components::HarvestSessionStore;
use crate::combat::components::{Lifecycle, LifecycleState, TICKS_PER_SECOND};
use crate::cultivation::components::Realm;
use crate::gathering::session::{
    GatheringProgressFrame, GatheringSession, GatheringSessionStart, GatheringSessionStore,
};
use crate::gathering::tools::GatheringTargetKind;
use crate::inventory::{ItemRegistry, PlayerInventory};
use crate::network::weapon_equipped_emit::{equipped_slot_view, send_weapon_equipped};
use crate::player::gameplay::GameplayTick;
use crate::schema::combat_hud::WeaponViewV1;
use crate::spiritwood::session::WoodSessionStore;

#[derive(Default)]
pub(super) struct GatheringSceneState {
    sessions: HashMap<Entity, String>,
    sequence: u64,
}

impl Resource for GatheringSceneState {}

#[derive(SystemParam)]
pub(super) struct GatheringSceneContext<'w> {
    state: ResMut<'w, GatheringSceneState>,
    store: Option<ResMut<'w, GatheringSessionStore>>,
    frames: Option<ResMut<'w, Events<GatheringProgressFrame>>>,
    tick: Option<Res<'w, GameplayTick>>,
    herbs: Option<Res<'w, HarvestSessionStore>>,
    wood: Option<Res<'w, WoodSessionStore>>,
}

impl GatheringSceneContext<'_> {
    pub fn start(
        &mut self,
        player: Entity,
        target: GatheringTargetKind,
        target_name: &str,
        position: &Position,
    ) -> Result<(), &'static str> {
        let (Some(store), Some(_), Some(tick)) = (&self.store, &self.frames, &self.tick) else {
            return Err("采集系统尚未就绪。");
        };
        if self.has_specialized_session(player)
            || store.session_for(player).is_some_and(|session| {
                self.state.sessions.get(&player) != Some(&session.session_id)
            })
        {
            return Err("请先结束当前的正常采集，再加载测试场景。");
        }
        let now = tick.current_tick();
        self.clear(player);
        self.state.sequence += 1;
        let session_id = format!(
            "scene:gathering:{}:{}",
            player.to_bits(),
            self.state.sequence
        );
        let mut session = GatheringSession::new(GatheringSessionStart {
            player,
            session_id: session_id.clone(),
            target,
            target_name: target_name.to_string(),
            started_at_tick: now,
            origin_position: position.get().to_array(),
            tool: None,
            realm: Realm::Awaken,
            auto_complete: true,
        });
        // 固定展示时长，不依赖玩家装备/境界，也不消耗真实工具耐久。
        session.total_ticks = 8 * TICKS_PER_SECOND;
        self.frames
            .as_mut()
            .unwrap()
            .send(session.progress_frame(now, false, false));
        self.store.as_mut().unwrap().upsert(session);
        self.state.sessions.insert(player, session_id);
        Ok(())
    }

    fn has_specialized_session(&self, player: Entity) -> bool {
        self.herbs.as_ref().is_some_and(|store| {
            store
                .iter()
                .any(|session| session.client_entity == player && !session.is_terminal())
        }) || self
            .wood
            .as_ref()
            .is_some_and(|store| store.session_for(player).is_some())
    }

    pub fn clear(&mut self, player: Entity) {
        let Some(id) = self.state.sessions.remove(&player) else {
            return;
        };
        let (Some(store), Some(frames)) = (&mut self.store, &mut self.frames) else {
            return;
        };
        // 会话可能已结束或被正常玩法接管，只能清理仍属于场景的那一段。
        if store
            .session_for(player)
            .is_some_and(|session| session.session_id == id)
        {
            let session = store.remove(player).unwrap();
            let now = self.tick.as_ref().map_or(0, |tick| tick.current_tick());
            frames.send(session.progress_frame(now, true, false));
        }
    }
}

pub(super) fn cleanup_scenes(
    mut scene: GatheringSceneContext<'_>,
    players: Query<&Lifecycle, With<Client>>,
) {
    let expired: Vec<Entity> = scene
        .state
        .sessions
        .iter()
        .filter_map(|(player, id)| {
            let active = scene
                .store
                .as_ref()
                .and_then(|store| store.session_for(*player));
            let alive = players
                .get(*player)
                .is_ok_and(|life| life.state == LifecycleState::Alive);
            (!alive
                || scene.has_specialized_session(*player)
                || active.is_none_or(|session| &session.session_id != id))
            .then_some(*player)
        })
        .collect();
    for player in expired {
        scene.clear(player);
    }
}

/// 只覆盖手持视图，不改真实背包；结束后重新读取当前装备，避免还原过时快照。
pub(super) fn sync_scene_tools(
    scene: Res<GatheringSceneState>,
    store: Option<Res<GatheringSessionStore>>,
    registry: Res<ItemRegistry>,
    mut players: Query<(Entity, &mut Client, Option<Ref<PlayerInventory>>)>,
    mut shown: Local<HashMap<Entity, String>>,
) {
    shown.retain(|entity, _| players.get(*entity).is_ok());
    for (entity, mut client, inventory) in &mut players {
        let session = store
            .as_ref()
            .and_then(|store| store.session_for(entity))
            .filter(|session| scene.sessions.get(&entity) == Some(&session.session_id));
        if let Some(session) = session {
            if shown.get(&entity) == Some(&session.session_id)
                && !inventory
                    .as_ref()
                    .is_some_and(|inventory| inventory.is_changed())
            {
                continue;
            }
            let template = match session.target {
                GatheringTargetKind::Herb => "hoe_iron",
                GatheringTargetKind::Ore => "pickaxe_iron",
                GatheringTargetKind::Wood => "axe_iron",
            };
            send_weapon_equipped(
                &mut client,
                "main_hand",
                Some(WeaponViewV1 {
                    instance_id: 0,
                    template_id: template.to_string(),
                    weapon_kind: "tool".to_string(),
                    durability_current: 1.0,
                    durability_max: 1.0,
                    quality_tier: 0,
                }),
            );
            shown.insert(entity, session.session_id.clone());
        } else if shown.remove(&entity).is_some() {
            let restored = inventory
                .as_ref()
                .and_then(|inventory| equipped_slot_view(inventory, &registry, "main_hand"));
            send_weapon_equipped(&mut client, "main_hand", restored);
        }
    }
}
