pub mod gameplay;
pub mod home_return;
pub mod spawn_selector;
pub mod state;

use self::state::{
    canonical_player_id, load_player_slices, save_player_core_slice, save_player_inventory_slice,
    save_player_known_techniques_slice, save_player_lifecycle_slice,
    save_player_lifespan_slice_with_coffin, save_player_skill_slice,
    save_player_slices_with_coffin, save_player_slow_slice, LoadedKnownTechniques, PlayerState,
    PlayerStateAutosaveTimer, PlayerStatePersistence,
};
use crate::coffin::{coffin_lower_from_player_position, CoffinComponent, CoffinRegistry};
use crate::combat::components::{Lifecycle, UnlockedStyles, TICKS_PER_SECOND};
use crate::combat::woliu_v2::erosion::VoidErosion;
use crate::combat::CombatClock;
use crate::craft::CraftSession;
use crate::cultivation::color::PracticeLog;
use crate::cultivation::components::{Contamination, Cultivation, Karma, MeridianSystem, QiColor};
use crate::cultivation::insight::InsightQuota;
use crate::cultivation::insight_apply::{InsightModifiers, UnlockedPerceptions};
use crate::cultivation::known_techniques::{KnownTechniques, KnownTechniquesLoadFailed};
use crate::cultivation::life_record::LifeRecord;
use crate::cultivation::lifespan::LifespanComponent;
use crate::cultivation::meridian::severed::MeridianSeveredPermanent;
use crate::cultivation::poison_trait::{DigestionLoad, PoisonToxicity};
use crate::inventory::{attach_inventory_to_joined_clients, PlayerInventory};
use crate::persistence::persist_player_cultivation_bundle;
use crate::persistence::slice::ReconnectHandoffQueue;
use crate::persistence::PersistenceSettings;
use crate::skill::components::SkillSet;
use crate::skill::config::{SkillConfigSchemas, SkillConfigStore};
use crate::world::dimension::{CurrentDimension, DimensionKind, DimensionLayers};
use crate::world::spawn_tutorial::TutorialState;
use valence::entity::entity::Flags;
use valence::message::SendMessage;
use valence::prelude::bevy_ecs::query::Has;
use valence::prelude::Despawned;
use valence::prelude::{
    bevy_ecs, Added, App, AppExit, Changed, Client, Commands, Component, Entity, EntityLayerId,
    EventReader, GameMode, IntoSystemConfigs, Last, Or, Position, Query, RemovedComponents, Res,
    ResMut, Update, Username, VisibleChunkLayer, VisibleEntityLayers, With, Without,
};

const WELCOME_MESSAGE: &str =
    "Welcome to Bong! Test commands: /zones, /tpzone <zone>, /top, /gm <c|a|s>, /spawn";
const CORE_SLICE_FLUSH_INTERVAL_TICKS: u64 = 5 * TICKS_PER_SECOND;
const SLOW_UI_SLICE_FLUSH_INTERVAL_TICKS: u64 = 60 * TICKS_PER_SECOND;
const LIFESPAN_SLICE_FLUSH_INTERVAL_TICKS: u64 = 60 * TICKS_PER_SECOND;
const CULTIVATION_FLUSH_INTERVAL_TICKS: u64 = 60 * TICKS_PER_SECOND;
// bughunt player-lifecycle-relog-death-consequence-wipe（OPUS 返工要求 5）：镜像兄弟 slice
// 的 60s autosave 节奏——硬崩（非 AppExit，没有 flush_connected_players_on_shutdown 兜底）
// 后该行最多陈旧 60s，而不是任意陈旧到"上一次断线/关服"为止。
const LIFECYCLE_SLICE_FLUSH_INTERVAL_TICKS: u64 = 60 * TICKS_PER_SECOND;

type ClientInitQueryItem<'a> = (
    Entity,
    &'a mut Client,
    &'a mut EntityLayerId,
    &'a mut VisibleChunkLayer,
    &'a mut VisibleEntityLayers,
    &'a mut Position,
    &'a mut GameMode,
);

type JoinedClientsWithoutStateQueryItem<'a> = (
    Entity,
    &'a Username,
    &'a mut EntityLayerId,
    &'a mut VisibleChunkLayer,
    &'a mut VisibleEntityLayers,
    &'a mut Position,
    Option<&'a mut Flags>,
);
type JoinedClientsWithoutStateQueryFilter = (Added<Client>, Without<PlayerState>);
#[derive(Component, Default)]
struct InventoryPersistenceDirty;

type ChangedInventoryClientsQueryItem<'a> = (Entity, &'a Username, &'a PlayerInventory);
type ChangedInventoryClientsQueryFilter = (
    With<Client>,
    Without<crate::network::craft_emit::CraftSessionPersistenceDirty>,
    Or<(Changed<PlayerInventory>, With<InventoryPersistenceDirty>)>,
);
type ChangedSkillClientsQueryItem<'a> = (&'a Username, &'a SkillSet);
type ChangedSkillClientsQueryFilter = (With<Client>, Changed<SkillSet>);
type ChangedKnownTechniquesClientsQueryItem<'a> = (&'a Username, &'a KnownTechniques);
// Without<KnownTechniquesLoadFailed>：加载失败会话禁止把 default 表写回覆盖真实存档
type ChangedKnownTechniquesClientsQueryFilter = (
    With<Client>,
    Changed<KnownTechniques>,
    Without<KnownTechniquesLoadFailed>,
);
type CultivationBundleQueryItem<'a> = (
    &'a Username,
    &'a Cultivation,
    &'a MeridianSystem,
    &'a QiColor,
    &'a Karma,
    &'a PracticeLog,
    &'a Contamination,
    &'a LifeRecord,
    &'a InsightQuota,
    &'a UnlockedPerceptions,
    &'a InsightModifiers,
    Option<&'a TutorialState>,
    Option<&'a MeridianSeveredPermanent>,
    Option<&'a PoisonToxicity>,
    Option<&'a DigestionLoad>,
);

pub fn register(app: &mut App) {
    tracing::info!("[bong][player] registering player init/cleanup systems");
    app.insert_resource(PlayerStatePersistence::default());
    app.insert_resource(PlayerStateAutosaveTimer::default());
    gameplay::register(app);
    home_return::register(app);
    app.add_systems(
        Update,
        (
            init_clients,
            attach_player_state_to_joined_clients.after(init_clients),
            attach_inventory_to_joined_clients.after(attach_player_state_to_joined_clients),
            tick_player_persistence_timer,
            autosave_player_core_slices.after(tick_player_persistence_timer),
            autosave_player_slow_and_ui_slices.after(autosave_player_core_slices),
            autosave_player_cultivation_bundles.after(autosave_player_slow_and_ui_slices),
            autosave_player_lifespan_slices.after(autosave_player_cultivation_bundles),
            autosave_player_lifecycle_slices.after(autosave_player_lifespan_slices),
            flush_changed_player_skills.after(autosave_player_lifecycle_slices),
            flush_changed_player_known_techniques.after(flush_changed_player_skills),
            flush_changed_player_inventories
                .after(attach_inventory_to_joined_clients)
                .after(flush_changed_player_known_techniques)
                .after(crate::network::craft_emit::persist_dirty_craft_sessions),
            despawn_disconnected_clients.after(flush_changed_player_inventories),
        ),
    );
    app.add_systems(Last, flush_connected_players_on_shutdown);
}

pub fn spawn_position() -> [f64; 3] {
    spawn_selector::emergency_spawn_position()
}

pub fn spawn_position_for_seed(seed: &str, purpose: spawn_selector::SpawnPurpose) -> [f64; 3] {
    spawn_selector::fallback_spawn(seed, purpose)
}

pub fn welcome_message() -> &'static str {
    WELCOME_MESSAGE
}

pub fn initial_game_mode() -> GameMode {
    GameMode::Survival
}

fn init_clients(
    mut commands: Commands,
    mut clients: Query<ClientInitQueryItem<'_>, Added<Client>>,
    dimension_layers: Option<Res<DimensionLayers>>,
) {
    // Spawn defaults route every client into the overworld layer. The follow-up
    // `attach_player_state_to_joined_clients` system reads persisted state and
    // reroutes the client to its `last_dimension` (and inserts a matching
    // `CurrentDimension`) before any client packets are flushed this tick.
    // `DimensionLayers` is missing only in tests that do not bootstrap the world
    // plugin — fall through silently in that case.
    let Some(dimension_layers) = dimension_layers else {
        return;
    };
    let layer = dimension_layers.overworld;

    for (
        entity,
        mut client,
        mut layer_id,
        mut visible_chunk_layer,
        mut visible_entity_layers,
        mut position,
        mut game_mode,
    ) in &mut clients
    {
        apply_spawn_defaults(
            layer,
            &mut layer_id,
            &mut visible_chunk_layer,
            &mut visible_entity_layers,
            &mut position,
            &mut game_mode,
        );
        commands.entity(entity).insert(CurrentDimension::default());

        client.send_chat_message(welcome_message());

        let spawn_position = position_to_array(&position);
        let game_mode = *game_mode;
        tracing::info!(
            "[bong][player] initialized client entity {entity:?} at [{}, {}, {}] in {game_mode:?}",
            spawn_position[0],
            spawn_position[1],
            spawn_position[2]
        );
    }
}

pub(crate) fn attach_player_state_to_joined_clients(
    mut commands: Commands,
    persistence: Res<PlayerStatePersistence>,
    mut coffin_registry: Option<ResMut<CoffinRegistry>>,
    dimension_layers: Option<Res<DimensionLayers>>,
    mut skill_config_store: Option<ResMut<SkillConfigStore>>,
    skill_config_schemas: Option<Res<SkillConfigSchemas>>,
    mut joined_clients: Query<
        JoinedClientsWithoutStateQueryItem<'_>,
        JoinedClientsWithoutStateQueryFilter,
    >,
) {
    for (
        entity,
        username,
        mut layer_id,
        mut visible_chunk_layer,
        mut visible_entity_layers,
        mut position,
        flags,
    ) in &mut joined_clients
    {
        let persisted = load_player_slices(&persistence, username.0.as_str());
        let restored_inventory = persisted.inventory.is_some();
        let restored_lifespan = persisted.lifespan.is_some();
        let restored_skill = !persisted.skill_set.skills.is_empty()
            || !persisted.skill_set.consumed_scrolls.is_empty();
        let (known_techniques, techniques_load_failed) = match persisted.known_techniques {
            LoadedKnownTechniques::Loaded(known_techniques) => (known_techniques, false),
            LoadedKnownTechniques::LoadFailed => (KnownTechniques::default(), true),
        };
        let restored_technique = !known_techniques.entries.is_empty();
        let last_dimension = persisted.last_dimension;
        let composite_power = persisted.state.composite_power(&Cultivation::default());
        position.set(persisted.position);

        if let Some(layers) = dimension_layers.as_deref() {
            let target_layer = layers.entity_for(last_dimension);
            let previous_layer = layer_id.0;
            if previous_layer != target_layer {
                visible_entity_layers.0.remove(&previous_layer);
                layer_id.0 = target_layer;
                visible_chunk_layer.0 = target_layer;
                visible_entity_layers.0.insert(target_layer);
            }
        }

        let quick_slot_bindings = persisted
            .ui_prefs
            .quick_slot_bindings(persisted.inventory.as_ref());
        let skill_bar_bindings = persisted
            .ui_prefs
            .skill_bar_bindings(persisted.inventory.as_ref());
        if let (Some(store), Some(schemas)) = (
            skill_config_store.as_deref_mut(),
            skill_config_schemas.as_deref(),
        ) {
            store.replace_player_configs(
                canonical_player_id(username.0.as_str()).as_str(),
                persisted.ui_prefs.skill_configs.clone(),
                schemas,
            );
        }
        let mut entity_commands = commands.entity(entity);
        entity_commands.insert((
            persisted.state,
            CurrentDimension(last_dimension),
            quick_slot_bindings,
            skill_bar_bindings,
            UnlockedStyles::default(),
            known_techniques,
        ));
        if techniques_load_failed {
            entity_commands.insert(KnownTechniquesLoadFailed);
        }
        if let Some(player_inventory) = persisted.inventory {
            entity_commands.insert(player_inventory);
        }
        if let Some(craft_session) = persisted.craft_session {
            entity_commands.insert(craft_session);
        }
        if let Some(lifespan) = persisted.lifespan {
            entity_commands.insert(lifespan);
        }
        if persisted.in_coffin {
            if let Some(mut flags) = flags {
                flags.set_invisible(true);
            }
            let coffin_lower = coffin_lower_from_player_position(persisted.position);
            // coffin_grade = Option<CoffinGrade>；in_coffin=true 路径下 unwrap_or_default 安全
            let grade = persisted.coffin_grade.unwrap_or_default();
            if let Some(registry) = coffin_registry.as_deref_mut() {
                registry.reclaim_occupied(coffin_lower, entity, 0, grade);
            }
            entity_commands.insert(CoffinComponent {
                entered_at_tick: 0,
                coffin_lower,
                grade,
            });
        }
        entity_commands.insert(persisted.skill_set);
        // plan-combat-skill-feedback-bridges-v1 P3 — 虚蚀组件初始化。
        //
        // 此处 `insert(VoidErosion::default())` 是正确且无副作用的：
        // - 系统仅在 JoinedClientsWithoutStateQueryFilter 过滤后的实体上运行，
        //   即每次 join 时才执行，不会对已有 state 的实体重复触发。
        // - VoidErosion 目前**不在持久化路径**（persisted 结构体不包含该字段），
        //   每次 join 从 default() 开始是有意为之的当前设计。
        // - 跨死亡保留 cumulative_erosion 的语义由 last_reported_stage 字段
        //   在 session 内（不跨重启）由 ECS 组件生命周期保证；
        //   若未来需要跨 server 重启持久化，需同时修改 PlayerStateAutosave 序列化路径。
        entity_commands.insert(VoidErosion::default());
        tracing::info!(
            "[bong][player] attached PlayerState to client entity {entity:?} for `{}` (composite_power={composite_power:.3}, restored_inventory={restored_inventory}, restored_lifespan={restored_lifespan}, restored_skill={restored_skill}, restored_technique={restored_technique}, last_dimension={last_dimension:?})",
            username.0,
        );
    }
}

fn apply_spawn_defaults(
    layer: Entity,
    layer_id: &mut EntityLayerId,
    visible_chunk_layer: &mut VisibleChunkLayer,
    visible_entity_layers: &mut VisibleEntityLayers,
    position: &mut Position,
    game_mode: &mut GameMode,
) {
    layer_id.0 = layer;
    visible_chunk_layer.0 = layer;
    visible_entity_layers.0.insert(layer);
    position.set(spawn_position());
    *game_mode = initial_game_mode();
}

fn position_to_array(position: &Position) -> [f64; 3] {
    let current = position.get();
    [current.x, current.y, current.z]
}

fn tick_player_persistence_timer(mut timer: ResMut<PlayerStateAutosaveTimer>) {
    timer.ticks += 1;
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(crate) fn despawn_disconnected_clients(
    mut commands: Commands,
    persistence: Res<PlayerStatePersistence>,
    mut coffin_registry: Option<ResMut<CoffinRegistry>>,
    mut disconnected_clients: RemovedComponents<Client>,
    settings: Res<PersistenceSettings>,
    // bughunt player-lifecycle-relog-death-consequence-wipe（OPUS 返工要求 1）：落盘
    // Lifecycle 时必须记录断连那一刻的 CombatClock.tick 作为跨重启折算 deadline 的锚点
    // （见 player::state::save_player_lifecycle_slice）。缺省（未注册 CombatClock 的最小化
    // 测试 app）时按 0 处理。
    combat_clock: Option<Res<CombatClock>>,
    mut reconnect_handoffs: Option<ResMut<ReconnectHandoffQueue>>,
    core_players: Query<(
        &Username,
        &PlayerState,
        &Position,
        Option<&CurrentDimension>,
        Option<&PlayerInventory>,
        Option<&LifespanComponent>,
        Option<&SkillSet>,
        Option<&KnownTechniques>,
        Has<KnownTechniquesLoadFailed>,
        Option<&CoffinComponent>,
        Option<&CraftSession>,
        Option<&Lifecycle>,
    )>,
    cultivation_bundle: Query<(
        &Cultivation,
        &MeridianSystem,
        &QiColor,
        &Karma,
        &PracticeLog,
        &Contamination,
        &LifeRecord,
        &InsightQuota,
        &UnlockedPerceptions,
        &InsightModifiers,
        Option<&TutorialState>,
        Option<&MeridianSeveredPermanent>,
        Option<&PoisonToxicity>,
        Option<&DigestionLoad>,
    )>,
) {
    let combat_clock_tick = combat_clock.as_deref().map_or(0, |clock| clock.tick);
    for entity in disconnected_clients.read() {
        // plan-race-system-v1 P4（决议 §6）—— 下线三条解除易形触发路径之一：断线即刻
        // 解除易形（移除 `MorphState` + 重扫装备门，见 `body_plan::morph::
        // release_morph_state`），防止玩家带着"易形态穿戴"的非法装备快照落盘。
        commands.add(
            move |world: &mut valence::prelude::bevy_ecs::world::World| {
                crate::body_plan::morph::release_morph_state(world, entity);
            },
        );
        if let Ok((
            username,
            player_state,
            position,
            current_dimension,
            player_inventory,
            lifespan,
            skill_set,
            known_techniques,
            known_techniques_load_failed,
            coffin,
            craft_session,
            lifecycle,
        )) = core_players.get(entity)
        {
            if let Some(queue) = reconnect_handoffs.as_deref_mut() {
                queue.enqueue_subject(canonical_player_id(username.0.as_str()).as_str());
            }
            let last_dimension = current_dimension
                .map(|cd| cd.0)
                .unwrap_or(DimensionKind::default());

            if let Ok((
                cultivation,
                meridians,
                qi_color,
                karma,
                practice_log,
                contamination,
                life_record,
                insight_quota,
                unlocked_perceptions,
                insight_modifiers,
                tutorial_state,
                severed,
                poison_toxicity,
                digestion_load,
            )) = cultivation_bundle.get(entity)
            {
                let severed_owned: MeridianSeveredPermanent = severed.cloned().unwrap_or_default();
                if let Err(error) = persist_player_cultivation_bundle(
                    &settings,
                    username.0.as_str(),
                    cultivation,
                    meridians,
                    qi_color,
                    karma,
                    contamination,
                    life_record,
                    practice_log,
                    insight_quota,
                    unlocked_perceptions,
                    insight_modifiers,
                    tutorial_state,
                    &severed_owned,
                    poison_toxicity,
                    digestion_load,
                ) {
                    tracing::warn!(
                        "[bong][player] failed to persist cultivation bundle for disconnected client `{}`: {error}",
                        username.0,
                    );
                }
            }
            match save_player_slices_with_coffin(
                &persistence,
                username.0.as_str(),
                player_state,
                position_to_array(position),
                last_dimension,
                player_inventory,
                lifespan,
                skill_set.unwrap_or(&SkillSet::default()),
                coffin.map(|c| c.grade),
                craft_session,
            ) {
                Ok(path) => tracing::info!(
                    "[bong][player] saved player slices for disconnected client `{}` to {} before cleanup",
                    username.0,
                    path.display()
                ),
                Err(error) => tracing::warn!(
                    "[bong][player] failed to save player slices for disconnected client `{}`: {error}",
                    username.0,
                ),
            }
            if known_techniques_load_failed {
                tracing::warn!(
                    "[bong][player] skipping known techniques save for disconnected client `{}`: join-time load failed, refusing to overwrite the stored row",
                    username.0,
                );
            } else if let Some(known_techniques) = known_techniques {
                if let Err(error) = save_player_known_techniques_slice(
                    &persistence,
                    username.0.as_str(),
                    known_techniques,
                ) {
                    tracing::warn!(
                        "[bong][player] failed to save known techniques for disconnected client `{}`: {error}",
                        username.0,
                    );
                }
            }
            // bughunt player-lifecycle-relog-death-consequence-wipe：断线必须落盘死亡/
            // 复活状态机，否则重连时 attach_combat_bundle_to_joined_clients 只能盲插
            // Lifecycle::default()，把 NearDeath/AwaitingRevival 玩家重置成满状态新角色。
            if let Some(lifecycle) = lifecycle {
                if let Err(error) = save_player_lifecycle_slice(
                    &persistence,
                    username.0.as_str(),
                    lifecycle,
                    combat_clock_tick,
                ) {
                    tracing::warn!(
                        "[bong][player] failed to save lifecycle state for disconnected client `{}`: {error}",
                        username.0,
                    );
                }
            }
        } else {
            tracing::warn!(
                "[bong][player] disconnected client entity {entity:?} had no username/PlayerState/Position to persist before cleanup"
            );
        }

        if let Some(registry) = coffin_registry.as_deref_mut() {
            registry.clear_player(entity);
        }
        tracing::info!("[bong][player] cleaning up disconnected client entity {entity:?}");
        if let Some(mut entity_commands) = commands.get_entity(entity) {
            entity_commands.insert(Despawned);
        }
    }
}

#[allow(clippy::type_complexity)]
fn flush_connected_players_on_shutdown(
    persistence: Res<PlayerStatePersistence>,
    mut app_exit: EventReader<AppExit>,
    settings: Res<PersistenceSettings>,
    // bughunt player-lifecycle-relog-death-consequence-wipe（OPUS 返工要求 1）：同
    // despawn_disconnected_clients，关服 flush 落盘 Lifecycle 时同样要记录 CombatClock.tick
    // 锚点。
    combat_clock: Option<Res<CombatClock>>,
    players: Query<
        (
            Entity,
            &Username,
            &PlayerState,
            &Position,
            Option<&CurrentDimension>,
            Option<&PlayerInventory>,
            Option<&LifespanComponent>,
            Option<&SkillSet>,
            Option<&KnownTechniques>,
            Has<KnownTechniquesLoadFailed>,
            Option<&CoffinComponent>,
            Option<&CraftSession>,
            Option<&Lifecycle>,
        ),
        With<Client>,
    >,
    cultivation_bundle: Query<(
        &Cultivation,
        &MeridianSystem,
        &QiColor,
        &Karma,
        &PracticeLog,
        &Contamination,
        &LifeRecord,
        &InsightQuota,
        &UnlockedPerceptions,
        &InsightModifiers,
        Option<&TutorialState>,
        Option<&MeridianSeveredPermanent>,
        Option<&PoisonToxicity>,
        Option<&DigestionLoad>,
    )>,
) {
    if app_exit.read().next().is_none() {
        return;
    }

    let combat_clock_tick = combat_clock.as_deref().map_or(0, |clock| clock.tick);
    for (
        entity,
        username,
        player_state,
        position,
        current_dimension,
        player_inventory,
        lifespan,
        skill_set,
        known_techniques,
        known_techniques_load_failed,
        coffin,
        craft_session,
        lifecycle,
    ) in &players
    {
        let last_dimension = current_dimension
            .map(|cd| cd.0)
            .unwrap_or(DimensionKind::default());

        if let Ok((
            cultivation,
            meridians,
            qi_color,
            karma,
            practice_log,
            contamination,
            life_record,
            insight_quota,
            unlocked_perceptions,
            insight_modifiers,
            tutorial_state,
            severed,
            poison_toxicity,
            digestion_load,
        )) = cultivation_bundle.get(entity)
        {
            let severed_owned: MeridianSeveredPermanent = severed.cloned().unwrap_or_default();
            if let Err(error) = persist_player_cultivation_bundle(
                &settings,
                username.0.as_str(),
                cultivation,
                meridians,
                qi_color,
                karma,
                contamination,
                life_record,
                practice_log,
                insight_quota,
                unlocked_perceptions,
                insight_modifiers,
                tutorial_state,
                &severed_owned,
                poison_toxicity,
                digestion_load,
            ) {
                tracing::warn!(
                    "[bong][player] failed to persist cultivation bundle during shutdown flush for `{}`: {error}",
                    username.0,
                );
            }
        }
        match save_player_slices_with_coffin(
            &persistence,
            username.0.as_str(),
            player_state,
            position_to_array(position),
            last_dimension,
            player_inventory,
            lifespan,
            skill_set.unwrap_or(&SkillSet::default()),
            coffin.map(|c| c.grade),
            craft_session,
        ) {
            Ok(path) => tracing::info!(
                "[bong][player] saved player slices for shutdown flush `{}` to {}",
                username.0,
                path.display()
            ),
            Err(error) => tracing::warn!(
                "[bong][player] failed to save player slices during shutdown flush for `{}`: {error}",
                username.0,
            ),
        }
        if known_techniques_load_failed {
            tracing::warn!(
                "[bong][player] skipping known techniques save during shutdown flush for `{}`: join-time load failed, refusing to overwrite the stored row",
                username.0,
            );
        } else if let Some(known_techniques) = known_techniques {
            if let Err(error) = save_player_known_techniques_slice(
                &persistence,
                username.0.as_str(),
                known_techniques,
            ) {
                tracing::warn!(
                    "[bong][player] failed to save known techniques during shutdown flush for `{}`: {error}",
                    username.0,
                );
            }
        }
        // bughunt player-lifecycle-relog-death-consequence-wipe：关服时同样要落盘死亡/
        // 复活状态机（同 despawn_disconnected_clients 的写路径），否则重启后重连会命中
        // 老档缺失行、回退到 Lifecycle::default() 抹掉关服前的濒死/待复活状态。
        if let Some(lifecycle) = lifecycle {
            if let Err(error) = save_player_lifecycle_slice(
                &persistence,
                username.0.as_str(),
                lifecycle,
                combat_clock_tick,
            ) {
                tracing::warn!(
                    "[bong][player] failed to save lifecycle state during shutdown flush for `{}`: {error}",
                    username.0,
                );
            }
        }
    }
}

fn autosave_player_core_slices(
    persistence: Res<PlayerStatePersistence>,
    timer: Res<PlayerStateAutosaveTimer>,
    players: Query<(&Username, &PlayerState), With<Client>>,
) {
    if !timer.ticks.is_multiple_of(CORE_SLICE_FLUSH_INTERVAL_TICKS) {
        return;
    }

    let mut saved_count = 0usize;

    for (username, player_state) in &players {
        match save_player_core_slice(&persistence, username.0.as_str(), player_state) {
            Ok(_) => saved_count += 1,
            Err(error) => tracing::warn!(
                "[bong][player] 5s core flush failed for `{}`: {error}",
                username.0,
            ),
        }
    }

    tracing::info!(
        "[bong][player] flushed {saved_count} core player slice(s) after {CORE_SLICE_FLUSH_INTERVAL_TICKS} ticks"
    );
}

fn autosave_player_slow_and_ui_slices(
    persistence: Res<PlayerStatePersistence>,
    timer: Res<PlayerStateAutosaveTimer>,
    players: Query<(&Username, &Position, Option<&CurrentDimension>), With<Client>>,
) {
    if !timer
        .ticks
        .is_multiple_of(SLOW_UI_SLICE_FLUSH_INTERVAL_TICKS)
    {
        return;
    }

    let mut saved_count = 0usize;

    for (username, position, current_dimension) in &players {
        let last_dimension = current_dimension
            .map(|cd| cd.0)
            .unwrap_or(DimensionKind::default());
        match save_player_slow_slice(
            &persistence,
            username.0.as_str(),
            position_to_array(position),
            last_dimension,
        ) {
            Ok(_) => saved_count += 1,
            Err(error) => tracing::warn!(
                "[bong][player] 60s slow/ui flush failed for `{}`: {error}",
                username.0,
            ),
        }
    }

    tracing::info!(
        "[bong][player] flushed {saved_count} slow/ui player slice(s) after {SLOW_UI_SLICE_FLUSH_INTERVAL_TICKS} ticks"
    );
}

fn autosave_player_cultivation_bundles(
    settings: Res<PersistenceSettings>,
    timer: Res<PlayerStateAutosaveTimer>,
    players: Query<CultivationBundleQueryItem<'_>, With<Client>>,
) {
    if !timer.ticks.is_multiple_of(CULTIVATION_FLUSH_INTERVAL_TICKS) {
        return;
    }

    let mut saved_count = 0usize;

    for (
        username,
        cultivation,
        meridians,
        qi_color,
        karma,
        practice_log,
        contamination,
        life_record,
        insight_quota,
        unlocked_perceptions,
        insight_modifiers,
        tutorial_state,
        severed,
        poison_toxicity,
        digestion_load,
    ) in &players
    {
        let severed_owned: MeridianSeveredPermanent = severed.cloned().unwrap_or_default();
        match persist_player_cultivation_bundle(
            &settings,
            username.0.as_str(),
            cultivation,
            meridians,
            qi_color,
            karma,
            contamination,
            life_record,
            practice_log,
            insight_quota,
            unlocked_perceptions,
            insight_modifiers,
            tutorial_state,
            &severed_owned,
            poison_toxicity,
            digestion_load,
        ) {
            Ok(()) => saved_count += 1,
            Err(error) => tracing::warn!(
                "[bong][player] 60s cultivation flush failed for `{}`: {error}",
                username.0,
            ),
        }
    }

    tracing::info!(
        "[bong][player] flushed {saved_count} cultivation bundle(s) after {CULTIVATION_FLUSH_INTERVAL_TICKS} ticks"
    );
}

fn autosave_player_lifespan_slices(
    persistence: Res<PlayerStatePersistence>,
    timer: Res<PlayerStateAutosaveTimer>,
    players: Query<(&Username, &LifespanComponent, Option<&CoffinComponent>), With<Client>>,
) {
    if !timer
        .ticks
        .is_multiple_of(LIFESPAN_SLICE_FLUSH_INTERVAL_TICKS)
    {
        return;
    }

    let mut saved_count = 0usize;

    for (username, lifespan, coffin) in &players {
        match save_player_lifespan_slice_with_coffin(
            &persistence,
            username.0.as_str(),
            lifespan,
            coffin.map(|c| c.grade),
        ) {
            Ok(_) => saved_count += 1,
            Err(error) => tracing::warn!(
                "[bong][player] 60s lifespan flush failed for `{}`: {error}",
                username.0,
            ),
        }
    }

    tracing::info!(
        "[bong][player] flushed {saved_count} lifespan slice(s) after {LIFESPAN_SLICE_FLUSH_INTERVAL_TICKS} ticks"
    );
}

/// bughunt player-lifecycle-relog-death-consequence-wipe（OPUS 返工要求 5）：Lifecycle 之前
/// 只在断线 (`despawn_disconnected_clients`) / 关服 (`flush_connected_players_on_shutdown`)
/// 两条路径落盘，硬崩（进程被杀、非 `AppExit` 的正常关服路径）时这两条路径都不会触发，
/// 该行会残留到"上一次真正的断线/关服"为止——可能是几小时前的死亡状态。镜像兄弟 slice
/// （lifespan/core/cultivation）既有的 autosave 节奏，每 60s 兜底落盘一次，硬崩后最多陈旧
/// 60s。
fn autosave_player_lifecycle_slices(
    persistence: Res<PlayerStatePersistence>,
    timer: Res<PlayerStateAutosaveTimer>,
    combat_clock: Option<Res<CombatClock>>,
    players: Query<(&Username, &Lifecycle), With<Client>>,
) {
    if !timer
        .ticks
        .is_multiple_of(LIFECYCLE_SLICE_FLUSH_INTERVAL_TICKS)
    {
        return;
    }

    let combat_clock_tick = combat_clock.as_deref().map_or(0, |clock| clock.tick);
    let mut saved_count = 0usize;

    for (username, lifecycle) in &players {
        match save_player_lifecycle_slice(
            &persistence,
            username.0.as_str(),
            lifecycle,
            combat_clock_tick,
        ) {
            Ok(_) => saved_count += 1,
            Err(error) => tracing::warn!(
                "[bong][player] 60s lifecycle flush failed for `{}`: {error}",
                username.0,
            ),
        }
    }

    tracing::info!(
        "[bong][player] flushed {saved_count} lifecycle slice(s) after {LIFECYCLE_SLICE_FLUSH_INTERVAL_TICKS} ticks"
    );
}

fn flush_changed_player_inventories(
    mut commands: Commands,
    persistence: Res<PlayerStatePersistence>,
    players: Query<ChangedInventoryClientsQueryItem<'_>, ChangedInventoryClientsQueryFilter>,
) {
    for (entity, username, player_inventory) in &players {
        match save_player_inventory_slice(&persistence, username.0.as_str(), Some(player_inventory))
        {
            Ok(_) => {
                commands
                    .entity(entity)
                    .remove::<InventoryPersistenceDirty>();
            }
            Err(error) => {
                commands.entity(entity).insert(InventoryPersistenceDirty);
                tracing::warn!(
                    "[bong][player] immediate inventory flush failed for `{}`: {error}",
                    username.0,
                );
            }
        }
    }
}

fn flush_changed_player_skills(
    persistence: Res<PlayerStatePersistence>,
    players: Query<ChangedSkillClientsQueryItem<'_>, ChangedSkillClientsQueryFilter>,
) {
    for (username, skill_set) in &players {
        if let Err(error) = save_player_skill_slice(&persistence, username.0.as_str(), skill_set) {
            tracing::warn!(
                "[bong][player] immediate skill flush failed for `{}`: {error}",
                username.0,
            );
        }
    }
}

fn flush_changed_player_known_techniques(
    persistence: Res<PlayerStatePersistence>,
    players: Query<
        ChangedKnownTechniquesClientsQueryItem<'_>,
        ChangedKnownTechniquesClientsQueryFilter,
    >,
) {
    for (username, known_techniques) in &players {
        if let Err(error) =
            save_player_known_techniques_slice(&persistence, username.0.as_str(), known_techniques)
        {
            tracing::warn!(
                "[bong][player] immediate known techniques flush failed for `{}`: {error}",
                username.0,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use rusqlite::{params, Connection};
    use valence::prelude::{App, DVec3, Position, Resource, Update};
    use valence::testing::create_mock_client;

    use crate::inventory::{
        ContainerState, InventoryRevision, ItemInstance, ItemRarity, PlacedItemState,
        PlayerInventory,
    };
    use crate::persistence::bootstrap_sqlite;

    #[test]
    fn spawn_defaults_are_preserved() {
        let mut app = App::new();
        let initial_layer = app.world_mut().spawn_empty().id();
        let spawn_layer = app.world_mut().spawn_empty().id();
        let mut layer_id = EntityLayerId(initial_layer);
        let mut visible_chunk_layer = VisibleChunkLayer(initial_layer);
        let mut visible_entity_layers = VisibleEntityLayers::default();
        let mut position = Position::new([0.0, 0.0, 0.0]);
        let mut game_mode = GameMode::Survival;

        visible_entity_layers.0.insert(initial_layer);

        apply_spawn_defaults(
            spawn_layer,
            &mut layer_id,
            &mut visible_chunk_layer,
            &mut visible_entity_layers,
            &mut position,
            &mut game_mode,
        );

        assert_eq!(spawn_position(), [8.0, 150.0, 8.0]);
        assert_eq!(position.get(), DVec3::new(8.0, 150.0, 8.0));
        assert_eq!(initial_game_mode(), GameMode::Survival);
        assert_eq!(game_mode, GameMode::Survival);
        assert_eq!(welcome_message(), WELCOME_MESSAGE);
        assert_eq!(layer_id.0, spawn_layer);
        assert_eq!(visible_chunk_layer.0, spawn_layer);
        assert!(visible_entity_layers.0.contains(&spawn_layer));
    }

    fn unique_temp_dir(test_name: &str) -> PathBuf {
        let unique_suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();

        std::env::temp_dir().join(format!(
            "bong-player-mod-{test_name}-{}-{unique_suffix}",
            std::process::id()
        ))
    }

    fn sqlite_persistence(test_name: &str) -> (PlayerStatePersistence, PathBuf, PathBuf) {
        let data_dir = unique_temp_dir(test_name);
        let db_path = data_dir.join("bong.db");
        bootstrap_sqlite(&db_path, &format!("player-mod-{test_name}"))
            .expect("sqlite bootstrap should succeed");
        (
            PlayerStatePersistence::with_db_path(&data_dir, &db_path),
            data_dir,
            db_path,
        )
    }

    fn make_inventory() -> PlayerInventory {
        PlayerInventory {
            triggered_treasures: Vec::new(),
            revision: InventoryRevision(7),
            containers: vec![ContainerState {
                quick_access: false,
                id: "main_pack".to_string(),
                name: "主背包".to_string(),
                rows: 5,
                cols: 7,
                items: vec![PlacedItemState {
                    row: 0,
                    col: 0,
                    instance: ItemInstance {
                        instance_id: 77,
                        template_id: "starter_talisman".to_string(),
                        display_name: "启程护符".to_string(),
                        grid_w: 1,
                        grid_h: 1,
                        weight: 0.1,
                        rarity: ItemRarity::Common,
                        description: "fixture".to_string(),
                        stack_count: 1,
                        spirit_quality: 1.0,
                        durability: 1.0,
                        freshness: None,
                        mineral_id: None,
                        charges: None,
                        forge_quality: None,
                        forge_color: None,
                        forge_side_effects: Vec::new(),
                        forge_achieved_tier: None,
                        alchemy: None,
                        lingering_owner_qi: None,
                    },
                }],

                owner_instance_id: None,
            }],
            equipped: HashMap::new(),
            hotbar: Default::default(),
            bone_coins: 9,
            max_weight: 45.0,
        }
    }

    fn read_core_snapshot(db_path: &PathBuf) -> (f64, f64) {
        let connection = Connection::open(db_path).expect("sqlite db should open");
        connection
            .query_row(
                "
                SELECT karma, inventory_score
                FROM player_core
                WHERE username = ?1
                ",
                params!["Azure"],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("player_core row should exist")
    }

    fn read_position_snapshot(db_path: &PathBuf) -> (f64, f64, f64) {
        let connection = Connection::open(db_path).expect("sqlite db should open");
        connection
            .query_row(
                "SELECT pos_x, pos_y, pos_z FROM player_slow WHERE username = ?1",
                params!["Azure"],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("player_slow row should exist")
    }

    fn read_inventory_json(db_path: &PathBuf) -> String {
        let connection = Connection::open(db_path).expect("sqlite db should open");
        connection
            .query_row(
                "SELECT inventory_json FROM inventories WHERE username = ?1",
                params!["Azure"],
                |row| row.get(0),
            )
            .expect("inventories row should exist")
    }

    fn read_known_techniques_json(db_path: &PathBuf) -> String {
        let connection = Connection::open(db_path).expect("sqlite db should open");
        connection
            .query_row(
                "SELECT known_techniques_json FROM player_known_techniques WHERE username = ?1",
                params!["Azure"],
                |row| row.get(0),
            )
            .expect("player_known_techniques row should exist")
    }

    fn dash_known_techniques(proficiency: f32) -> KnownTechniques {
        KnownTechniques {
            entries: vec![crate::cultivation::known_techniques::KnownTechnique {
                id: "movement.dash".to_string(),
                proficiency,
                active: true,
            }],
        }
    }

    fn dash_proficiency_from_json(json: &str) -> f64 {
        serde_json::from_str::<serde_json::Value>(json)
            .expect("known techniques JSON should decode")
            .pointer("/entries/0/proficiency")
            .and_then(serde_json::Value::as_f64)
            .expect("dash proficiency should exist")
    }

    #[derive(Default)]
    struct CapturedLoginPosition(Option<[f64; 3]>);

    impl Resource for CapturedLoginPosition {}

    fn capture_login_position_after_attach(
        mut captured: ResMut<CapturedLoginPosition>,
        players: Query<(&Username, &Position), With<Client>>,
    ) {
        for (username, position) in &players {
            if username.0 == "Azure" {
                captured.0 = Some(position.get().to_array());
            }
        }
    }

    fn read_ui_prefs_json(db_path: &PathBuf) -> String {
        let connection = Connection::open(db_path).expect("sqlite db should open");
        connection
            .query_row(
                "SELECT prefs_json FROM player_ui_prefs WHERE username = ?1",
                params!["Azure"],
                |row| row.get(0),
            )
            .expect("player_ui_prefs row should exist")
    }

    fn read_cultivation_json(db_path: &PathBuf) -> String {
        let connection = Connection::open(db_path).expect("sqlite db should open");
        connection
            .query_row(
                "SELECT cultivation_json FROM player_cultivation WHERE username = ?1",
                params!["Azure"],
                |row| row.get(0),
            )
            .expect("player_cultivation row should exist")
    }

    #[test]
    fn player_flushes_core_slow_inventory_and_ui_slices() {
        let (persistence, data_dir, db_path) = sqlite_persistence("flush-slices");
        crate::player::state::save_player_state(&persistence, "Azure", &PlayerState::default())
            .expect("baseline player state should persist");
        let mut app = App::new();
        app.insert_resource(persistence);
        app.insert_resource(PlayerStateAutosaveTimer {
            ticks: CORE_SLICE_FLUSH_INTERVAL_TICKS - 1,
        });
        app.add_systems(
            Update,
            (
                tick_player_persistence_timer,
                autosave_player_core_slices.after(tick_player_persistence_timer),
                autosave_player_slow_and_ui_slices.after(autosave_player_core_slices),
                flush_changed_player_inventories.after(autosave_player_slow_and_ui_slices),
            ),
        );

        let (mut client_bundle, _helper) = create_mock_client("Azure");
        client_bundle.player.position = Position::new([42.0, 77.0, -3.5]);
        let entity = app.world_mut().spawn(client_bundle).id();
        app.world_mut().entity_mut(entity).insert(PlayerState {
            karma: 0.2,
            inventory_score: 0.4,
        });
        app.world_mut().entity_mut(entity).insert(make_inventory());

        app.update();

        let (karma, inventory_score) = read_core_snapshot(&db_path);
        let (pos_x, pos_y, pos_z) = read_position_snapshot(&db_path);
        let inventory_json = read_inventory_json(&db_path);
        let prefs_json = read_ui_prefs_json(&db_path);

        assert_eq!(karma, 0.2);
        assert_eq!(inventory_score, 0.4);
        let [spawn_x, spawn_y, spawn_z] = spawn_position_for_seed(
            "Azure",
            crate::player::spawn_selector::SpawnPurpose::InitialLogin,
        );
        assert_eq!((pos_x, pos_y, pos_z), (spawn_x, spawn_y, spawn_z));
        assert_ne!(
            serde_json::from_str::<serde_json::Value>(&inventory_json)
                .expect("inventory_json should decode"),
            serde_json::Value::Null
        );
        let prefs_value = serde_json::from_str::<serde_json::Value>(&prefs_json)
            .expect("prefs_json should decode");
        assert!(prefs_value.get("quick_slots").is_some());
        assert!(prefs_value.get("skill_bar").is_some());

        app.world_mut()
            .resource_mut::<PlayerStateAutosaveTimer>()
            .ticks = SLOW_UI_SLICE_FLUSH_INTERVAL_TICKS - 1;
        app.update();

        let (karma_after_slow, inventory_score_after_slow) = read_core_snapshot(&db_path);
        let (pos_x_after_slow, pos_y_after_slow, pos_z_after_slow) =
            read_position_snapshot(&db_path);

        assert_eq!(karma_after_slow, 0.2);
        assert_eq!(inventory_score_after_slow, 0.4);
        assert_eq!(
            (pos_x_after_slow, pos_y_after_slow, pos_z_after_slow),
            (42.0, 77.0, -3.5)
        );

        let _ = fs::remove_dir_all(&data_dir);
    }

    #[test]
    fn cultivation_bundle_flushes_periodically() {
        let (persistence, data_dir, db_path) = sqlite_persistence("cultivation-flush");
        let mut app = App::new();
        app.insert_resource(PersistenceSettings::with_paths(
            &db_path,
            data_dir.join("deceased"),
            "player-cultivation-flush",
        ));
        app.insert_resource(PlayerStateAutosaveTimer {
            ticks: CULTIVATION_FLUSH_INTERVAL_TICKS - 1,
        });
        app.add_systems(
            Update,
            (
                tick_player_persistence_timer,
                autosave_player_cultivation_bundles.after(tick_player_persistence_timer),
            ),
        );

        let (client_bundle, _helper) = create_mock_client("Azure");
        let entity = app.world_mut().spawn(client_bundle).id();
        app.world_mut().entity_mut(entity).insert((
            Cultivation {
                realm: crate::cultivation::components::Realm::Condense,
                qi_current: 42.0,
                qi_max: 88.0,
                ..Default::default()
            },
            MeridianSystem::default(),
            QiColor::default(),
            Karma::default(),
            PracticeLog::default(),
            Contamination::default(),
            LifeRecord::new(crate::player::state::canonical_player_id("Azure")),
            InsightQuota::default(),
            UnlockedPerceptions::default(),
            InsightModifiers::new(),
        ));

        app.update();

        let cultivation_json = read_cultivation_json(&db_path);
        let bundle: serde_json::Value =
            serde_json::from_str(&cultivation_json).expect("cultivation bundle should deserialize");
        assert_eq!(bundle["cultivation"]["realm"].as_str(), Some("Condense"));
        assert_eq!(bundle["cultivation"]["qi_current"].as_f64(), Some(42.0));
        assert_eq!(bundle["cultivation"]["qi_max"].as_f64(), Some(88.0));

        let _ = persistence;
        let _ = fs::remove_dir_all(&data_dir);
    }

    // ── bughunt player-lifecycle-relog-death-consequence-wipe（OPUS 返工要求 5）──
    //
    // Lifecycle 之前只在断线/关服两条路径落盘；硬崩（非 AppExit）时两条路径都不触发，该行
    // 会残留到"上一次真正的断线/关服"为止。镜像兄弟 slice 的 60s autosave 节奏兜底。

    #[test]
    fn lifecycle_slice_flushes_periodically_at_interval_boundary() {
        use crate::combat::components::LifecycleState;

        let (persistence, data_dir, db_path) = sqlite_persistence("lifecycle-autosave-flush");
        let mut app = App::new();
        app.insert_resource(persistence);
        app.insert_resource(PersistenceSettings::with_paths(
            &db_path,
            data_dir.join("deceased"),
            "player-lifecycle-autosave-flush",
        ));
        app.insert_resource(PlayerStateAutosaveTimer {
            ticks: LIFECYCLE_SLICE_FLUSH_INTERVAL_TICKS - 1,
        });
        app.insert_resource(CombatClock { tick: 999 });
        app.add_systems(
            Update,
            (
                tick_player_persistence_timer,
                autosave_player_lifecycle_slices.after(tick_player_persistence_timer),
            ),
        );

        let (client_bundle, _helper) = create_mock_client("Azure");
        let entity = app.world_mut().spawn(client_bundle).id();
        app.world_mut().entity_mut(entity).insert(Lifecycle {
            state: LifecycleState::NearDeath,
            fortune_remaining: 2,
            near_death_deadline_tick: Some(1_020),
            ..Lifecycle::default()
        });

        // timer.ticks 恰好落在 INTERVAL_TICKS 边界上（LIFECYCLE_SLICE_FLUSH_INTERVAL_TICKS - 1
        // + tick_player_persistence_timer 的 +1 = LIFECYCLE_SLICE_FLUSH_INTERVAL_TICKS），
        // 60s autosave 必须在这一 tick 触发落盘。
        app.update();

        let lifecycle_json = read_lifecycle_json(&db_path);
        let persisted: Lifecycle =
            serde_json::from_str(&lifecycle_json).expect("persisted lifecycle_json should decode");
        assert_eq!(
            persisted.state,
            LifecycleState::NearDeath,
            "60s autosave 边界 tick 必须落盘当前 Lifecycle 状态"
        );
        assert_eq!(persisted.fortune_remaining, 2);
        assert_eq!(
            read_lifecycle_combat_clock_tick_at_save(&db_path),
            999,
            "autosave 落盘时也必须记录当时的 CombatClock.tick 锚点"
        );

        let _ = fs::remove_dir_all(&data_dir);
    }

    #[test]
    fn lifecycle_slice_does_not_flush_before_interval_boundary() {
        use crate::combat::components::LifecycleState;

        let (persistence, data_dir, db_path) =
            sqlite_persistence("lifecycle-autosave-no-early-flush");
        let mut app = App::new();
        app.insert_resource(persistence);
        app.insert_resource(PersistenceSettings::with_paths(
            &db_path,
            data_dir.join("deceased"),
            "player-lifecycle-autosave-no-early-flush",
        ));
        // ticks - 1 后面还差 2 才到 INTERVAL_TICKS，tick_player_persistence_timer 的 +1
        // 只能凑到 INTERVAL_TICKS - 1，不该触发落盘。
        app.insert_resource(PlayerStateAutosaveTimer {
            ticks: LIFECYCLE_SLICE_FLUSH_INTERVAL_TICKS.saturating_sub(2),
        });
        app.add_systems(
            Update,
            (
                tick_player_persistence_timer,
                autosave_player_lifecycle_slices.after(tick_player_persistence_timer),
            ),
        );

        let (client_bundle, _helper) = create_mock_client("Azure");
        let entity = app.world_mut().spawn(client_bundle).id();
        app.world_mut().entity_mut(entity).insert(Lifecycle {
            state: LifecycleState::NearDeath,
            ..Lifecycle::default()
        });

        app.update();

        let connection = Connection::open(&db_path).expect("sqlite db should open");
        let row_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM player_lifecycle WHERE username = ?1",
                params!["Azure"],
                |row| row.get(0),
            )
            .expect("row count query should succeed");
        assert_eq!(
            row_count, 0,
            "距 60s autosave 边界还差 1 tick，不应该提前落盘（否则边界判定逻辑被破坏，\
             要么漏判要么误判）"
        );

        let _ = fs::remove_dir_all(&data_dir);
    }

    #[test]
    fn changed_known_techniques_flush_persists_dash_proficiency() {
        let (persistence, data_dir, db_path) = sqlite_persistence("known-techniques-changed-flush");
        crate::player::state::save_player_state(&persistence, "Azure", &PlayerState::default())
            .expect("baseline player state should persist");

        let mut app = App::new();
        app.insert_resource(persistence);
        app.add_systems(Update, flush_changed_player_known_techniques);

        let (client_bundle, _helper) = create_mock_client("Azure");
        let entity = app.world_mut().spawn(client_bundle).id();
        app.world_mut()
            .entity_mut(entity)
            .insert(dash_known_techniques(0.58));

        app.update();

        let known_techniques_json = read_known_techniques_json(&db_path);
        assert!((dash_proficiency_from_json(&known_techniques_json) - 0.58).abs() < 1e-6);

        let _ = fs::remove_dir_all(&data_dir);
    }

    #[test]
    fn disconnect_flush_persists_latest_player_slices_before_cleanup() {
        let (persistence, data_dir, db_path) = sqlite_persistence("disconnect-flush");
        crate::player::state::save_player_state(&persistence, "Azure", &PlayerState::default())
            .expect("baseline player state should persist");

        let mut app = App::new();
        app.insert_resource(persistence);
        app.insert_resource(PersistenceSettings::with_paths(
            &db_path,
            data_dir.join("deceased"),
            "player-disconnect-flush",
        ));
        app.add_systems(Update, despawn_disconnected_clients);

        let (mut client_bundle, _helper) = create_mock_client("Azure");
        client_bundle.player.position = Position::new([42.0, 77.0, -3.5]);
        let entity = app.world_mut().spawn(client_bundle).id();
        app.world_mut().entity_mut(entity).insert(PlayerState {
            karma: -0.15,
            inventory_score: 0.7,
        });
        app.world_mut().entity_mut(entity).insert(make_inventory());
        app.world_mut()
            .entity_mut(entity)
            .insert(dash_known_techniques(0.37));

        app.world_mut().entity_mut(entity).remove::<Client>();
        app.update();

        let (karma, inventory_score) = read_core_snapshot(&db_path);
        let (pos_x, pos_y, pos_z) = read_position_snapshot(&db_path);
        let inventory_json = read_inventory_json(&db_path);
        let known_techniques_json = read_known_techniques_json(&db_path);

        assert_eq!(karma, -0.15);
        assert_eq!(inventory_score, 0.7);
        assert_eq!((pos_x, pos_y, pos_z), (42.0, 77.0, -3.5));
        assert!((dash_proficiency_from_json(&known_techniques_json) - 0.37).abs() < 1e-6);
        assert_ne!(
            serde_json::from_str::<serde_json::Value>(&inventory_json)
                .expect("inventory_json should decode"),
            serde_json::Value::Null
        );
        assert!(
            app.world().get::<Despawned>(entity).is_some(),
            "disconnect cleanup should mark entity as despawned"
        );

        let _ = fs::remove_dir_all(&data_dir);
    }

    #[test]
    fn disconnect_auto_releases_morph_state_before_persist_snapshot() {
        // plan-race-system-v1 P4 opus verifier MAJOR — 下线三条易形自动解除触发路径
        // 之一（见 despawn_disconnected_clients 内 release_morph_state deferred
        // command）此前零测试断言真被 remove。镜像
        // `disconnect_flush_persists_latest_player_slices_before_cleanup` 同款
        // RemovedComponents<Client> 触发模式（先 remove::<Client>() 再 app.update()）。
        let (persistence, data_dir, db_path) = sqlite_persistence("morph-auto-release-disconnect");
        crate::player::state::save_player_state(&persistence, "Azure", &PlayerState::default())
            .expect("baseline player state should persist");

        let mut app = App::new();
        app.insert_resource(persistence);
        app.insert_resource(PersistenceSettings::with_paths(
            &db_path,
            data_dir.join("deceased"),
            "player-morph-auto-release-disconnect",
        ));
        app.add_systems(Update, despawn_disconnected_clients);

        let (mut client_bundle, _helper) = create_mock_client("Azure");
        client_bundle.player.position = Position::new([1.0, 70.0, 1.0]);
        let entity = app.world_mut().spawn(client_bundle).id();
        app.world_mut().entity_mut(entity).insert(PlayerState {
            karma: 0.0,
            inventory_score: 0.0,
        });
        app.world_mut().entity_mut(entity).insert(make_inventory());
        app.world_mut()
            .entity_mut(entity)
            .insert(crate::body_plan::MorphState::new(
                crate::body_plan::RaceId::new("whale"),
                0,
                100,
            ));

        assert!(
            app.world()
                .entity(entity)
                .get::<crate::body_plan::MorphState>()
                .is_some(),
            "前置条件：下线前应处于易形态"
        );

        app.world_mut().entity_mut(entity).remove::<Client>();
        app.update();

        assert!(
            app.world()
                .entity(entity)
                .get::<crate::body_plan::MorphState>()
                .is_none(),
            "下线（RemovedComponents<Client>）应通过 release_morph_state 的 deferred \
             command 移除 MorphState，实测组件仍在场"
        );

        let _ = fs::remove_dir_all(&data_dir);
    }

    fn read_lifecycle_json(db_path: &PathBuf) -> String {
        let connection = Connection::open(db_path).expect("sqlite db should open");
        connection
            .query_row(
                "SELECT lifecycle_json FROM player_lifecycle WHERE username = ?1",
                params!["Azure"],
                |row| row.get(0),
            )
            .expect("player_lifecycle row should exist")
    }

    // bughunt player-lifecycle-relog-death-consequence-wipe（OPUS 返工要求 1）：读取
    // `combat_clock_tick_at_save` 锚点列——落盘时的 CombatClock.tick，用于跨重启折算 deadline
    // （见 player::state::translate_lifecycle_deadline_tick_across_restart）。
    fn read_lifecycle_combat_clock_tick_at_save(db_path: &PathBuf) -> u64 {
        let connection = Connection::open(db_path).expect("sqlite db should open");
        connection
            .query_row(
                "SELECT combat_clock_tick_at_save FROM player_lifecycle WHERE username = ?1",
                params!["Azure"],
                |row| row.get(0),
            )
            .expect("player_lifecycle row should exist")
    }

    #[test]
    fn disconnect_flush_persists_lifecycle_state_before_cleanup() {
        // bughunt player-lifecycle-relog-death-consequence-wipe：断线必须把死亡/复活
        // 状态机落盘（同 disconnect_auto_releases_morph_state_before_persist_snapshot 的
        // RemovedComponents<Client> 触发模式），否则重连时
        // attach_combat_bundle_to_joined_clients 只能盲插 Lifecycle::default()，把
        // AwaitingRevival + fortune_remaining=0 的濒死玩家重置成满状态新角色，完全绕过
        // 渡劫概率判定与永久终结风险。
        use crate::combat::components::{LifecycleState, RevivalDecision};

        let (persistence, data_dir, db_path) = sqlite_persistence("lifecycle-disconnect-flush");
        crate::player::state::save_player_state(&persistence, "Azure", &PlayerState::default())
            .expect("baseline player state should persist");

        let mut app = App::new();
        app.insert_resource(persistence);
        app.insert_resource(PersistenceSettings::with_paths(
            &db_path,
            data_dir.join("deceased"),
            "player-lifecycle-disconnect-flush",
        ));
        app.add_systems(Update, despawn_disconnected_clients);

        let (mut client_bundle, _helper) = create_mock_client("Azure");
        client_bundle.player.position = Position::new([1.0, 70.0, 1.0]);
        let entity = app.world_mut().spawn(client_bundle).id();
        app.world_mut().entity_mut(entity).insert(PlayerState {
            karma: 0.0,
            inventory_score: 0.0,
        });
        app.world_mut().entity_mut(entity).insert(make_inventory());
        app.world_mut().entity_mut(entity).insert(Lifecycle {
            character_id: "offline:Azure:char-1".to_string(),
            death_count: 4,
            fortune_remaining: 0,
            last_death_tick: Some(1_000),
            last_revive_tick: Some(500),
            spawn_anchor: Some([9.0, 64.0, -3.0]),
            spawn_anchor_damaged: true,
            near_death_deadline_tick: None,
            awaiting_decision: Some(RevivalDecision::Tribulation { chance: 0.2 }),
            revival_decision_deadline_tick: Some(1_600),
            weakened_until_tick: None,
            state: LifecycleState::AwaitingRevival,
        });

        app.world_mut().entity_mut(entity).remove::<Client>();
        app.update();

        let lifecycle_json = read_lifecycle_json(&db_path);
        let persisted: Lifecycle =
            serde_json::from_str(&lifecycle_json).expect("persisted lifecycle_json should decode");

        assert_eq!(persisted.character_id, "offline:Azure:char-1");
        assert_eq!(persisted.death_count, 4);
        assert_eq!(
            persisted.fortune_remaining, 0,
            "断线前 fortune_remaining=0（运气已耗尽）必须原样落盘，不能被写路径悄悄补回默认值 3"
        );
        assert_eq!(
            persisted.state,
            LifecycleState::AwaitingRevival,
            "断线前的 AwaitingRevival 决策窗口状态必须落盘，不能丢失/降级"
        );
        assert_eq!(
            persisted.awaiting_decision,
            Some(RevivalDecision::Tribulation { chance: 0.2 }),
            "待决策的渡劫结果（含永久终结风险）必须原样落盘"
        );
        assert_eq!(persisted.revival_decision_deadline_tick, Some(1_600));

        let _ = fs::remove_dir_all(&data_dir);
    }

    #[test]
    fn disconnect_flush_persists_combat_clock_tick_anchor_for_deadline_translation() {
        // bughunt player-lifecycle-relog-death-consequence-wipe（OPUS 返工要求 1）：断线
        // flush 必须把断连那一刻的 CombatClock.tick 写进 combat_clock_tick_at_save 列——
        // 这是跨重启折算 deadline 的锚点，缺了它 `load_player_lifecycle_slice` 就没法把
        // 落盘时的绝对 tick 换算到重启后的新 tick 空间。
        use crate::combat::components::{LifecycleState, RevivalDecision};

        let (persistence, data_dir, db_path) =
            sqlite_persistence("lifecycle-disconnect-clock-anchor");
        crate::player::state::save_player_state(&persistence, "Azure", &PlayerState::default())
            .expect("baseline player state should persist");

        let mut app = App::new();
        app.insert_resource(persistence);
        app.insert_resource(PersistenceSettings::with_paths(
            &db_path,
            data_dir.join("deceased"),
            "player-lifecycle-disconnect-clock-anchor",
        ));
        app.insert_resource(CombatClock { tick: 500_000 });
        app.add_systems(Update, despawn_disconnected_clients);

        let (mut client_bundle, _helper) = create_mock_client("Azure");
        client_bundle.player.position = Position::new([1.0, 70.0, 1.0]);
        let entity = app.world_mut().spawn(client_bundle).id();
        app.world_mut().entity_mut(entity).insert(PlayerState {
            karma: 0.0,
            inventory_score: 0.0,
        });
        app.world_mut().entity_mut(entity).insert(make_inventory());
        app.world_mut().entity_mut(entity).insert(Lifecycle {
            state: LifecycleState::AwaitingRevival,
            awaiting_decision: Some(RevivalDecision::Fortune { chance: 1.0 }),
            revival_decision_deadline_tick: Some(501_200),
            ..Lifecycle::default()
        });

        app.world_mut().entity_mut(entity).remove::<Client>();
        app.update();

        assert_eq!(
            read_lifecycle_combat_clock_tick_at_save(&db_path),
            500_000,
            "断线 flush 必须把当时的 CombatClock.tick(500_000) 写进 combat_clock_tick_at_save，\
             否则重连读档时无法折算 deadline 是否已跨重启过期"
        );

        let _ = fs::remove_dir_all(&data_dir);
    }

    #[test]
    fn shutdown_flush_persists_combat_clock_tick_anchor_for_deadline_translation() {
        // 同上，覆盖 flush_connected_players_on_shutdown 这条写路径（关服而非断线）。
        use crate::combat::components::LifecycleState;

        let (persistence, data_dir, db_path) =
            sqlite_persistence("lifecycle-shutdown-clock-anchor");
        crate::player::state::save_player_state(&persistence, "Azure", &PlayerState::default())
            .expect("baseline player state should persist");

        let mut app = App::default();
        app.insert_resource(persistence);
        app.insert_resource(PersistenceSettings::with_paths(
            &db_path,
            data_dir.join("deceased"),
            "player-lifecycle-shutdown-clock-anchor",
        ));
        app.insert_resource(CombatClock { tick: 777_000 });
        app.add_systems(Last, flush_connected_players_on_shutdown);

        let (mut client_bundle, _helper) = create_mock_client("Azure");
        client_bundle.player.position = Position::new([64.0, 80.0, -12.0]);
        let entity = app.world_mut().spawn(client_bundle).id();
        app.world_mut().entity_mut(entity).insert(PlayerState {
            karma: 0.0,
            inventory_score: 0.0,
        });
        app.world_mut().entity_mut(entity).insert(make_inventory());
        app.world_mut().entity_mut(entity).insert(Lifecycle {
            state: LifecycleState::NearDeath,
            near_death_deadline_tick: Some(777_600),
            ..Lifecycle::default()
        });

        app.world_mut().send_event(AppExit::Success);
        app.update();

        assert_eq!(
            read_lifecycle_combat_clock_tick_at_save(&db_path),
            777_000,
            "关服 flush 必须把当时的 CombatClock.tick(777_000) 写进 combat_clock_tick_at_save"
        );

        let _ = fs::remove_dir_all(&data_dir);
    }

    #[test]
    fn shutdown_flush_persists_connected_player_slices_without_disconnect() {
        let (persistence, data_dir, db_path) = sqlite_persistence("shutdown-flush");
        crate::player::state::save_player_state(&persistence, "Azure", &PlayerState::default())
            .expect("baseline player state should persist");

        let mut app = App::default();
        app.insert_resource(persistence);
        app.insert_resource(PersistenceSettings::with_paths(
            &db_path,
            data_dir.join("deceased"),
            "player-shutdown-flush",
        ));
        app.add_systems(Last, flush_connected_players_on_shutdown);

        let (mut client_bundle, _helper) = create_mock_client("Azure");
        client_bundle.player.position = Position::new([64.0, 80.0, -12.0]);
        let entity = app.world_mut().spawn(client_bundle).id();
        app.world_mut().entity_mut(entity).insert(PlayerState {
            karma: 0.33,
            inventory_score: 0.85,
        });
        app.world_mut().entity_mut(entity).insert(make_inventory());
        app.world_mut()
            .entity_mut(entity)
            .insert(dash_known_techniques(0.64));

        app.world_mut().send_event(AppExit::Success);
        app.update();

        let (karma, inventory_score) = read_core_snapshot(&db_path);
        let (pos_x, pos_y, pos_z) = read_position_snapshot(&db_path);
        let inventory_json = read_inventory_json(&db_path);
        let known_techniques_json = read_known_techniques_json(&db_path);

        assert_eq!(karma, 0.33);
        assert_eq!(inventory_score, 0.85);
        assert_eq!((pos_x, pos_y, pos_z), (64.0, 80.0, -12.0));
        assert!((dash_proficiency_from_json(&known_techniques_json) - 0.64).abs() < 1e-6);
        assert_ne!(
            serde_json::from_str::<serde_json::Value>(&inventory_json)
                .expect("inventory_json should decode"),
            serde_json::Value::Null
        );
        assert!(
            app.world().get::<Client>(entity).is_some(),
            "shutdown flush should persist while the player is still connected"
        );

        let _ = fs::remove_dir_all(&data_dir);
    }

    fn seed_and_corrupt_known_techniques_row(persistence: &PlayerStatePersistence) {
        crate::player::state::save_player_state(persistence, "Azure", &PlayerState::default())
            .expect("baseline player state should persist");
        crate::player::state::save_player_known_techniques_slice(
            persistence,
            "Azure",
            &dash_known_techniques(0.42),
        )
        .expect("seeding known techniques row should persist");
        let connection = Connection::open(persistence.db_path()).expect("sqlite db should open");
        connection
            .execute(
                "UPDATE player_known_techniques SET known_techniques_json = '{not json' WHERE username = ?1",
                params!["Azure"],
            )
            .expect("corrupting known techniques row should succeed");
    }

    #[test]
    fn join_with_corrupt_known_techniques_row_blocks_flush_from_wiping_it() {
        // C1 回归主锚：损坏行 → join 兜底 default → 同 tick Changed(=Added) flush。
        // 修复前该 flush 会把 default 空表写回 DB，玩家全部功法+熟练度永久蒸发。
        let (persistence, data_dir, db_path) = sqlite_persistence("known-techniques-corrupt-join");
        seed_and_corrupt_known_techniques_row(&persistence);

        let mut app = App::new();
        app.insert_resource(persistence);
        app.add_systems(
            Update,
            (
                attach_player_state_to_joined_clients,
                flush_changed_player_known_techniques.after(attach_player_state_to_joined_clients),
            ),
        );

        let (client_bundle, _helper) = create_mock_client("Azure");
        let entity = app.world_mut().spawn(client_bundle).id();
        app.update();
        // 多跑一帧：attach 的 insert 经 deferred command 落地后，Changed 过滤在
        // 下一帧才对 flush 系统可见，两帧覆盖「join 当帧 + 组件落地帧」全窗口。
        app.update();

        assert!(
            app.world()
                .get::<KnownTechniquesLoadFailed>(entity)
                .is_some(),
            "加载失败的 join 应给实体挂 KnownTechniquesLoadFailed 写保护标记"
        );
        assert_eq!(
            app.world()
                .get::<KnownTechniques>(entity)
                .expect("join should still attach a KnownTechniques component"),
            &KnownTechniques::default(),
            "加载失败时会话内组件应为 default（玩家本次会话看到空表，但存档不受损）"
        );
        assert_eq!(
            read_known_techniques_json(&db_path),
            "{not json",
            "DB 行必须保持损坏原文原样——任何写回（哪怕合法格式）都意味着真实存档被覆盖"
        );

        let _ = fs::remove_dir_all(&data_dir);
    }

    #[test]
    fn join_without_known_techniques_row_flushes_normally_as_new_player() {
        // 对照组：DB 无行（真新玩家）不挂写保护，后续熟练度增长照常落盘。
        let (persistence, data_dir, db_path) = sqlite_persistence("known-techniques-new-join");
        crate::player::state::save_player_state(&persistence, "Azure", &PlayerState::default())
            .expect("baseline player state should persist");

        let mut app = App::new();
        app.insert_resource(persistence);
        app.add_systems(
            Update,
            (
                attach_player_state_to_joined_clients,
                flush_changed_player_known_techniques.after(attach_player_state_to_joined_clients),
            ),
        );

        let (client_bundle, _helper) = create_mock_client("Azure");
        let entity = app.world_mut().spawn(client_bundle).id();
        app.update();
        app.update();

        assert!(
            app.world()
                .get::<KnownTechniquesLoadFailed>(entity)
                .is_none(),
            "真新玩家（无行）不得挂写保护标记，否则整个会话的功法进度都无法持久化"
        );

        app.world_mut()
            .entity_mut(entity)
            .insert(dash_known_techniques(0.58));
        app.update();

        assert!(
            (dash_proficiency_from_json(&read_known_techniques_json(&db_path)) - 0.58).abs() < 1e-6,
            "新玩家会话内的功法变更应照常经 Changed flush 落盘"
        );

        let _ = fs::remove_dir_all(&data_dir);
    }

    #[test]
    fn disconnect_save_skips_known_techniques_when_load_failed_marker_present() {
        let (persistence, data_dir, db_path) = sqlite_persistence("known-techniques-disc-guard");
        seed_and_corrupt_known_techniques_row(&persistence);

        let mut app = App::new();
        app.insert_resource(persistence);
        app.insert_resource(PersistenceSettings::with_paths(
            &db_path,
            data_dir.join("deceased"),
            "player-known-techniques-disc-guard",
        ));
        app.add_systems(Update, despawn_disconnected_clients);

        let (client_bundle, _helper) = create_mock_client("Azure");
        let entity = app.world_mut().spawn(client_bundle).id();
        app.world_mut().entity_mut(entity).insert((
            PlayerState::default(),
            dash_known_techniques(0.99),
            KnownTechniquesLoadFailed,
        ));

        app.world_mut().entity_mut(entity).remove::<Client>();
        app.update();

        assert_eq!(
            read_known_techniques_json(&db_path),
            "{not json",
            "挂写保护标记的实体断线时不得把会话内组件（0.99）写回覆盖损坏前的真实存档"
        );

        let _ = fs::remove_dir_all(&data_dir);
    }

    #[test]
    fn shutdown_flush_skips_known_techniques_when_load_failed_marker_present() {
        let (persistence, data_dir, db_path) =
            sqlite_persistence("known-techniques-shutdown-guard");
        seed_and_corrupt_known_techniques_row(&persistence);

        let mut app = App::default();
        app.insert_resource(persistence);
        app.insert_resource(PersistenceSettings::with_paths(
            &db_path,
            data_dir.join("deceased"),
            "player-known-techniques-shutdown-guard",
        ));
        app.add_systems(Last, flush_connected_players_on_shutdown);

        let (client_bundle, _helper) = create_mock_client("Azure");
        let entity = app.world_mut().spawn(client_bundle).id();
        app.world_mut().entity_mut(entity).insert((
            PlayerState::default(),
            dash_known_techniques(0.99),
            KnownTechniquesLoadFailed,
        ));

        app.world_mut().send_event(AppExit::Success);
        app.update();

        assert_eq!(
            read_known_techniques_json(&db_path),
            "{not json",
            "挂写保护标记的实体在停服 flush 时同样必须跳过功法落盘"
        );

        let _ = fs::remove_dir_all(&data_dir);
    }

    #[test]
    fn join_with_unopenable_db_marks_load_failed_and_recovery_preserves_row() {
        // 锁定「连接打不开」早退分支的全链路契约（review #1288 major finding）：
        // db_path 指向目录 → open_player_connection 必 SQLITE_CANTOPEN（稳定跨平台，
        // 不依赖权限行为）→ join 挂写保护标记；DB 恢复可访问后，带标记会话的
        // Changed flush 仍不得把 default/会话内数据写回覆盖真实存档；
        // 恢复后的新 join 则完整加载原行、不带标记。
        let data_dir = unique_temp_dir("known-techniques-cantopen-join");
        let healthy_db = data_dir.join("healthy.db");
        bootstrap_sqlite(&healthy_db, "player-mod-cantopen-join")
            .expect("sqlite bootstrap should succeed");
        let seed_persistence = PlayerStatePersistence::with_db_path(&data_dir, &healthy_db);
        crate::player::state::save_player_state(
            &seed_persistence,
            "Azure",
            &PlayerState::default(),
        )
        .expect("baseline player state should persist");
        crate::player::state::save_player_known_techniques_slice(
            &seed_persistence,
            "Azure",
            &dash_known_techniques(0.42),
        )
        .expect("seeding known techniques row should persist");

        // 运行时 persistence 指向 bong.db——先以同名目录占位，令连接打开必失败。
        let db_path = data_dir.join("bong.db");
        fs::create_dir_all(&db_path).expect("creating directory placeholder should succeed");
        let persistence = PlayerStatePersistence::with_db_path(&data_dir, &db_path);

        let mut app = App::new();
        app.insert_resource(persistence);
        app.add_systems(
            Update,
            (
                attach_player_state_to_joined_clients,
                flush_changed_player_known_techniques.after(attach_player_state_to_joined_clients),
            ),
        );

        let (client_bundle, _helper) = create_mock_client("Azure");
        let entity = app.world_mut().spawn(client_bundle).id();
        app.update();
        app.update();

        assert!(
            app.world()
                .get::<KnownTechniquesLoadFailed>(entity)
                .is_some(),
            "连接打不开（行状态不可知）的 join 应挂 KnownTechniquesLoadFailed 写保护标记"
        );
        assert_eq!(
            app.world()
                .get::<KnownTechniques>(entity)
                .expect("join should still attach a KnownTechniques component"),
            &KnownTechniques::default(),
            "连接失败时会话内组件应为 default（本次会话降级，但不得反向污染存档）"
        );

        // 模拟 DB 恢复：目录占位撤掉，真实健康库落位到同一路径。
        fs::remove_dir(&db_path).expect("removing directory placeholder should succeed");
        fs::rename(&healthy_db, &db_path).expect("restoring healthy db should succeed");

        // 带标记会话内的变更（0.99）不得写回：行必须保持恢复前的 0.42。
        app.world_mut()
            .entity_mut(entity)
            .insert(dash_known_techniques(0.99));
        app.update();
        assert!(
            (dash_proficiency_from_json(&read_known_techniques_json(&db_path)) - 0.42).abs() < 1e-6,
            "DB 恢复后，加载失败会话的 Changed flush 仍必须被写保护标记拦住，\
             期望行保持 0.42（真实存档），若被写成 0.99/空表即丢档回归"
        );

        // 恢复后的新 join（重连）应完整加载原行且不带标记——失败状态不粘滞。
        let (client_bundle2, _helper2) = create_mock_client("Azure");
        let entity2 = app.world_mut().spawn(client_bundle2).id();
        app.update();
        app.update();

        assert!(
            app.world()
                .get::<KnownTechniquesLoadFailed>(entity2)
                .is_none(),
            "DB 恢复后的新 join 不得再挂写保护标记"
        );
        assert_eq!(
            app.world()
                .get::<KnownTechniques>(entity2)
                .expect("recovered join should attach KnownTechniques"),
            &dash_known_techniques(0.42),
            "DB 恢复后的新 join 应完整加载原功法行（dash 0.42）"
        );

        let _ = fs::remove_dir_all(&data_dir);
    }

    #[test]
    fn shutdown_flush_persists_lifecycle_state_without_disconnect() {
        // bughunt player-lifecycle-relog-death-consequence-wipe：关服时的 flush 路径
        // （flush_connected_players_on_shutdown）与断线路径共享同一个漏洞面，必须同样
        // 落盘 Lifecycle，否则重启后重连会命中老档缺失行、回退到 Lifecycle::default()
        // 抹掉关服前的濒死/待复活状态。这里专注 NearDeath 分支（AwaitingRevival +
        // RevivalDecision 已由 disconnect_flush_persists_lifecycle_state_before_cleanup
        // 覆盖，避免重复断言）。
        use crate::combat::components::LifecycleState;

        let (persistence, data_dir, db_path) = sqlite_persistence("lifecycle-shutdown-flush");
        crate::player::state::save_player_state(&persistence, "Azure", &PlayerState::default())
            .expect("baseline player state should persist");

        let mut app = App::default();
        app.insert_resource(persistence);
        app.insert_resource(PersistenceSettings::with_paths(
            &db_path,
            data_dir.join("deceased"),
            "player-lifecycle-shutdown-flush",
        ));
        app.add_systems(Last, flush_connected_players_on_shutdown);

        let (mut client_bundle, _helper) = create_mock_client("Azure");
        client_bundle.player.position = Position::new([64.0, 80.0, -12.0]);
        let entity = app.world_mut().spawn(client_bundle).id();
        app.world_mut().entity_mut(entity).insert(PlayerState {
            karma: 0.0,
            inventory_score: 0.0,
        });
        app.world_mut().entity_mut(entity).insert(make_inventory());
        app.world_mut().entity_mut(entity).insert(Lifecycle {
            state: LifecycleState::NearDeath,
            fortune_remaining: 1,
            near_death_deadline_tick: Some(2_000),
            awaiting_decision: None,
            ..Lifecycle::default()
        });

        app.world_mut().send_event(AppExit::Success);
        app.update();

        let lifecycle_json = read_lifecycle_json(&db_path);
        let persisted: Lifecycle =
            serde_json::from_str(&lifecycle_json).expect("persisted lifecycle_json should decode");

        assert_eq!(
            persisted.state,
            LifecycleState::NearDeath,
            "关服前的 NearDeath 濒死状态必须落盘"
        );
        assert_eq!(persisted.fortune_remaining, 1);
        assert_eq!(persisted.near_death_deadline_tick, Some(2_000));
        assert_eq!(persisted.awaiting_decision, None);

        let _ = fs::remove_dir_all(&data_dir);
    }

    #[test]
    fn reconnecting_into_tsy_routes_layer_and_current_dimension() {
        use crate::world::dimension::{DimensionKind, DimensionLayers};

        let (persistence, data_dir, _db_path) = sqlite_persistence("reconnect-into-tsy");

        // Persist a player whose last dimension is Tsy so reconnect should
        // route them back into the TSY layer rather than the overworld default.
        crate::player::state::save_player_slices(
            &persistence,
            "Azure",
            &PlayerState::default(),
            [12.0, 80.0, -34.0],
            DimensionKind::Tsy,
            None,
            None,
            &SkillSet::default(),
        )
        .expect("seeding TSY-resident player should persist");

        let mut app = App::new();
        let overworld_layer = app.world_mut().spawn_empty().id();
        let tsy_layer = app.world_mut().spawn_empty().id();
        app.insert_resource(DimensionLayers {
            overworld: overworld_layer,
            tsy: tsy_layer,
        });
        app.insert_resource(persistence);
        app.add_systems(Update, attach_player_state_to_joined_clients);

        // Mock client bundle: Added<Client> fires this tick. Pre-set its layer
        // pointers to the overworld so we can verify attach reroutes them.
        let (mut client_bundle, _helper) = valence::testing::create_mock_client("Azure");
        client_bundle.player.layer.0 = overworld_layer;
        client_bundle.visible_chunk_layer.0 = overworld_layer;
        client_bundle
            .visible_entity_layers
            .0
            .insert(overworld_layer);
        let entity = app.world_mut().spawn(client_bundle).id();

        app.update();

        let world = app.world();
        let er = world.entity(entity);
        let current = er
            .get::<CurrentDimension>()
            .copied()
            .expect("attach should insert CurrentDimension");
        let layer_id = er
            .get::<EntityLayerId>()
            .expect("client bundle should carry EntityLayerId")
            .0;
        let visible_chunk = er
            .get::<VisibleChunkLayer>()
            .expect("client bundle should carry VisibleChunkLayer")
            .0;
        let visible_entities = &er
            .get::<VisibleEntityLayers>()
            .expect("client bundle should carry VisibleEntityLayers")
            .0;
        let position = er.get::<Position>().expect("position should be set").get();

        assert_eq!(current, CurrentDimension(DimensionKind::Tsy));
        assert_eq!(layer_id, tsy_layer);
        assert_eq!(visible_chunk, tsy_layer);
        assert!(visible_entities.contains(&tsy_layer));
        assert!(!visible_entities.contains(&overworld_layer));
        assert_eq!(position, DVec3::new(12.0, 80.0, -34.0));

        let _ = fs::remove_dir_all(&data_dir);
    }

    #[test]
    fn restored_login_position_is_visible_to_followup_update_system_same_frame() {
        let (persistence, data_dir, _db_path) = sqlite_persistence("login-position-same-frame");
        crate::player::state::save_player_slices(
            &persistence,
            "Azure",
            &PlayerState::default(),
            [512.0, 96.0, -768.0],
            DimensionKind::default(),
            None,
            None,
            &SkillSet::default(),
        )
        .expect("seeding restored player position should persist");

        let mut app = App::new();
        let overworld_layer = app.world_mut().spawn_empty().id();
        let tsy_layer = app.world_mut().spawn_empty().id();
        app.insert_resource(DimensionLayers {
            overworld: overworld_layer,
            tsy: tsy_layer,
        });
        app.insert_resource(persistence);
        app.insert_resource(CapturedLoginPosition::default());
        app.add_systems(
            Update,
            (
                attach_player_state_to_joined_clients,
                capture_login_position_after_attach.after(attach_player_state_to_joined_clients),
            ),
        );

        let (mut client_bundle, _helper) = valence::testing::create_mock_client("Azure");
        client_bundle.player.position = Position::new(crate::player::spawn_position());
        client_bundle.player.layer.0 = overworld_layer;
        client_bundle.visible_chunk_layer.0 = overworld_layer;
        client_bundle
            .visible_entity_layers
            .0
            .insert(overworld_layer);
        app.world_mut().spawn(client_bundle);

        app.update();

        let captured = app.world().resource::<CapturedLoginPosition>();
        assert_eq!(captured.0, Some([512.0, 96.0, -768.0]));

        let _ = fs::remove_dir_all(&data_dir);
    }
}
