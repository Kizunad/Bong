use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::json;
use valence::prelude::{
    bevy_ecs, App, Bundle, Commands, Component, DVec3, Despawned, Entity, Event, EventReader,
    EventWriter, IntoSystemConfigs, Position, PreUpdate, Query, Res, ResMut, Resource, Update,
    With, Without,
};

use crate::combat::components::{
    CombatState, DerivedAttrs, Lifecycle, LifecycleState, Stamina, StatusEffects, Wounds,
};
use crate::cultivation::components::{Contamination, Cultivation, MeridianSystem};
use crate::cultivation::death_hooks::{
    CultivationDeathCause, CultivationDeathTrigger, PlayerTerminated,
};
use crate::npc::brain::canonical_npc_id;
use crate::npc::spawn::NpcMarker;

type RegistryNpcQuery<'w, 's> = Query<
    'w,
    's,
    (&'static NpcArchetype, Option<&'static Lifecycle>),
    (With<NpcMarker>, Without<Despawned>),
>;

type AgingNpcQuery<'w, 's> = Query<
    'w,
    's,
    &'static mut NpcLifespan,
    (
        With<NpcMarker>,
        Without<Despawned>,
        Without<PendingRetirement>,
    ),
>;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize, Component)]
#[serde(rename_all = "snake_case")]
pub enum NpcArchetype {
    #[default]
    Zombie,
    Commoner,
    Rogue,
    Beast,
    Disciple,
    GuardianRelic,
}

impl NpcArchetype {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Zombie => "zombie",
            Self::Commoner => "commoner",
            Self::Rogue => "rogue",
            Self::Beast => "beast",
            Self::Disciple => "disciple",
            Self::GuardianRelic => "guardian_relic",
        }
    }

    pub const fn default_max_age_ticks(self) -> f64 {
        match self {
            Self::Zombie => 120_000.0,
            Self::Commoner => 90_000.0,
            Self::Rogue => 110_000.0,
            Self::Beast => 80_000.0,
            Self::Disciple => 140_000.0,
            Self::GuardianRelic => 1_000_000.0,
        }
    }
}

#[derive(Clone, Copy, Debug, Component, Serialize, Deserialize)]
pub struct NpcLifespan {
    pub age_ticks: f64,
    pub max_age_ticks: f64,
}

impl NpcLifespan {
    pub const fn new(age_ticks: f64, max_age_ticks: f64) -> Self {
        Self {
            age_ticks,
            max_age_ticks,
        }
    }

    pub fn for_archetype(archetype: NpcArchetype) -> Self {
        Self::new(0.0, archetype.default_max_age_ticks())
    }

    pub fn age_ratio(&self) -> f64 {
        if self.max_age_ticks <= f64::EPSILON {
            1.0
        } else {
            (self.age_ticks / self.max_age_ticks).clamp(0.0, 16.0)
        }
    }

    pub fn is_expired(&self) -> bool {
        self.age_ticks >= self.max_age_ticks
    }
}

#[derive(Clone, Copy, Debug, Resource)]
pub struct NpcAgingConfig {
    pub enabled: bool,
    pub rate_multiplier: f64,
}

impl Default for NpcAgingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            rate_multiplier: 0.3,
        }
    }
}

#[derive(Clone, Debug, Resource)]
pub struct NpcRegistry {
    pub live_npc_count: usize,
    pub max_npc_count: usize,
    pub resume_npc_count: usize,
    pub spawn_paused: bool,
    pub counts_by_archetype: HashMap<NpcArchetype, usize>,
}

impl Default for NpcRegistry {
    fn default() -> Self {
        Self {
            live_npc_count: 0,
            max_npc_count: 512,
            resume_npc_count: 460,
            spawn_paused: false,
            counts_by_archetype: HashMap::new(),
        }
    }
}

impl NpcRegistry {
    pub fn refresh_from_counts(
        &mut self,
        live_npc_count: usize,
        counts_by_archetype: HashMap<NpcArchetype, usize>,
    ) {
        self.live_npc_count = live_npc_count;
        self.counts_by_archetype = counts_by_archetype;

        if self.live_npc_count >= self.max_npc_count {
            self.spawn_paused = true;
        } else if self.live_npc_count < self.resume_npc_count {
            self.spawn_paused = false;
        }
    }

    pub fn reserve_spawn_batch(&mut self, desired: usize) -> usize {
        if desired == 0 {
            return 0;
        }

        if self.spawn_paused && self.live_npc_count >= self.resume_npc_count {
            return 0;
        }

        let remaining = self.max_npc_count.saturating_sub(self.live_npc_count);
        let granted = desired.min(remaining);
        self.live_npc_count = self.live_npc_count.saturating_add(granted);
        if self.live_npc_count >= self.max_npc_count {
            self.spawn_paused = true;
        }
        granted
    }

    /// 回滚已 reserve 但未实际落盘的配额。用于"先 reserve 再决定能否 spawn"
    /// 路径在早退分支未回退导致的 1-tick 暂态泄漏 —— 这一 tick 里
    /// `live_npc_count >= resume_npc_count` 会误触发 `spawn_paused=true`，
    /// 同 tick 后续 spawn 分支被误杀。
    pub fn release_spawn_batch(&mut self, count: usize) {
        if count == 0 {
            return;
        }
        self.live_npc_count = self.live_npc_count.saturating_sub(count);
        if self.live_npc_count < self.resume_npc_count {
            self.spawn_paused = false;
        }
    }

    pub fn should_reduce_population(&self) -> bool {
        self.live_npc_count >= self.max_npc_count
    }
}

#[derive(Clone, Copy, Debug, Component)]
pub struct PendingRetirement;

#[derive(Clone, Debug, Event)]
pub struct NpcRetireRequest {
    pub entity: Entity,
}

/// 邻居生子（plan §3.3）：Commoner 老死后由 spawn 侧消费，在死者附近
/// 生一个年龄 0–5% max_age 的新生儿。受 `NpcRegistry` 预留预算约束。
///
/// Beast 领地繁衍（§8）复用同一通道：`archetype = Beast` + 必填
/// `territory_center` / `territory_radius`（新生幼崽要挂 Territory 组件，
/// spawn 侧据此重建）。避免 lifecycle.rs 反向依赖 territory.rs。
#[derive(Clone, Debug, Event)]
pub struct NpcReproductionRequest {
    pub archetype: NpcArchetype,
    pub position: DVec3,
    pub home_zone: String,
    pub initial_age_ticks: f64,
    /// Beast 必填；Commoner 忽略。
    pub territory_center: Option<DVec3>,
    /// Beast 必填；Commoner 忽略。
    pub territory_radius: Option<f64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NpcDeathReason {
    NaturalAging,
}

#[derive(Clone, Debug, Event)]
#[allow(dead_code)]
pub struct NpcDeathNotice {
    pub npc_id: String,
    pub archetype: NpcArchetype,
    pub reason: NpcDeathReason,
    pub age_ticks: f64,
    pub max_age_ticks: f64,
}

pub fn register(app: &mut App) {
    app.insert_resource(NpcAgingConfig::default())
        .insert_resource(NpcRegistry::default())
        .add_event::<CultivationDeathTrigger>()
        .add_event::<PlayerTerminated>()
        .add_event::<NpcRetireRequest>()
        .add_event::<NpcReproductionRequest>()
        .add_event::<NpcDeathNotice>()
        .add_systems(
            PreUpdate,
            (update_npc_registry, age_npcs).before(big_brain::prelude::BigBrainSet::Scorers),
        )
        .add_systems(Update, (process_npc_retire_requests, handle_npc_terminated));
}

#[derive(Bundle)]
pub struct NpcRuntimeBundle {
    pub archetype: NpcArchetype,
    pub lifespan: NpcLifespan,
    pub cultivation: Cultivation,
    pub meridian_system: MeridianSystem,
    pub contamination: Contamination,
    pub wounds: Wounds,
    pub stamina: Stamina,
    pub combat_state: CombatState,
    pub status_effects: StatusEffects,
    pub derived_attrs: DerivedAttrs,
    pub lifecycle: Lifecycle,
}

pub fn npc_runtime_bundle(entity: Entity, archetype: NpcArchetype) -> NpcRuntimeBundle {
    NpcRuntimeBundle {
        archetype,
        lifespan: NpcLifespan::for_archetype(archetype),
        cultivation: Cultivation::default(),
        meridian_system: MeridianSystem::default(),
        contamination: Contamination::default(),
        wounds: Wounds::default(),
        stamina: Stamina::default(),
        combat_state: CombatState::default(),
        status_effects: StatusEffects::default(),
        derived_attrs: DerivedAttrs::default(),
        lifecycle: Lifecycle {
            character_id: canonical_npc_id(entity),
            fortune_remaining: 0,
            ..Default::default()
        },
    }
}

fn update_npc_registry(mut registry: ResMut<NpcRegistry>, npcs: RegistryNpcQuery<'_, '_>) {
    let mut counts_by_archetype = HashMap::new();
    let mut live_npc_count = 0usize;

    for (archetype, lifecycle) in &npcs {
        if lifecycle.is_some_and(|lifecycle| lifecycle.state == LifecycleState::Terminated) {
            continue;
        }

        live_npc_count += 1;
        *counts_by_archetype.entry(*archetype).or_default() += 1;
    }

    registry.refresh_from_counts(live_npc_count, counts_by_archetype);
}

fn age_npcs(config: Res<NpcAgingConfig>, mut npcs: AgingNpcQuery<'_, '_>) {
    if !config.enabled {
        return;
    }

    for mut lifespan in &mut npcs {
        lifespan.age_ticks += config.rate_multiplier.max(0.0);
    }
}

#[allow(clippy::type_complexity)]
fn process_npc_retire_requests(
    mut retire_requests: EventReader<NpcRetireRequest>,
    npcs: Query<
        (
            &NpcArchetype,
            &NpcLifespan,
            Option<&Position>,
            Option<&crate::npc::patrol::NpcPatrol>,
        ),
        With<NpcMarker>,
    >,
    mut cultivation_deaths: EventWriter<CultivationDeathTrigger>,
    mut reproduction_requests: EventWriter<NpcReproductionRequest>,
) {
    for request in retire_requests.read() {
        let Ok((archetype, lifespan, position, patrol)) = npcs.get(request.entity) else {
            continue;
        };

        cultivation_deaths.send(CultivationDeathTrigger {
            entity: request.entity,
            cause: CultivationDeathCause::NaturalAging,
            context: json!({
                "npc_id": canonical_npc_id(request.entity),
                "archetype": archetype.as_str(),
                "age_ticks": lifespan.age_ticks,
                "max_age_ticks": lifespan.max_age_ticks,
                "age_ratio": lifespan.age_ratio(),
                "reason": "retire_action",
            }),
        });

        // plan §3.3 — 凡人老死即邻居生子。由 spawn 侧消费事件并通过
        // `NpcRegistry::reserve_spawn_batch` 统一占配额，避免击穿上限。
        if *archetype == NpcArchetype::Commoner {
            if let (Some(pos), Some(patrol)) = (position, patrol) {
                reproduction_requests.send(NpcReproductionRequest {
                    archetype: NpcArchetype::Commoner,
                    position: pos.get(),
                    home_zone: patrol.home_zone.clone(),
                    initial_age_ticks: 0.0,
                    territory_center: None,
                    territory_radius: None,
                });
            }
        }
    }
}

fn handle_npc_terminated(
    mut commands: Commands,
    mut terminated: EventReader<PlayerTerminated>,
    npcs: Query<(&NpcArchetype, &NpcLifespan, Option<&PendingRetirement>), With<NpcMarker>>,
    mut notices: EventWriter<NpcDeathNotice>,
) {
    for event in terminated.read() {
        let Ok((archetype, lifespan, pending_retirement)) = npcs.get(event.entity) else {
            continue;
        };

        if pending_retirement.is_some() {
            notices.send(NpcDeathNotice {
                npc_id: canonical_npc_id(event.entity),
                archetype: *archetype,
                reason: NpcDeathReason::NaturalAging,
                age_ticks: lifespan.age_ticks,
                max_age_ticks: lifespan.max_age_ticks,
            });
        }

        if let Some(mut entity_commands) = commands.get_entity(event.entity) {
            entity_commands.insert(Despawned);
            entity_commands.remove::<PendingRetirement>();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use valence::prelude::{App, Update};

    #[test]
    fn registry_hysteresis_pauses_at_cap_and_resumes_below_low_watermark() {
        let mut registry = NpcRegistry::default();

        registry.refresh_from_counts(512, HashMap::new());
        assert!(registry.spawn_paused);

        registry.refresh_from_counts(500, HashMap::new());
        assert!(
            registry.spawn_paused,
            "should remain paused until low watermark"
        );

        registry.refresh_from_counts(459, HashMap::new());
        assert!(!registry.spawn_paused, "should resume below low watermark");
    }

    #[test]
    fn reserve_spawn_batch_clamps_to_remaining_capacity() {
        let mut registry = NpcRegistry::default();
        registry.refresh_from_counts(510, HashMap::new());

        let granted = registry.reserve_spawn_batch(8);
        assert_eq!(granted, 2);
        assert_eq!(registry.live_npc_count, 512);
        assert!(registry.spawn_paused);
    }

    #[test]
    fn process_retire_requests_emits_natural_aging_trigger() {
        let mut app = App::new();
        app.add_event::<NpcRetireRequest>();
        app.add_event::<CultivationDeathTrigger>();
        app.add_event::<NpcReproductionRequest>();
        app.add_systems(Update, process_npc_retire_requests);

        let entity = app
            .world_mut()
            .spawn((
                NpcMarker,
                NpcArchetype::Zombie,
                NpcLifespan::new(99.0, 100.0),
            ))
            .id();

        app.world_mut().send_event(NpcRetireRequest { entity });
        app.update();

        let events = app
            .world()
            .resource::<bevy_ecs::event::Events<CultivationDeathTrigger>>();
        assert_eq!(events.len(), 1);

        let births = app
            .world()
            .resource::<bevy_ecs::event::Events<NpcReproductionRequest>>();
        assert_eq!(
            births.len(),
            0,
            "zombie retirement must not trigger reproduction"
        );
    }

    #[test]
    fn process_retire_requests_triggers_commoner_reproduction() {
        let mut app = App::new();
        app.add_event::<NpcRetireRequest>();
        app.add_event::<CultivationDeathTrigger>();
        app.add_event::<NpcReproductionRequest>();
        app.add_systems(Update, process_npc_retire_requests);

        let entity = app
            .world_mut()
            .spawn((
                NpcMarker,
                NpcArchetype::Commoner,
                NpcLifespan::new(89_999.0, 90_000.0),
                Position::new([42.0, 66.0, 17.5]),
                crate::npc::patrol::NpcPatrol::new("forest", DVec3::new(42.0, 66.0, 17.5)),
            ))
            .id();

        app.world_mut().send_event(NpcRetireRequest { entity });
        app.update();

        let births = app
            .world()
            .resource::<bevy_ecs::event::Events<NpcReproductionRequest>>();
        let all: Vec<_> = births.iter_current_update_events().collect();
        assert_eq!(all.len(), 1);
        let req = all[0];
        assert_eq!(req.archetype, NpcArchetype::Commoner);
        assert_eq!(req.home_zone, "forest");
        assert_eq!(req.position, DVec3::new(42.0, 66.0, 17.5));
        assert_eq!(req.initial_age_ticks, 0.0);
    }

    #[test]
    fn handle_npc_terminated_emits_notice_and_marks_despawned() {
        let mut app = App::new();
        app.add_event::<PlayerTerminated>();
        app.add_event::<NpcDeathNotice>();
        app.add_systems(Update, handle_npc_terminated);

        let entity = app
            .world_mut()
            .spawn((
                NpcMarker,
                NpcArchetype::Zombie,
                NpcLifespan::new(120.0, 100.0),
                PendingRetirement,
            ))
            .id();

        app.world_mut().send_event(PlayerTerminated { entity });
        app.update();

        let events = app
            .world()
            .resource::<bevy_ecs::event::Events<NpcDeathNotice>>();
        assert_eq!(events.len(), 1);
        assert!(app.world().get::<Despawned>(entity).is_some());
    }

    #[test]
    fn npc_death_notice_fields_are_readable_for_bridge_consumers() {
        let notice = NpcDeathNotice {
            npc_id: "npc_1v1".to_string(),
            archetype: NpcArchetype::Zombie,
            reason: NpcDeathReason::NaturalAging,
            age_ticks: 120.0,
            max_age_ticks: 100.0,
        };

        assert_eq!(notice.npc_id, "npc_1v1");
        assert_eq!(notice.archetype, NpcArchetype::Zombie);
        assert_eq!(notice.reason, NpcDeathReason::NaturalAging);
        assert_eq!(notice.age_ticks, 120.0);
        assert_eq!(notice.max_age_ticks, 100.0);
    }

    /// 端到端：致命 AttackIntent → resolve → DeathEvent → death_arbiter
    /// → NearDeath → near_death_tick 过 deadline → PlayerTerminated
    /// → handle_npc_terminated → `Despawned`. 全栈 NPC 无 LifeRecord。
    #[test]
    fn npc_full_death_chain_from_attack_to_despawned() {
        use crate::combat::components::NEAR_DEATH_WINDOW_TICKS;
        use crate::combat::events::{
            ApplyStatusEffectIntent, AttackIntent, CombatEvent, DeathEvent, FIST_REACH,
        };
        use crate::combat::lifecycle::{death_arbiter_tick, near_death_tick};
        use crate::combat::resolve::resolve_attack_intents;
        use crate::combat::CombatClock;
        use crate::cultivation::death_hooks::{
            CultivationDeathTrigger, PlayerRevived, PlayerTerminated,
        };
        use valence::prelude::{App, IntoSystemConfigs, Position, Update};

        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 100 });
        app.insert_resource(crate::persistence::PersistenceSettings::default());
        app.add_event::<AttackIntent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<CombatEvent>();
        app.add_event::<DeathEvent>();
        app.add_event::<crate::combat::weapon::WeaponBroken>();
        app.add_event::<crate::network::vfx_event_emit::VfxEventRequest>();
        app.add_event::<PlayerTerminated>();
        app.add_event::<PlayerRevived>();
        app.add_event::<CultivationDeathTrigger>();
        app.add_event::<NpcDeathNotice>();
        app.add_systems(
            Update,
            (
                resolve_attack_intents,
                death_arbiter_tick.after(resolve_attack_intents),
                near_death_tick.after(death_arbiter_tick),
                handle_npc_terminated.after(near_death_tick),
            ),
        );

        // 两个 NPC：attacker（满 qi）+ victim（濒死）
        let attacker = app
            .world_mut()
            .spawn((NpcMarker, Position::new([0.0, 64.0, 0.0])))
            .id();
        let mut attacker_bundle = npc_runtime_bundle(attacker, NpcArchetype::Zombie);
        attacker_bundle.cultivation.qi_current = 80.0;
        attacker_bundle.cultivation.qi_max = 100.0;
        app.world_mut().entity_mut(attacker).insert(attacker_bundle);

        let victim = app
            .world_mut()
            .spawn((NpcMarker, Position::new([1.0, 64.0, 0.0])))
            .id();
        let mut victim_bundle = npc_runtime_bundle(victim, NpcArchetype::Commoner);
        victim_bundle.wounds.health_current = 3.0;
        victim_bundle.wounds.health_max = 100.0;
        victim_bundle.cultivation.qi_current = 80.0;
        victim_bundle.cultivation.qi_max = 100.0;
        app.world_mut().entity_mut(victim).insert(victim_bundle);

        // victim 应无 LifeRecord（生产形态）。
        assert!(
            app.world()
                .get::<crate::cultivation::life_record::LifeRecord>(victim)
                .is_none(),
            "victim must not carry LifeRecord to prove NPC production bundle"
        );

        // 一击致命。
        app.world_mut().send_event(AttackIntent {
            attacker,
            target: Some(victim),
            issued_at_tick: 99,
            reach: FIST_REACH,
            qi_invest: 30.0,
            wound_kind: crate::combat::components::WoundKind::Blunt,
            debug_command: None,
        });

        // Tick 1: resolve 写 Wounds + DeathEvent；death_arbiter 看到 DeathEvent
        // 转 NearDeath + 设 deadline = clock.tick + 600。
        app.update();

        let victim_lifecycle = app
            .world()
            .entity(victim)
            .get::<crate::combat::components::Lifecycle>()
            .expect("victim keeps Lifecycle");
        assert_eq!(
            victim_lifecycle.state,
            crate::combat::components::LifecycleState::NearDeath,
            "after first tick victim should be NearDeath"
        );
        let deadline = victim_lifecycle
            .near_death_deadline_tick
            .expect("deadline should be set on NearDeath entry");
        assert_eq!(deadline, 100 + NEAR_DEATH_WINDOW_TICKS);
        assert!(app
            .world()
            .get::<valence::prelude::Despawned>(victim)
            .is_none());

        // 推进 CombatClock 过 deadline：NPC fortune_remaining=0 → 直接 Terminated。
        app.world_mut().resource_mut::<CombatClock>().tick = deadline + 1;

        // Tick 2: near_death_tick 发 PlayerTerminated；handle_npc_terminated
        //         插 Despawned + 发 NpcDeathNotice（只在 PendingRetirement 存在时，
        //         这里没有，所以 notice 不 fire — 但 Despawned 必须有）。
        app.update();

        assert!(
            app.world()
                .get::<valence::prelude::Despawned>(victim)
                .is_some(),
            "victim should be marked Despawned after termination chain"
        );

        // attacker 存活。
        assert!(app
            .world()
            .get::<valence::prelude::Despawned>(attacker)
            .is_none());
        let attacker_life = app
            .world()
            .entity(attacker)
            .get::<crate::combat::components::Lifecycle>()
            .unwrap();
        assert_eq!(
            attacker_life.state,
            crate::combat::components::LifecycleState::Alive
        );
    }
}
