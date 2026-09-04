pub mod gameplay;
pub mod home_return;
pub mod spawn_selector;
pub mod state;

use self::state::{
    canonical_player_id, load_player_slices_for_canonical_techniques, save_player_core_slice,
    save_player_inventory_slice, save_player_lifecycle_slice,
    save_player_lifespan_slice_with_coffin, save_player_skill_slice,
    save_player_slices_with_coffin, save_player_slow_slice, update_player_ui_prefs, PlayerState,
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
use crate::cultivation::known_techniques::TechniqueRegistry;
use crate::cultivation::life_record::LifeRecord;
use crate::cultivation::lifespan::LifespanComponent;
use crate::cultivation::meridian::severed::MeridianSeveredPermanent;
use crate::cultivation::poison_trait::{DigestionLoad, PoisonToxicity};
use crate::inventory::{attach_inventory_to_joined_clients, PlayerInventory};
use crate::persistence::persist_player_cultivation_bundle;
use crate::persistence::PersistenceSettings;
use crate::skill::components::SkillSet;
use crate::skill::config::{SkillConfigSchemas, SkillConfigStore};
use crate::world::dimension::{CurrentDimension, DimensionKind, DimensionLayers};
use crate::world::spawn_tutorial::TutorialState;
use valence::entity::entity::Flags;
use valence::message::SendMessage;
use valence::prelude::bevy_ecs::system::SystemParam;
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

type ClientInitQueryFilter = (
    Or<(
        Added<Client>,
        Added<crate::cultivation::known_techniques::KnownTechniquesReconnectReady>,
    )>,
    Without<crate::cultivation::known_techniques::KnownTechniquesReconnectBlocked>,
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
type JoinedClientsWithoutStateQueryFilter = (
    Or<(
        Added<Client>,
        Added<crate::cultivation::known_techniques::KnownTechniquesReconnectReady>,
        Added<ReconnectPersistencePending>,
    )>,
    Without<PlayerState>,
    Without<crate::cultivation::known_techniques::KnownTechniquesReconnectBlocked>,
);

#[derive(SystemParam)]
pub(crate) struct PlayerAttachResources<'w> {
    skill_config_store: Option<ResMut<'w, SkillConfigStore>>,
    skill_config_schemas: Option<Res<'w, SkillConfigSchemas>>,
    technique_registry: Option<Res<'w, TechniqueRegistry>>,
}

#[derive(Component, Default)]
struct InventoryPersistenceDirty;

/// Same-username reconnects wait one frame while the old disconnected entity's final persistence
/// checkpoint runs. This marker is consumed by the normal join attach system on the next frame.
#[derive(Component, Default)]
pub(crate) struct ReconnectPersistencePending;

type ChangedInventoryClientsQueryItem<'a> = (Entity, &'a Username, &'a PlayerInventory);
type ChangedInventoryClientsQueryFilter = (
    With<Client>,
    Without<crate::network::craft_emit::CraftSessionPersistenceDirty>,
    Or<(Changed<PlayerInventory>, With<InventoryPersistenceDirty>)>,
);
type ChangedSkillClientsQueryItem<'a> = (&'a Username, &'a SkillSet);
type ChangedSkillClientsQueryFilter = (With<Client>, Changed<SkillSet>);
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

/// fix-spec-1901-v2 §4.2 — 出生/重连位置提交进入统一移动 commit set；灵田
/// post-transfer validator / completion 复验排在其后。生产 `register()` 与回归测试
/// 共用此注册路径：测试不得在本地重建 set 会员，否则生产注册丢失 membership
/// 时测试仍会绿，无法发现调度契约退化。
pub(crate) fn register_authoritative_position_commit_systems(app: &mut App) {
    app.add_systems(
        Update,
        (
            init_clients.in_set(crate::world::movement_commit::AuthoritativePositionCommitSet),
            attach_player_state_to_joined_clients
                .after(init_clients)
                .in_set(crate::world::movement_commit::AuthoritativePositionCommitSet),
        ),
    );
}

pub fn register(app: &mut App) {
    tracing::info!("[bong][player] registering player init/cleanup systems");
    app.insert_resource(PlayerStatePersistence::default());
    app.insert_resource(PlayerStateAutosaveTimer::default());
    gameplay::register(app);
    home_return::register(app);
    register_authoritative_position_commit_systems(app);
    app.add_systems(
        Update,
        (
            attach_inventory_to_joined_clients.after(attach_player_state_to_joined_clients),
            tick_player_persistence_timer,
            autosave_player_core_slices.after(tick_player_persistence_timer),
            autosave_player_slow_and_ui_slices.after(autosave_player_core_slices),
            autosave_player_cultivation_bundles.after(autosave_player_slow_and_ui_slices),
            autosave_player_lifespan_slices.after(autosave_player_cultivation_bundles),
            autosave_player_lifecycle_slices.after(autosave_player_lifespan_slices),
            flush_changed_player_skills.after(autosave_player_lifecycle_slices),
            flush_changed_player_inventories
                .after(attach_inventory_to_joined_clients)
                .after(flush_changed_player_skills)
                .after(crate::network::craft_emit::persist_dirty_craft_sessions),
            despawn_disconnected_clients
                .after(flush_changed_player_inventories)
                .after(crate::persistence::dispatch_known_techniques_reconnects),
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

pub(crate) fn init_clients(
    mut commands: Commands,
    mut clients: Query<ClientInitQueryItem<'_>, ClientInitQueryFilter>,
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
    mut resources: PlayerAttachResources<'_>,
    pending_disconnects: Query<&Username, (Without<Client>, Without<Despawned>)>,
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
        if pending_disconnects
            .iter()
            .any(|disconnected| disconnected.0 == username.0)
        {
            commands.entity(entity).insert(ReconnectPersistencePending);
            continue;
        }

        commands
            .entity(entity)
            .remove::<ReconnectPersistencePending>();
        let persisted =
            load_player_slices_for_canonical_techniques(&persistence, username.0.as_str());
        let restored_inventory = persisted.inventory.is_some();
        let restored_lifespan = persisted.lifespan.is_some();
        let restored_skill = !persisted.skill_set.skills.is_empty()
            || !persisted.skill_set.consumed_scrolls.is_empty();
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

        let mut ui_prefs = persisted.ui_prefs.clone();
        let skill_bar_prefs_sanitized = resources
            .technique_registry
            .as_deref()
            .is_some_and(|registry| ui_prefs.sanitize_skill_bar_bindings(registry));
        if skill_bar_prefs_sanitized {
            let sanitized_skill_bar = ui_prefs.skill_bar.clone();
            if let Err(error) = update_player_ui_prefs(&persistence, username.0.as_str(), |prefs| {
                prefs.skill_bar = sanitized_skill_bar
            }) {
                tracing::warn!(
                    "[bong][player] failed to persist sanitized skill-bar bindings for `{}`: {error}",
                    username.0
                );
            }
        }
        let quick_slot_bindings = ui_prefs.quick_slot_bindings(persisted.inventory.as_ref());
        let skill_bar_bindings = ui_prefs.skill_bar_bindings(
            persisted.inventory.as_ref(),
            resources.technique_registry.as_deref(),
        );
        if let (Some(store), Some(schemas)) = (
            resources.skill_config_store.as_deref_mut(),
            resources.skill_config_schemas.as_deref(),
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
        ));
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
            "[bong][player] attached PlayerState to client entity {entity:?} for `{}` (composite_power={composite_power:.3}, restored_inventory={restored_inventory}, restored_lifespan={restored_lifespan}, restored_skill={restored_skill}, last_dimension={last_dimension:?})",
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
    core_players: Query<(
        &Username,
        &PlayerState,
        &Position,
        Option<&CurrentDimension>,
        Option<&PlayerInventory>,
        Option<&LifespanComponent>,
        Option<&SkillSet>,
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
            coffin,
            craft_session,
            lifecycle,
        )) = core_players.get(entity)
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
                        "[bong][player] failed to persist cultivation bundle for disconnected client `{}`: {error}",
                        username.0,
                    );
                }
            }
            // Valence detects a closed TCP connection asynchronously. During that window the
            // stale ECS entity still has `Client`, so an active CraftSession may advance a few
            // ticks after the bot has already disconnected. The periodic craft checkpoint is
            // the last authoritative in-game progress; prefer it for the disconnect flush so
            // reconnect cannot turn network-detection latency into free crafting time.
            let durable_craft_session =
                load_player_slices_for_canonical_techniques(&persistence, username.0.as_str())
                    .craft_session;
            let craft_session_for_disconnect = durable_craft_session.as_ref().or(craft_session);
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
                craft_session_for_disconnect,
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
        app.insert_resource(PersistenceSettings::with_db_path(
            &db_path,
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
        app.insert_resource(PersistenceSettings::with_db_path(
            &db_path,
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
        app.insert_resource(PersistenceSettings::with_db_path(
            &db_path,
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
    fn disconnect_flush_persists_latest_player_slices_before_cleanup() {
        let (persistence, data_dir, db_path) = sqlite_persistence("disconnect-flush");
        crate::player::state::save_player_state(&persistence, "Azure", &PlayerState::default())
            .expect("baseline player state should persist");

        let mut app = App::new();
        app.insert_resource(persistence);
        app.insert_resource(PersistenceSettings::with_db_path(
            &db_path,
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

        app.world_mut().entity_mut(entity).remove::<Client>();
        app.update();

        let (karma, inventory_score) = read_core_snapshot(&db_path);
        let (pos_x, pos_y, pos_z) = read_position_snapshot(&db_path);
        let inventory_json = read_inventory_json(&db_path);

        assert_eq!(karma, -0.15);
        assert_eq!(inventory_score, 0.7);
        assert_eq!((pos_x, pos_y, pos_z), (42.0, 77.0, -3.5));
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
    fn disconnect_flush_does_not_advance_craft_from_stale_ecs_session() {
        let (persistence, data_dir, db_path) = sqlite_persistence("disconnect-craft-checkpoint");
        crate::player::state::save_player_state(&persistence, "Azure", &PlayerState::default())
            .expect("baseline player state should persist");
        let inventory = make_inventory();
        let durable_session = CraftSession {
            recipe_id: crate::craft::RecipeId::new("craft.test.disconnect"),
            started_at_tick: 10,
            remaining_ticks: 37,
            total_ticks: 40,
            owner_player_id: canonical_player_id("Azure"),
            qi_paid: 0.0,
            quantity_total: 1,
            completed_count: 0,
        };
        crate::player::state::save_player_inventory_and_craft_session_slices(
            &persistence,
            "Azure",
            Some(&inventory),
            Some(&durable_session),
        )
        .expect("durable craft checkpoint should persist");

        let mut app = App::new();
        app.insert_resource(persistence);
        app.insert_resource(PersistenceSettings::with_db_path(
            &db_path,
            "player-disconnect-craft-checkpoint",
        ));
        app.add_systems(Update, despawn_disconnected_clients);

        let (client_bundle, _helper) = create_mock_client("Azure");
        let entity = app.world_mut().spawn(client_bundle).id();
        app.world_mut().entity_mut(entity).insert((
            PlayerState::default(),
            inventory,
            CraftSession {
                remaining_ticks: 35,
                ..durable_session.clone()
            },
        ));
        app.world_mut().entity_mut(entity).remove::<Client>();
        app.update();

        let reloaded = crate::player::state::load_player_slices(
            app.world().resource::<PlayerStatePersistence>(),
            "Azure",
        );
        assert_eq!(
            reloaded.craft_session.as_ref(),
            Some(&durable_session),
            "断线检测延迟不能把 stale ECS CraftSession 的进度写成免费制作时间"
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
        app.insert_resource(PersistenceSettings::with_db_path(
            &db_path,
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
        app.insert_resource(PersistenceSettings::with_db_path(
            &db_path,
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
        app.insert_resource(PersistenceSettings::with_db_path(
            &db_path,
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
        app.insert_resource(PersistenceSettings::with_db_path(
            &db_path,
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
        app.insert_resource(PersistenceSettings::with_db_path(
            &db_path,
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

        app.world_mut().send_event(AppExit::Success);
        app.update();

        let (karma, inventory_score) = read_core_snapshot(&db_path);
        let (pos_x, pos_y, pos_z) = read_position_snapshot(&db_path);
        let inventory_json = read_inventory_json(&db_path);

        assert_eq!(karma, 0.33);
        assert_eq!(inventory_score, 0.85);
        assert_eq!((pos_x, pos_y, pos_z), (64.0, 80.0, -12.0));
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
        app.insert_resource(PersistenceSettings::with_db_path(
            &db_path,
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

    #[test]
    fn production_register_places_restored_position_attach_in_authoritative_commit_set() {
        use crate::world::movement_commit::AuthoritativePositionCommitSet;
        use valence::prelude::SystemSet;

        let mut app = App::new();
        crate::player::register(&mut app);

        let schedule = app
            .get_schedule(Update)
            .expect("player::register 必须创建 Update 调度");
        let graph = schedule.graph();
        let attach_name = std::any::type_name_of_val(&attach_player_state_to_joined_clients);
        let attach_nodes: Vec<_> = graph
            .systems()
            .filter_map(|(node, system, _)| (system.name().as_ref() == attach_name).then_some(node))
            .collect();
        assert_eq!(
            attach_nodes.len(),
            1,
            "生产 Update 调度必须恰好注册一次 `{attach_name}`，实际 {} 次",
            attach_nodes.len()
        );

        let commit_set_nodes: Vec<_> = graph
            .system_sets()
            .filter_map(|(node, set, _)| {
                set.as_dyn_eq()
                    .dyn_eq(AuthoritativePositionCommitSet.as_dyn_eq())
                    .then_some(node)
            })
            .collect();
        assert_eq!(
            commit_set_nodes.len(),
            1,
            "生产 Update 调度必须恰好包含一个 AuthoritativePositionCommitSet，实际 {} 个",
            commit_set_nodes.len()
        );
        assert!(
            graph
                .hierarchy()
                .graph()
                .contains_edge(commit_set_nodes[0], attach_nodes[0]),
            "player::register 必须把 `{attach_name}` 直接放入 AuthoritativePositionCommitSet；\
             仅靠运行时 sibling 调度顺序不能保证重连位置先于灵田验证提交"
        );
    }

    #[test]
    fn reconnecting_restored_position_commits_before_lingtian_post_transfer_validation() {
        // fix-spec-1901-v2 #10：生产注册把 attach_player_state_to_joined_clients 放进
        // AuthoritativePositionCommitSet，灵田 post-transfer validator 排在 set 之后。
        // 本测试通过生产注册入口 player::register 获得 attach 的 set 会员，不在此地
        // 重建；attach 先注册、validator 后注册且不写 .after(attach)。未声明依赖的
        // sibling 系统执行顺序不受注册顺序保证，因此上方结构测试直接锁定 set 会员边，
        // 本测试只负责锁定完整重连行为（central review 1984-31447628937 finding [2]）。
        use crate::lingtian::events::{
            StartDrainQiRequest, StartHarvestRequest, StartPlantingRequest, StartRenewRequest,
            StartReplenishRequest, StartTillRequest,
        };
        use crate::lingtian::requests::{PendingLingtianRequest, PendingLingtianRequests};
        use crate::lingtian::session::SessionMode;
        use crate::lingtian::systems::validate_and_dispatch_lingtian_requests;
        use crate::world::dimension::DimensionKind;
        use crate::world::movement_commit::AuthoritativePositionCommitSet;
        use valence::prelude::{BlockPos, Events};

        let (persistence, data_dir, _db_path) = sqlite_persistence("reconnect-lingtian-gate");

        // 存档玩家上次离线在灵田目标旁（Overworld，目标 (0,64,0) 中心 (0.5,64.5,0.5)，
        // 玩家 (2.5,64.5,0.5) 距离 2.0，位于共享灵田交互 reach profile 内）。
        crate::player::state::save_player_slices(
            &persistence,
            "Azure",
            &PlayerState::default(),
            [2.5, 64.5, 0.5],
            DimensionKind::Overworld,
            None,
            None,
            &SkillSet::default(),
        )
        .expect("seeding nearby-resident player should persist");

        let mut app = App::new();
        // 走生产注册入口 player::register（central review 1984-31447628937
        // finding [2]）：attach 的 AuthoritativePositionCommitSet 会员与
        // PlayerStatePersistence 资源都由生产 register 提供，测试不在本地重建。
        // 直接调 register_authoritative_position_commit_systems 会让「生产 register
        // 丢失 membership」假绿（删掉 register 里的 helper 调用后测试仍因手动注入
        // 而通过）。register 先跑，随后用测试自己的 sqlite persistence 覆盖
        // register 插入的 default 资源，保证位置恢复读到的是测试存档。
        crate::player::register(&mut app);
        // player::register 注册的整套系统在裸 App 里需要以下资源/事件（生产由 main
        // 的 inventory/persistence/combat 注册提供）：bevy 0.14 对缺失的硬 Res /
        // 事件资源在系统运行时报 panic，缺一个 app.update() 即崩。只补存活前提
        // （空 registry/空 loadout/默认 allocator/settings），不重建 set 会员——
        // 顺序契约仍完全由生产 register 的 set 边提供。
        app.insert_resource(crate::inventory::ItemRegistry::default());
        app.insert_resource(crate::inventory::DefaultLoadout(
            crate::inventory::LoadoutSpec {
                containers: Vec::new(),
                equipped: HashMap::new(),
                hotbar: Default::default(),
                bone_coins: 0,
                max_weight: 0.0,
            },
        ));
        app.insert_resource(crate::inventory::InventoryInstanceIdAllocator::default());
        app.insert_resource(crate::persistence::PersistenceSettings::default());
        app.add_event::<crate::combat::events::AttackIntent>();
        app.add_event::<crate::cultivation::breakthrough::BreakthroughRequest>();
        app.insert_resource(persistence)
            .init_resource::<PendingLingtianRequests>()
            .add_event::<StartTillRequest>()
            .add_event::<StartRenewRequest>()
            .add_event::<StartPlantingRequest>()
            .add_event::<StartHarvestRequest>()
            .add_event::<StartReplenishRequest>()
            .add_event::<StartDrainQiRequest>();
        app.add_systems(
            Update,
            validate_and_dispatch_lingtian_requests.after(AuthoritativePositionCommitSet),
        );

        // Mock 客户端起点在远处（1000, 64.5, 1000）——若 attach 不在 commit set 内，
        // validator 会读到这个远点并拒绝请求。
        let (mut client_bundle, _helper) = create_mock_client("Azure");
        client_bundle.player.position = Position::new([1000.0, 64.5, 1000.0]);
        let entity = app.world_mut().spawn(client_bundle).id();

        app.world_mut()
            .resource_mut::<PendingLingtianRequests>()
            .push(PendingLingtianRequest::Till {
                actor: entity,
                pos: BlockPos::new(0, 64, 0),
                hoe_instance_id: 7,
                mode: SessionMode::Manual,
            });

        app.update();

        let start_events = app.world().resource::<Events<StartTillRequest>>();
        let mut reader = start_events.get_reader();
        let dispatched: Vec<_> = reader.read(start_events).collect();
        assert_eq!(
            dispatched.len(),
            1,
            "重连恢复的存档位置必须在 post-transfer 验证前提交；期望 1 条 StartTillRequest \
             （距目标 2.0 在 4.5 内），实际 {} 条——attach 若不在 \
             AuthoritativePositionCommitSet 内就会读到远处位置拒绝",
            dispatched.len()
        );
        assert_eq!(dispatched[0].player, entity);
        assert_eq!(dispatched[0].pos, BlockPos::new(0, 64, 0));

        let _ = fs::remove_dir_all(&data_dir);
    }
}
