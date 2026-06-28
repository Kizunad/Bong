use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use valence::entity::entity::Flags;
use valence::message::SendMessage;
use valence::prelude::{
    apply_deferred, bevy_ecs, Added, App, BlockPos, BlockState, ChunkLayer, Client, Commands,
    Component, DVec3, Despawned, Entity, Event, EventReader, EventWriter, IntoSystemConfigs,
    Position, Query, Res, ResMut, Resource, SneakEvent, SneakState, Update, Username, With,
};

use crate::combat::components::TICKS_PER_SECOND;
use crate::combat::CombatClock;
use crate::craft::{
    CraftCategory, CraftRecipe, CraftRegistry, CraftRequirements, RecipeId, RegistryError,
    UnlockSource,
};
use crate::cultivation::components::Cultivation;
use crate::cultivation::lifespan::LifespanComponent;
use crate::inventory::{
    add_item_to_player_inventory, consume_item_instance_once, inventory_item_by_instance,
    InventoryInstanceIdAllocator, ItemRegistry, PlayerInventory,
};
use crate::network::agent_bridge::{payload_type_label, serialize_server_data_payload};
use crate::network::audio_event_emit::{AudioRecipient, PlaySoundRecipeRequest};
use crate::network::inventory_snapshot_emit::send_inventory_snapshot_to_client;
use crate::network::{log_payload_build_error, send_server_data_payload};
use crate::player::state::PlayerState;
use crate::schema::server_data::{CoffinGradeV1, CoffinStateV1, ServerDataPayloadV1, ServerDataV1};
pub const MUNDANE_COFFIN_ITEM_ID: &str = "mundane_coffin";
/// Legacy constant kept for backward compat; use `CoffinGrade::Mundane.lifespan_factor()` in new code.
#[allow(dead_code)]
pub const COFFIN_LIFESPAN_FACTOR: f64 = 0.9;

/// 延寿棺灵材档级：凡木 / 寒玉 / 玄石 / 青铜（plan-coffin-tiers-v1 P0）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoffinGrade {
    #[default]
    Mundane,
    Jade,
    Stone,
    Bronze,
}

impl CoffinGrade {
    /// 寿元倍率：凡木 0.9 / 寒玉 0.7 / 玄石 0.5 / 青铜 0.3
    pub fn lifespan_factor(self) -> f64 {
        match self {
            CoffinGrade::Mundane => 0.9,
            CoffinGrade::Jade => 0.7,
            CoffinGrade::Stone => 0.5,
            CoffinGrade::Bronze => 0.3,
        }
    }

    /// 对应 ItemTemplate id
    pub fn item_id(self) -> &'static str {
        match self {
            CoffinGrade::Mundane => "mundane_coffin",
            CoffinGrade::Jade => "jade_coffin",
            CoffinGrade::Stone => "stone_coffin",
            CoffinGrade::Bronze => "bronze_coffin",
        }
    }

    /// 从 item_id 字符串反查档级
    pub fn from_item_id(id: &str) -> Option<CoffinGrade> {
        match id {
            "mundane_coffin" => Some(CoffinGrade::Mundane),
            "jade_coffin" => Some(CoffinGrade::Jade),
            "stone_coffin" => Some(CoffinGrade::Stone),
            "bronze_coffin" => Some(CoffinGrade::Bronze),
            _ => None,
        }
    }

    /// SQLite 持久化标签（snake_case 与 serde 对齐）
    pub fn as_db_str(self) -> &'static str {
        match self {
            CoffinGrade::Mundane => "mundane",
            CoffinGrade::Jade => "jade",
            CoffinGrade::Stone => "stone",
            CoffinGrade::Bronze => "bronze",
        }
    }

    /// 从 SQLite 读取标签，缺失或未知 → Mundane
    pub fn from_db_str(s: &str) -> CoffinGrade {
        match s {
            "jade" => CoffinGrade::Jade,
            "stone" => CoffinGrade::Stone,
            "bronze" => CoffinGrade::Bronze,
            _ => CoffinGrade::Mundane,
        }
    }
}

const COFFIN_AMBIENT_INTERVAL_TICKS: u64 = 3 * TICKS_PER_SECOND;
const COFFIN_INTERACT_MAX_DISTANCE_SQ: f64 = 36.0;
/// plan-coffin-tiers-v1 P2 — marker 渲染 state：0 = intact（在棺 / 掀盖 state 待 P3）。
const COFFIN_MARKER_VISUAL_STATE: u8 = 0;

#[derive(Debug, Clone, Copy, Component, PartialEq, Eq)]
pub struct CoffinComponent {
    pub entered_at_tick: u64,
    pub coffin_lower: BlockPos,
    pub grade: CoffinGrade,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CoffinEntity {
    pub lower: BlockPos,
    pub upper: BlockPos,
    pub occupied_by: Option<Entity>,
    pub placed_at_tick: u64,
    pub grade: CoffinGrade,
    /// plan-coffin-tiers-v1 P2 — 该棺的渲染 marker 实体 id（替代旧双 CHEST）。
    /// `None` = marker 尚未 spawn / 已 despawn（重连恢复的棺在 recovery 系统重建前
    /// 为 None）；recovery 系统据此判定哪些棺缺 marker 需补 spawn。
    pub marker_entity: Option<Entity>,
}

#[derive(Debug, Default, Resource)]
pub struct CoffinRegistry {
    pub coffins: HashMap<BlockPos, CoffinEntity>,
    pub player_in_coffin: HashMap<Entity, BlockPos>,
}

impl CoffinRegistry {
    pub fn insert(&mut self, lower: BlockPos, placed_at_tick: u64, grade: CoffinGrade) -> bool {
        let upper = coffin_upper_half(lower);
        if self.coffins.contains_key(&lower) || self.coffins.contains_key(&upper) {
            return false;
        }

        let coffin = CoffinEntity {
            lower,
            upper,
            occupied_by: None,
            placed_at_tick,
            grade,
            marker_entity: None,
        };
        self.write_coffin(coffin);
        true
    }

    pub fn lookup(&self, pos: BlockPos) -> Option<CoffinEntity> {
        self.coffins.get(&pos).copied()
    }

    /// plan-coffin-tiers-v1 P4 — `/coffin grade` dev 命令用：直写档级并同步双索引。
    /// 棺不存在则忽略（返回 false）。
    /// dev-only：绕过 worldview 修炼规则，不允许生产路径复用。
    pub fn set_grade(&mut self, lower: BlockPos, grade: CoffinGrade) -> bool {
        let Some(mut coffin) = self.lookup(lower) else {
            return false;
        };
        coffin.grade = grade;
        self.write_coffin(coffin);
        true
    }

    /// plan-coffin-tiers-v1 P2 — 记录某棺的渲染 marker 实体 id（放置 / recovery 重建后调）。
    /// 同步更新 lower / upper 两条索引。棺不存在则忽略（返回 false）。
    pub fn set_marker_entity(&mut self, lower: BlockPos, marker: Option<Entity>) -> bool {
        let Some(mut coffin) = self.lookup(lower) else {
            return false;
        };
        coffin.marker_entity = marker;
        self.write_coffin(coffin);
        true
    }

    /// plan-coffin-tiers-v1 P2 — 遍历所有去重后的棺记录（lower / upper 双索引去重到 lower）。
    /// recovery 系统据此判定哪些棺缺 marker；marker 重建后回填 `set_marker_entity`。
    pub fn iter_unique(&self) -> impl Iterator<Item = CoffinEntity> + '_ {
        self.coffins
            .iter()
            .filter(|(pos, coffin)| **pos == coffin.lower)
            .map(|(_, coffin)| *coffin)
    }

    pub fn set_occupied(&mut self, lower: BlockPos, player: Entity) -> bool {
        let Some(mut coffin) = self.lookup(lower) else {
            return false;
        };
        if coffin.occupied_by.is_some() {
            return false;
        }

        coffin.occupied_by = Some(player);
        self.write_coffin(coffin);
        self.player_in_coffin.insert(player, coffin.lower);
        true
    }

    pub fn reclaim_occupied(
        &mut self,
        lower: BlockPos,
        player: Entity,
        placed_at_tick: u64,
        grade: CoffinGrade,
    ) {
        let mut coffin = self.lookup(lower).unwrap_or(CoffinEntity {
            lower,
            upper: coffin_upper_half(lower),
            occupied_by: None,
            placed_at_tick,
            grade,
            marker_entity: None,
        });
        if let Some(previous_player) = coffin.occupied_by {
            self.player_in_coffin.remove(&previous_player);
        }

        coffin.occupied_by = Some(player);
        self.write_coffin(coffin);
        self.player_in_coffin.insert(player, coffin.lower);
    }

    pub fn clear_player(&mut self, player: Entity) -> Option<BlockPos> {
        let lower = self.player_in_coffin.remove(&player)?;
        if let Some(mut coffin) = self.lookup(lower) {
            if coffin.occupied_by == Some(player) {
                coffin.occupied_by = None;
                self.write_coffin(coffin);
            }
        }
        Some(lower)
    }

    pub fn remove_by_pos(&mut self, pos: BlockPos) -> Option<CoffinEntity> {
        let coffin = self.lookup(pos)?;
        self.coffins.remove(&coffin.lower);
        self.coffins.remove(&coffin.upper);
        if let Some(player) = coffin.occupied_by {
            self.player_in_coffin.remove(&player);
        }
        Some(coffin)
    }

    fn write_coffin(&mut self, coffin: CoffinEntity) {
        self.coffins.insert(coffin.lower, coffin);
        self.coffins.insert(coffin.upper, coffin);
    }
}

#[derive(Debug, Clone, Event)]
pub struct CoffinPlaceRequest {
    pub player: Entity,
    pub pos: BlockPos,
    pub item_instance_id: u64,
    pub tick: u64,
}

#[derive(Debug, Clone, Event)]
pub struct CoffinEnterRequest {
    pub player: Entity,
    pub pos: BlockPos,
    pub tick: u64,
}

#[derive(Debug, Clone, Event)]
pub struct CoffinLeaveRequest {
    pub player: Entity,
}

/// plan-coffin-tiers-v1 P2 §8.1 #3 — G 菜单 [回收] 请求。
///
/// **P2 范围**：server 侧处理函数 + 该 intent event 就位，可被 dev / 测试驱动。
/// G 菜单的 client UI（`CoffinMenuScreen`）+ C2S intent payload 接线是 **P3**；
/// P2 只锁定 server 端纯逻辑 + event 契约（接线时只换 emit 源，不改处理逻辑）。
#[derive(Debug, Clone, Event)]
pub struct CoffinMenuReclaimRequest {
    pub player: Entity,
    /// 目标棺的任意一格坐标（lower 或 upper 均可，registry 自动归一）。
    pub pos: BlockPos,
    pub tick: u64,
}

/// plan-coffin-tiers-v1 P2 — 破坏延寿棺请求（左键攻击 marker 实体，体验同破坏方块）。
///
/// **P2 范围**：server 侧处理 + event 契约就位。client 侧"攻击 marker 实体 → 发该 C2S intent"
/// 接线是 **P3**（marker 实体无方块，不能走 DiggingEvent，必须走实体攻击 intent）。
#[derive(Debug, Clone, Event)]
pub struct CoffinBreakRequest {
    pub player: Entity,
    /// 目标棺任意一格坐标（registry 自动归一）。
    pub pos: BlockPos,
    pub tick: u64,
}

#[derive(Debug, Clone, Event)]
pub struct CoffinStateChanged {
    pub player: Entity,
    /// None = 离棺，Some(grade) = 在棺内（grade 为当前档级）
    pub grade: Option<CoffinGrade>,
}

impl CoffinStateChanged {
    #[allow(dead_code)]
    pub fn in_coffin(&self) -> bool {
        self.grade.is_some()
    }
}

pub fn register(app: &mut App) {
    tracing::info!("[bong][coffin] registering mundane coffin subsystem (plan-coffin-v1)");
    app.insert_resource(CoffinRegistry::default());
    app.add_event::<CoffinPlaceRequest>();
    app.add_event::<CoffinEnterRequest>();
    app.add_event::<CoffinLeaveRequest>();
    app.add_event::<CoffinMenuReclaimRequest>();
    app.add_event::<CoffinBreakRequest>();
    app.add_event::<CoffinStateChanged>();
    app.add_systems(
        Update,
        (
            handle_coffin_place_requests,
            handle_coffin_enter_requests,
            handle_coffin_leave_requests,
            handle_sneak_leave_requests,
            handle_coffin_breaks,
            handle_coffin_menu_reclaim,
            emit_coffin_ambient_audio,
        )
            .after(crate::network::client_request_handler::handle_client_request_payloads)
            .before(crate::network::audio_event_emit::emit_audio_play_payloads),
    );
    app.add_systems(
        Update,
        // recovery：重连 / 区块加载后为缺 marker 的已注册棺重建渲染实体（grade 对齐）。
        // 排在放置 / 破坏 / 回收**之后** + `apply_deferred` flush，确保那些系统当帧
        // spawn/despawn 的 marker 已生效，recovery 的存活性查询不会把刚放置的 marker
        // 误判为缺失而重复 spawn。
        (apply_deferred, rebuild_missing_coffin_markers)
            .chain()
            .after(handle_coffin_place_requests)
            .after(handle_coffin_breaks)
            .after(handle_coffin_menu_reclaim),
    );
    app.add_systems(
        Update,
        (
            pin_coffin_players,
            emit_coffin_state_payloads,
            emit_coffin_state_to_joined_clients
                .after(crate::player::attach_player_state_to_joined_clients),
        ),
    );
}

pub fn register_craft_recipes(registry: &mut CraftRegistry) -> Result<(), RegistryError> {
    // 凡木棺 ×0.9 — 手搓（station: None），Scroll 解锁
    // 寒玉/玄石/青铜棺 workbench 配方由 craft::workbench_recipes 统一注册（plan-coffin-tiers-v1 P4）
    registry.register(CraftRecipe {
        id: RecipeId::new("coffin.mundane_coffin"),
        category: CraftCategory::Misc,
        display_name: "凡物棺材".into(),
        materials: vec![("ling_mu_ban".into(), 6), ("ling_mu_gun".into(), 2)],
        qi_cost: 0.0,
        time_ticks: 90 * TICKS_PER_SECOND,
        output: (MUNDANE_COFFIN_ITEM_ID.into(), 1),
        requirements: CraftRequirements::default(),
        unlock_sources: vec![UnlockSource::Scroll {
            item_template: "scroll_mundane_coffin".into(),
        }],
        station: None,
    })
}

/// 按档级返回棺内寿元倍率（Some(grade) = 在棺内；None = 不在棺内）
pub fn coffin_lifespan_multiplier(grade: Option<CoffinGrade>) -> f64 {
    match grade {
        Some(g) => g.lifespan_factor(),
        None => 1.0,
    }
}

/// 某档延寿棺对应的 craft 配方 id（与 `register_craft_recipes` 命名空间一致）。
/// 仅 mundane 在 P2 有配方；jade/stone/bronze 配方是 P4，查不到时 reclaim graceful 返空。
fn coffin_recipe_id(grade: CoffinGrade) -> RecipeId {
    RecipeId::new(format!("coffin.{}", grade.item_id()))
}

/// plan-coffin-tiers-v1 P2 §8.1 #3 — 计算延寿棺破坏 / 回收的材料返还清单（纯函数）。
///
/// 按 grade 查 craft 配方 → `craft::recipe_reclaim_drops`。**配方查不到（jade/stone/
/// bronze 的灵材配方是 P4）→ 返回空 vec，graceful 不 panic**。返回的每项 count >= 1
/// （Break 模式 0 项已过滤），调用方直接发还 inventory。
fn compute_coffin_reclaim_drops(
    craft_registry: &CraftRegistry,
    grade: CoffinGrade,
    mode: crate::craft::ReclaimMode,
    seed: u64,
) -> Vec<(String, u32)> {
    match craft_registry.get(&coffin_recipe_id(grade)) {
        Some(recipe) => crate::craft::recipe_reclaim_drops(recipe, mode, seed),
        None => Vec::new(),
    }
}

/// reclaim 随机种子：按棺位 + tick 派生，对齐仓内 loot roll 的 `DefaultHasher` 熵源约定
/// （避免引入 `rand` 依赖），让破坏返还按 tick 抖动。
/// 注：`DefaultHasher` 仅保证单次程序运行内确定性（不跨 Rust 版本/平台稳定）——
/// 单元测试同进程内复现成立；生产侧 per-break 抖动不依赖跨运行确定性。
fn coffin_reclaim_seed(lower: BlockPos, tick: u64) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    (lower.x, lower.y, lower.z, tick).hash(&mut hasher);
    hasher.finish()
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn handle_coffin_place_requests(
    mut events: EventReader<CoffinPlaceRequest>,
    mut commands: Commands,
    mut registry: ResMut<CoffinRegistry>,
    item_registry: Res<ItemRegistry>,
    mut allocator: ResMut<InventoryInstanceIdAllocator>,
    layers: Query<&ChunkLayer, With<crate::world::dimension::OverworldLayer>>,
    overworld_layer: Query<Entity, With<crate::world::dimension::OverworldLayer>>,
    mut players: Query<
        (
            &Username,
            &mut Client,
            &PlayerState,
            Option<&Cultivation>,
            &mut PlayerInventory,
            &Position,
        ),
        With<Client>,
    >,
) {
    for event in events.read() {
        let Ok((username, mut client, player_state, cultivation, mut inventory, position)) =
            players.get_mut(event.player)
        else {
            tracing::warn!(
                "[bong][coffin] place rejected: player {:?} has no inventory/client state",
                event.player
            );
            continue;
        };

        if !coffin_target_is_close(position, event.pos) {
            tracing::warn!(
                "[bong][coffin] place rejected for `{}`: target {:?} too far",
                username.0,
                event.pos
            );
            continue;
        }

        let Some(instance) = inventory_item_by_instance(&inventory, event.item_instance_id) else {
            tracing::warn!(
                "[bong][coffin] place rejected for `{}`: missing item instance {}",
                username.0,
                event.item_instance_id
            );
            continue;
        };
        let Some(grade) = CoffinGrade::from_item_id(&instance.template_id) else {
            tracing::warn!(
                "[bong][coffin] place rejected for `{}`: item `{}` is not a coffin",
                username.0,
                instance.template_id
            );
            continue;
        };

        let upper = coffin_upper_half(event.pos);
        if registry.lookup(event.pos).is_some() || registry.lookup(upper).is_some() {
            tracing::warn!(
                "[bong][coffin] place rejected for `{}`: target {:?}/{:?} already registered",
                username.0,
                event.pos,
                upper
            );
            continue;
        }
        if let Ok(layer) = layers.get_single() {
            if !block_is_air(layer, event.pos) || !block_is_air(layer, upper) {
                tracing::warn!(
                    "[bong][coffin] place rejected for `{}`: target {:?}/{:?} not empty",
                    username.0,
                    event.pos,
                    upper
                );
                continue;
            }
        }

        if let Err(error) = consume_item_instance_once(&mut inventory, event.item_instance_id) {
            tracing::warn!(
                "[bong][coffin] place rejected for `{}`: consume failed: {error}",
                username.0
            );
            continue;
        }
        if !registry.insert(event.pos, event.tick, grade) {
            if let Err(error) = add_item_to_player_inventory(
                &mut inventory,
                &item_registry,
                &mut allocator,
                grade.item_id(),
                1,
                0,
            ) {
                tracing::warn!(
                    "[bong][coffin] failed to refund rejected coffin placement for `{}`: {error}",
                    username.0
                );
            }
            continue;
        }

        // plan-coffin-tiers-v1 P2 §8.1 #4 — 退役双 BlockState::CHEST 占位，四档统一走
        // GeckoLib marker 实体（按 grade 选渲染壳）。marker id 记入 registry 供 despawn /
        // recovery 复用。layer 缺失（极早期 / 测试无 OverworldLayer）则跳过 spawn，registry
        // 仍登记，recovery 系统后续补 marker。
        if let Ok(layer_entity) = overworld_layer.get_single() {
            let marker = crate::world::entity_model::spawn_visual_marker(
                &mut commands,
                layer_entity,
                None,
                grade.visual_kind(),
                coffin_marker_position(event.pos),
                COFFIN_MARKER_VISUAL_STATE,
            );
            // charge 6 防御性检查：set_marker_entity 返回 false 说明 registry 条目意外不存在
            // （理论上不应发生：insert 刚成功，但 registry 可能在极端竞争下丢失），
            // 此时清掉刚 spawn 的孤儿 marker，避免世界里留永久孤儿实体。
            if !registry.set_marker_entity(event.pos, Some(marker)) {
                tracing::warn!(
                    "[bong][coffin] set_marker_entity returned false after insert for {:?}; \
                     despawning orphan marker {marker:?}",
                    event.pos
                );
                // 棺渲染 marker 是 spawn_visual_marker 的 MarkerEntityBundle 层实体，须经 Despawned
                // 标记移除（valence 先发客户端移除包），裸 .despawn() 会让 entity layer 索引悬空 panic。
                commands.entity(marker).insert(Despawned);
            }
        }

        let default_cultivation = Cultivation::default();
        send_inventory_snapshot_to_client(
            event.player,
            &mut client,
            username.0.as_str(),
            &inventory,
            player_state,
            cultivation.unwrap_or(&default_cultivation),
            "coffin_place_consumed",
        );
        tracing::info!(
            "[bong][coffin] placed {:?} coffin for `{}` at {:?}/{:?}",
            grade,
            username.0,
            event.pos,
            upper
        );
    }
}

#[allow(clippy::type_complexity)]
fn handle_coffin_enter_requests(
    mut events: EventReader<CoffinEnterRequest>,
    mut commands: Commands,
    mut registry: ResMut<CoffinRegistry>,
    mut players: Query<
        (
            &mut Position,
            Option<&mut Flags>,
            Option<&Username>,
            Option<&LifespanComponent>,
            Option<&CoffinComponent>,
        ),
        With<Client>,
    >,
    player_persistence: Option<Res<crate::player::state::PlayerStatePersistence>>,
    mut state_events: EventWriter<CoffinStateChanged>,
    mut audio_events: EventWriter<PlaySoundRecipeRequest>,
) {
    for event in events.read() {
        let Some(coffin) = registry.lookup(event.pos) else {
            tracing::warn!(
                "[bong][coffin] enter rejected: no registered coffin at {:?}",
                event.pos
            );
            continue;
        };
        if coffin.occupied_by.is_some() {
            tracing::warn!(
                "[bong][coffin] enter rejected: coffin {:?} already occupied",
                coffin.lower
            );
            continue;
        }

        let Ok((mut position, flags, username, lifespan, current_coffin)) =
            players.get_mut(event.player)
        else {
            continue;
        };
        if current_coffin.is_some() || !coffin_target_is_close(&position, event.pos) {
            continue;
        }
        if !registry.set_occupied(coffin.lower, event.player) {
            continue;
        }

        if let Some(mut flags) = flags {
            flags.set_invisible(true);
        }
        position.set(coffin_player_position(coffin.lower));
        commands.entity(event.player).insert(CoffinComponent {
            entered_at_tick: event.tick,
            coffin_lower: coffin.lower,
            grade: coffin.grade,
        });
        persist_in_coffin(
            player_persistence.as_deref(),
            username,
            lifespan,
            Some(coffin.grade),
        );
        state_events.send(CoffinStateChanged {
            player: event.player,
            grade: Some(coffin.grade),
        });
        play_coffin_audio(
            &mut audio_events,
            "coffin_enter",
            event.player,
            Some(coffin.lower),
        );
    }
}

#[allow(clippy::type_complexity)]
fn handle_coffin_leave_requests(
    mut events: EventReader<CoffinLeaveRequest>,
    mut commands: Commands,
    mut registry: ResMut<CoffinRegistry>,
    mut players: Query<
        (
            &mut Position,
            Option<&mut Flags>,
            Option<&Username>,
            Option<&LifespanComponent>,
            Option<&CoffinComponent>,
        ),
        With<Client>,
    >,
    player_persistence: Option<Res<crate::player::state::PlayerStatePersistence>>,
    mut state_events: EventWriter<CoffinStateChanged>,
    mut audio_events: EventWriter<PlaySoundRecipeRequest>,
) {
    for event in events.read() {
        let Ok((mut position, flags, username, lifespan, current_coffin)) =
            players.get_mut(event.player)
        else {
            continue;
        };
        let Some(current_coffin) = current_coffin else {
            continue;
        };
        let lower = registry
            .clear_player(event.player)
            .unwrap_or(current_coffin.coffin_lower);
        if let Some(mut flags) = flags {
            flags.set_invisible(false);
        }
        position.set(coffin_exit_position(lower));
        commands.entity(event.player).remove::<CoffinComponent>();
        persist_in_coffin(player_persistence.as_deref(), username, lifespan, None);
        state_events.send(CoffinStateChanged {
            player: event.player,
            grade: None,
        });
        play_coffin_audio(&mut audio_events, "coffin_exit", event.player, Some(lower));
    }
}

fn handle_sneak_leave_requests(
    mut sneaks: EventReader<SneakEvent>,
    players: Query<&CoffinComponent, With<Client>>,
    mut leave_tx: EventWriter<CoffinLeaveRequest>,
) {
    for event in sneaks.read() {
        if event.state != SneakState::Start || players.get(event.client).is_err() {
            continue;
        }
        leave_tx.send(CoffinLeaveRequest {
            player: event.client,
        });
    }
}

/// plan-coffin-tiers-v1 P2 — 破坏延寿棺（消费 `CoffinBreakRequest`）。
///
/// 之前消费 `DiggingEvent`（挖方块），但 P2 棺已改为 marker 实体（坐标是 AIR），
/// MC 客户端对 AIR 不发挖掘事件，故 `DiggingEvent` 永不触发。改为消费显式意图事件
/// `CoffinBreakRequest`，无挖掘态门控（该 event 本身即破坏意图）。
/// client 侧"攻击 marker 实体 → emit 该 C2S intent"接线是 **P3**。
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn handle_coffin_breaks(
    mut events: EventReader<CoffinBreakRequest>,
    mut commands: Commands,
    mut registry: ResMut<CoffinRegistry>,
    craft_registry: Option<Res<CraftRegistry>>,
    mut players: Query<
        (
            Option<&mut PlayerInventory>,
            Option<&Username>,
            &mut Client,
            Option<&PlayerState>,
            Option<&Cultivation>,
            Option<&mut Position>,
            Option<&mut Flags>,
            Option<&LifespanComponent>,
        ),
        With<Client>,
    >,
    item_registry: Option<Res<ItemRegistry>>,
    mut allocator: Option<ResMut<InventoryInstanceIdAllocator>>,
    player_persistence: Option<Res<crate::player::state::PlayerStatePersistence>>,
    mut state_events: EventWriter<CoffinStateChanged>,
    mut audio_events: EventWriter<PlaySoundRecipeRequest>,
) {
    for event in events.read() {
        // 校验玩家实体存在（断连 / 无效 entity 时静默跳过）。
        if players.get(event.player).is_err() {
            continue;
        }
        // 近距校验（与放置 / 进棺 / 回收一致），防远程破坏。
        // 玩家无 Position 则跳过校验（测试容忍；生产 ClientBundle 恒带 Position）。
        if let Ok((.., Some(position), _, _)) = players.get(event.player) {
            if !coffin_target_is_close(position, event.pos) {
                tracing::warn!(
                    "[bong][coffin] break rejected: player {:?} too far from {:?}",
                    event.player,
                    event.pos
                );
                continue;
            }
        }
        let Some(coffin) = registry.remove_by_pos(event.pos) else {
            continue;
        };

        // plan-coffin-tiers-v1 P2 §8.1 #4 — despawn 渲染 marker（替代旧 set_block AIR）。
        despawn_coffin_marker(&mut commands, coffin.marker_entity);

        if let Some(occupant) = coffin.occupied_by {
            commands.entity(occupant).remove::<CoffinComponent>();
            if let Ok((_, username, _, _, _, position, flags, lifespan)) = players.get_mut(occupant)
            {
                if let Some(mut position) = position {
                    position.set(coffin_exit_position(coffin.lower));
                }
                if let Some(mut flags) = flags {
                    flags.set_invisible(false);
                }
                persist_in_coffin(player_persistence.as_deref(), username, lifespan, None);
            }
            state_events.send(CoffinStateChanged {
                player: occupant,
                grade: None,
            });
        }

        // plan-coffin-tiers-v1 P2 §8.1 #3 — 破坏 = 随机部分返还合成材料（Break 模式），
        // 不再原样返还棺材物品。配方查不到的档（jade/stone/bronze 配方 P4）→ 空 vec graceful。
        let seed = coffin_reclaim_seed(coffin.lower, event.tick);
        let drops = craft_registry.as_deref().map_or_else(Vec::new, |reg| {
            compute_coffin_reclaim_drops(reg, coffin.grade, crate::craft::ReclaimMode::Break, seed)
        });
        if let (Some(item_registry), Some(allocator)) =
            (item_registry.as_deref(), allocator.as_deref_mut())
        {
            if let Ok((
                Some(mut inventory),
                Some(username),
                mut client,
                player_state,
                cultivation,
                ..,
            )) = players.get_mut(event.player)
            {
                let granted = grant_reclaim_drops_to_inventory(
                    &mut inventory,
                    item_registry,
                    allocator,
                    username.0.as_str(),
                    &drops,
                    Some(&mut client),
                );
                if granted {
                    if let Some(player_state) = player_state {
                        let default_cultivation = Cultivation::default();
                        send_inventory_snapshot_to_client(
                            event.player,
                            &mut client,
                            username.0.as_str(),
                            &inventory,
                            player_state,
                            cultivation.unwrap_or(&default_cultivation),
                            "coffin_break_reclaimed",
                        );
                    }
                }
            }
        }
        play_coffin_audio(
            &mut audio_events,
            "coffin_break",
            event.player,
            Some(coffin.lower),
        );
    }
}

/// plan-coffin-tiers-v1 P2 §8.1 #3 — G 菜单 [回收] 处理。
///
/// 体验 = 玩家主动回收：despawn marker + 移除 registry 记录 + **较全返还**合成材料
/// （`ReclaimMode::Reclaim`，恒 >= 破坏的随机部分返还）。占用中的棺先弹出占用者。
///
/// **intent wiring**：本系统消费 `CoffinMenuReclaimRequest` event。P2 该 event 由
/// dev / 测试驱动（client G 菜单 UI + C2S intent 是 P3）；P3 只需在 client→server
/// 链路 emit 同一 event，处理逻辑零改动。
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn handle_coffin_menu_reclaim(
    mut events: EventReader<CoffinMenuReclaimRequest>,
    mut commands: Commands,
    mut registry: ResMut<CoffinRegistry>,
    craft_registry: Option<Res<CraftRegistry>>,
    mut players: Query<
        (
            Option<&mut PlayerInventory>,
            Option<&Username>,
            &mut Client,
            Option<&PlayerState>,
            Option<&Cultivation>,
            Option<&mut Position>,
            Option<&mut Flags>,
            Option<&LifespanComponent>,
        ),
        With<Client>,
    >,
    item_registry: Option<Res<ItemRegistry>>,
    mut allocator: Option<ResMut<InventoryInstanceIdAllocator>>,
    player_persistence: Option<Res<crate::player::state::PlayerStatePersistence>>,
    mut state_events: EventWriter<CoffinStateChanged>,
    mut audio_events: EventWriter<PlaySoundRecipeRequest>,
) {
    for event in events.read() {
        // 校验玩家实体存在（断连 / 无效 entity 时静默跳过，防断连数据丢失）。
        if players.get(event.player).is_err() {
            continue;
        }
        let Some(coffin) = registry.lookup(event.pos) else {
            tracing::warn!(
                "[bong][coffin] reclaim rejected: no registered coffin at {:?}",
                event.pos
            );
            continue;
        };
        // 近距校验（与放置 / 进棺一致），防远程回收。玩家无 Position 则跳过校验（测试容忍）。
        if let Ok((.., Some(position), _, _)) = players.get(event.player) {
            if !coffin_target_is_close(position, coffin.lower) {
                tracing::warn!(
                    "[bong][coffin] reclaim rejected: player {:?} too far from {:?}",
                    event.player,
                    coffin.lower
                );
                continue;
            }
        }

        // 从 registry 移除（lower + upper 两索引）+ despawn marker。
        let Some(coffin) = registry.remove_by_pos(coffin.lower) else {
            continue;
        };
        despawn_coffin_marker(&mut commands, coffin.marker_entity);

        // 占用者弹出（与破坏一致）。
        if let Some(occupant) = coffin.occupied_by {
            commands.entity(occupant).remove::<CoffinComponent>();
            if let Ok((_, username, _, _, _, position, flags, lifespan)) = players.get_mut(occupant)
            {
                if let Some(mut position) = position {
                    position.set(coffin_exit_position(coffin.lower));
                }
                if let Some(mut flags) = flags {
                    flags.set_invisible(false);
                }
                persist_in_coffin(player_persistence.as_deref(), username, lifespan, None);
            }
            state_events.send(CoffinStateChanged {
                player: occupant,
                grade: None,
            });
        }

        // 较全返还（Reclaim 模式）。配方查不到的档（jade/stone/bronze P4）→ 空 vec graceful。
        let seed = coffin_reclaim_seed(coffin.lower, event.tick);
        let drops = craft_registry.as_deref().map_or_else(Vec::new, |reg| {
            compute_coffin_reclaim_drops(
                reg,
                coffin.grade,
                crate::craft::ReclaimMode::Reclaim,
                seed,
            )
        });
        if let (Some(item_registry), Some(allocator)) =
            (item_registry.as_deref(), allocator.as_deref_mut())
        {
            if let Ok((
                Some(mut inventory),
                Some(username),
                mut client,
                player_state,
                cultivation,
                ..,
            )) = players.get_mut(event.player)
            {
                let granted = grant_reclaim_drops_to_inventory(
                    &mut inventory,
                    item_registry,
                    allocator,
                    username.0.as_str(),
                    &drops,
                    Some(&mut client),
                );
                if granted {
                    if let Some(player_state) = player_state {
                        let default_cultivation = Cultivation::default();
                        send_inventory_snapshot_to_client(
                            event.player,
                            &mut client,
                            username.0.as_str(),
                            &inventory,
                            player_state,
                            cultivation.unwrap_or(&default_cultivation),
                            "coffin_menu_reclaimed",
                        );
                    }
                }
            }
        }
        play_coffin_audio(
            &mut audio_events,
            "coffin_reclaim",
            event.player,
            Some(coffin.lower),
        );
    }
}

/// plan-coffin-tiers-v1 P2 — recovery：为已注册但缺渲染 marker 的棺重建 marker（grade
/// 对齐）。触发场景：玩家重连 / 区块加载后，registry 里的棺（含重连恢复进棺的玩家所在棺）
/// marker_entity=None 或指向已 despawn 的实体。本系统每 tick 扫描成本低（棺数量有限），
/// 一旦补齐 marker 即把 id 写回 registry，下次跳过。
fn rebuild_missing_coffin_markers(
    mut commands: Commands,
    mut registry: ResMut<CoffinRegistry>,
    overworld_layer: Query<Entity, With<crate::world::dimension::OverworldLayer>>,
    visuals: Query<(), With<crate::world::entity_model::BongVisualEntity>>,
) {
    let Ok(layer_entity) = overworld_layer.get_single() else {
        return;
    };

    // 先收集需要重建的棺（避免在迭代 registry 时借用冲突）。
    let mut to_rebuild: Vec<(BlockPos, CoffinGrade)> = Vec::new();
    for coffin in registry.iter_unique() {
        let alive = coffin
            .marker_entity
            .is_some_and(|marker| visuals.get(marker).is_ok());
        if !alive {
            to_rebuild.push((coffin.lower, coffin.grade));
        }
    }

    for (lower, grade) in to_rebuild {
        let marker = crate::world::entity_model::spawn_visual_marker(
            &mut commands,
            layer_entity,
            None,
            grade.visual_kind(),
            coffin_marker_position(lower),
            COFFIN_MARKER_VISUAL_STATE,
        );
        registry.set_marker_entity(lower, Some(marker));
        tracing::debug!(
            "[bong][coffin] rebuilt {:?} marker for coffin at {:?} (reconnect/chunk-load recovery)",
            grade,
            lower
        );
    }
}

/// plan-coffin-tiers-v1 P2/P3 — 把 reclaim 返还清单逐项发还玩家背包。返回 true 表示
/// 至少发还了一项（用于决定是否推 inventory snapshot）。
///
/// P3 非静默修复（CodeRabbit P2 major #2）：发还失败时记录 warn 并通过聊天栏提示玩家
/// "背包已满，X 份材料未能返还"，避免静默吞材料。真·地面掉落待 P4 或 item-entity
/// 机制就位（`DroppedLootRegistry` 路径）后升级。
fn grant_reclaim_drops_to_inventory(
    inventory: &mut PlayerInventory,
    item_registry: &ItemRegistry,
    allocator: &mut InventoryInstanceIdAllocator,
    username: &str,
    drops: &[(String, u32)],
    client: Option<&mut Client>,
) -> bool {
    let mut granted_any = false;
    let mut failed_count = 0u32;
    for (template_id, count) in drops {
        match add_item_to_player_inventory(
            inventory,
            item_registry,
            allocator,
            template_id,
            *count,
            0,
        ) {
            Ok(_) => granted_any = true,
            Err(error) => {
                tracing::warn!(
                    "[bong][coffin] failed to return reclaim drop {template_id}×{count} to `{username}`: {error}"
                );
                failed_count = failed_count.saturating_add(*count);
            }
        }
    }
    if failed_count > 0 {
        let msg = format!("§e[棺] 背包已满，{failed_count} 份材料未能返还（满包时材料不补偿）");
        tracing::warn!("[bong][coffin] {username} 背包满，{failed_count} 份 reclaim 材料无法返还");
        if let Some(c) = client {
            c.send_chat_message(msg);
        }
    }
    granted_any
}

/// plan-coffin-tiers-v1 P2 — despawn 棺的渲染 marker 实体（若存在）。幂等：marker
/// 已不存在 / 从未 spawn（None）时静默跳过。
fn despawn_coffin_marker(commands: &mut Commands, marker: Option<Entity>) {
    if let Some(marker) = marker {
        // 棺渲染 marker 是 spawn_visual_marker 的 MarkerEntityBundle 层实体，须经 Despawned
        // 标记移除（valence 先发客户端移除包），裸 .despawn() 会让 entity layer 索引悬空 panic。
        commands.entity(marker).insert(Despawned);
    }
}

fn pin_coffin_players(mut players: Query<(&CoffinComponent, &mut Position), With<Client>>) {
    for (coffin, mut position) in &mut players {
        position.set(coffin_player_position(coffin.coffin_lower));
    }
}

fn emit_coffin_ambient_audio(
    clock: Option<Res<CombatClock>>,
    players: Query<(Entity, &CoffinComponent), With<Client>>,
    mut audio_events: EventWriter<PlaySoundRecipeRequest>,
) {
    let Some(clock) = clock else {
        return;
    };
    if !clock.tick.is_multiple_of(COFFIN_AMBIENT_INTERVAL_TICKS) {
        return;
    }
    for (player, coffin) in &players {
        play_coffin_audio(
            &mut audio_events,
            "coffin_ambient",
            player,
            Some(coffin.coffin_lower),
        );
    }
}

fn emit_coffin_state_payloads(
    mut events: EventReader<CoffinStateChanged>,
    mut clients: Query<&mut Client, With<Client>>,
) {
    for event in events.read() {
        let Ok(mut client) = clients.get_mut(event.player) else {
            continue;
        };
        send_coffin_state(&mut client, event.grade);
    }
}

fn emit_coffin_state_to_joined_clients(
    mut clients: Query<(&mut Client, Option<&CoffinComponent>), Added<Client>>,
) {
    for (mut client, coffin) in &mut clients {
        send_coffin_state(&mut client, coffin.map(|c| c.grade));
    }
}

fn send_coffin_state(client: &mut Client, grade: Option<CoffinGrade>) {
    let payload = ServerDataV1::new(ServerDataPayloadV1::CoffinState(CoffinStateV1 {
        in_coffin: grade.is_some(),
        lifespan_rate_multiplier: coffin_lifespan_multiplier(grade),
        coffin_grade: grade.map(CoffinGradeV1::from),
    }));
    let payload_type = payload_type_label(payload.payload_type());
    match serialize_server_data_payload(&payload) {
        Ok(bytes) => send_server_data_payload(client, bytes.as_slice()),
        Err(error) => log_payload_build_error(payload_type, &error),
    }
}

fn play_coffin_audio(
    audio_events: &mut EventWriter<PlaySoundRecipeRequest>,
    recipe_id: &str,
    player: Entity,
    pos: Option<BlockPos>,
) {
    audio_events.send(PlaySoundRecipeRequest {
        recipe_id: recipe_id.to_string(),
        instance_id: 0,
        pos: pos.map(block_pos_array),
        flag: None,
        volume_mul: 1.0,
        pitch_shift: 0.0,
        recipient: AudioRecipient::Single(player),
    });
}

pub(crate) fn persist_in_coffin(
    player_persistence: Option<&crate::player::state::PlayerStatePersistence>,
    username: Option<&Username>,
    lifespan: Option<&LifespanComponent>,
    grade: Option<CoffinGrade>,
) {
    let (Some(player_persistence), Some(username), Some(lifespan)) =
        (player_persistence, username, lifespan)
    else {
        // grade=None means we're clearing coffin state (exit path: revive / terminate / new_char).
        // If we can't persist because components are missing, SQLite keeps the stale in_coffin=true,
        // and the player will be re-pinned to a potentially nonexistent coffin on reconnect.
        // This is the same risk path as the "重启复钉" bug — warn so it doesn't go unnoticed.
        if grade.is_none() {
            tracing::warn!(
                "[bong][coffin] cannot clear SQLite in_coffin (grade=None): \
                 persistence={} username={} lifespan={} — player may re-pin to coffin on reconnect",
                player_persistence.is_some(),
                username.is_some(),
                lifespan.is_some(),
            );
        }
        return;
    };
    if let Err(error) = crate::player::state::save_player_lifespan_slice_with_coffin(
        player_persistence,
        username.0.as_str(),
        lifespan,
        grade,
    ) {
        tracing::warn!(
            "[bong][coffin] failed to persist grade={:?} for `{}`: {error}",
            grade,
            username.0
        );
    }
}

fn coffin_upper_half(lower: BlockPos) -> BlockPos {
    BlockPos::new(lower.x + 1, lower.y, lower.z)
}

fn block_is_air(layer: &ChunkLayer, pos: BlockPos) -> bool {
    layer
        .block(pos)
        .map(|block| block.state == BlockState::AIR)
        .unwrap_or(true)
}

fn coffin_target_is_close(position: &Position, target: BlockPos) -> bool {
    let target_center = DVec3::new(
        f64::from(target.x) + 0.5,
        f64::from(target.y) + 0.5,
        f64::from(target.z) + 0.5,
    );
    position.get().distance_squared(target_center) <= COFFIN_INTERACT_MAX_DISTANCE_SQ
}

/// plan-coffin-tiers-v1 P2 — marker 渲染实体的 spawn 坐标。棺横跨 lower→upper（x+1），
/// marker 取两格中心（lower.x + 1.0）让 GeckoLib 模型居中覆盖整张棺，y 贴地。
fn coffin_marker_position(lower: BlockPos) -> DVec3 {
    DVec3::new(
        f64::from(lower.x) + 1.0,
        f64::from(lower.y),
        f64::from(lower.z) + 0.5,
    )
}

fn coffin_player_position(lower: BlockPos) -> [f64; 3] {
    [
        f64::from(lower.x) + 0.5,
        f64::from(lower.y) + 0.05,
        f64::from(lower.z) + 0.5,
    ]
}

pub fn coffin_lower_from_player_position(position: [f64; 3]) -> BlockPos {
    BlockPos::new(
        position[0].floor() as i32,
        position[1].floor() as i32,
        position[2].floor() as i32,
    )
}

fn coffin_exit_position(lower: BlockPos) -> [f64; 3] {
    [
        f64::from(lower.x) - 0.5,
        f64::from(lower.y) + 0.05,
        f64::from(lower.z) + 0.5,
    ]
}

fn block_pos_array(pos: BlockPos) -> [i32; 3] {
    [pos.x, pos.y, pos.z]
}

#[cfg(test)]
mod tests {
    use super::*;

    // ───────────────────────── plan-coffin-tiers-v1 P2 ─────────────────────────

    use crate::craft::{CraftRegistry, ReclaimMode};
    use crate::world::dimension::OverworldLayer;
    use crate::world::entity_model::{
        BongVisualEntity, COFFIN_BRONZE_ENTITY_KIND, COFFIN_JADE_ENTITY_KIND,
        COFFIN_MUNDANE_ENTITY_KIND, COFFIN_STONE_ENTITY_KIND,
    };
    use valence::prelude::{App, EntityKind, Update};

    const ALL_GRADES: [CoffinGrade; 4] = [
        CoffinGrade::Mundane,
        CoffinGrade::Jade,
        CoffinGrade::Stone,
        CoffinGrade::Bronze,
    ];

    fn craft_registry_with_coffin() -> CraftRegistry {
        let mut registry = CraftRegistry::new();
        register_craft_recipes(&mut registry).expect("coffin mundane recipe should register");
        // P4: jade/stone/bronze 配方在 workbench_recipes 注册
        crate::craft::register_workbench_recipes(&mut registry)
            .expect("workbench recipes should register (includes P4 coffin tiers)");
        registry
    }

    fn expected_entity_kind(grade: CoffinGrade) -> EntityKind {
        match grade {
            CoffinGrade::Mundane => COFFIN_MUNDANE_ENTITY_KIND,
            CoffinGrade::Jade => COFFIN_JADE_ENTITY_KIND,
            CoffinGrade::Stone => COFFIN_STONE_ENTITY_KIND,
            CoffinGrade::Bronze => COFFIN_BRONZE_ENTITY_KIND,
        }
    }

    /// 构造一个跑 recovery 系统的 app + 一个打了 OverworldLayer 标记的 layer 实体。
    fn app_with_recovery() -> (App, Entity) {
        let mut app = App::new();
        app.add_systems(Update, rebuild_missing_coffin_markers);
        app.insert_resource(CoffinRegistry::default());
        // 裸 layer 实体 + OverworldLayer 标记（marker spawn 只需 layer Entity id）。
        let layer = app.world_mut().spawn(OverworldLayer).id();
        (app, layer)
    }

    #[test]
    fn registry_set_marker_entity_updates_both_indices() {
        let mut registry = CoffinRegistry::default();
        let lower = BlockPos::new(4, 64, 4);
        let upper = coffin_upper_half(lower);
        assert!(registry.insert(lower, 0, CoffinGrade::Mundane));
        // 初始无 marker。
        assert_eq!(registry.lookup(lower).unwrap().marker_entity, None);

        let marker = Entity::from_raw(99);
        assert!(registry.set_marker_entity(lower, Some(marker)));
        // lower / upper 两索引都应更新到同一 marker。
        assert_eq!(registry.lookup(lower).unwrap().marker_entity, Some(marker));
        assert_eq!(registry.lookup(upper).unwrap().marker_entity, Some(marker));
    }

    #[test]
    fn registry_set_marker_entity_on_missing_coffin_returns_false() {
        let mut registry = CoffinRegistry::default();
        assert!(
            !registry.set_marker_entity(BlockPos::new(0, 0, 0), Some(Entity::from_raw(1))),
            "对不存在的棺设 marker 应返回 false（不静默造记录）"
        );
    }

    #[test]
    fn registry_iter_unique_yields_one_record_per_coffin() {
        let mut registry = CoffinRegistry::default();
        registry.insert(BlockPos::new(0, 64, 0), 0, CoffinGrade::Mundane);
        registry.insert(BlockPos::new(10, 64, 10), 0, CoffinGrade::Jade);
        let lowers: std::collections::HashSet<BlockPos> =
            registry.iter_unique().map(|c| c.lower).collect();
        assert_eq!(
            lowers.len(),
            2,
            "两张棺占 4 条索引，iter_unique 应去重到 2 条 lower 记录，实得 {lowers:?}"
        );
    }

    #[test]
    fn coffin_recipe_id_matches_register_namespace() {
        // coffin_recipe_id(Mundane) 必须命中 register_craft_recipes 注册的真实 id。
        let registry = craft_registry_with_coffin();
        assert!(
            registry
                .get(&coffin_recipe_id(CoffinGrade::Mundane))
                .is_some(),
            "coffin_recipe_id(Mundane) 应命中 `coffin.mundane_coffin` 配方"
        );
    }

    #[test]
    fn compute_reclaim_drops_mundane_break_returns_partial_materials() {
        // Mundane 有配方 → Break 返还 ⊆ 配方原料，且数量 ∈ [0,count]。
        let registry = craft_registry_with_coffin();
        let recipe = registry
            .get(&coffin_recipe_id(CoffinGrade::Mundane))
            .expect("mundane recipe");
        for seed in 0u64..50 {
            let drops = compute_coffin_reclaim_drops(
                &registry,
                CoffinGrade::Mundane,
                ReclaimMode::Break,
                seed,
            );
            for (template, count) in &drops {
                let original = recipe
                    .materials
                    .iter()
                    .find(|(t, _)| t == template)
                    .map(|(_, c)| *c)
                    .unwrap_or_else(|| panic!("drop {template} 不在 mundane 配方原料中"));
                assert!(
                    *count >= 1 && *count <= original,
                    "Break 返还 {template}={count} 应在 [1,{original}]（已过滤 0），seed={seed}"
                );
            }
        }
    }

    #[test]
    fn compute_reclaim_drops_mundane_reclaim_returns_full_materials() {
        // Reclaim = full 返还，逐项等于 mundane 配方原料。
        let registry = craft_registry_with_coffin();
        let recipe = registry
            .get(&coffin_recipe_id(CoffinGrade::Mundane))
            .expect("mundane recipe")
            .clone();
        let drops =
            compute_coffin_reclaim_drops(&registry, CoffinGrade::Mundane, ReclaimMode::Reclaim, 1);
        assert_eq!(
            drops, recipe.materials,
            "Reclaim 应 full 返还 mundane 配方原料（含顺序）"
        );
    }

    #[test]
    fn compute_reclaim_drops_all_grades_have_recipes_after_p4() {
        // plan-coffin-tiers-v1 P4 完成后，jade/stone/bronze 均已注册配方 →
        // Reclaim 模式返还非空；Break 模式可能随机返还空，但不 panic。
        let registry = craft_registry_with_coffin();
        for grade in [CoffinGrade::Jade, CoffinGrade::Stone, CoffinGrade::Bronze] {
            // 配方应存在
            assert!(
                registry.get(&coffin_recipe_id(grade)).is_some(),
                "{grade:?} 配方应在 P4 注册，registry 中找不到"
            );
            // Reclaim（全量）返还非空
            let drops_reclaim =
                compute_coffin_reclaim_drops(&registry, grade, ReclaimMode::Reclaim, 42);
            assert!(
                !drops_reclaim.is_empty(),
                "{grade:?} Reclaim 模式应返还非空（配方已注册），实得空"
            );
            // Break（随机部分）不 panic（不校验非空，随机可能 0 项）
            let _ = compute_coffin_reclaim_drops(&registry, grade, ReclaimMode::Break, 42);
        }
    }

    #[test]
    fn reclaim_seed_is_deterministic_for_same_pos_and_tick() {
        let lower = BlockPos::new(3, 64, 7);
        assert_eq!(
            coffin_reclaim_seed(lower, 100),
            coffin_reclaim_seed(lower, 100),
            "同棺位 + 同 tick 的 reclaim seed 必须确定性一致"
        );
        assert_ne!(
            coffin_reclaim_seed(lower, 100),
            coffin_reclaim_seed(lower, 101),
            "不同 tick 应产生不同 seed（破坏返还有抖动）"
        );
    }

    #[test]
    fn recovery_rebuilds_marker_for_each_grade_with_aligned_visual_kind() {
        // 四档：注册棺（marker_entity=None）→ 跑 recovery → marker 被重建且 EntityKind
        // 与 grade 对齐（161/162/163/164）。这同时锁定 放置→marker 的 grade→视觉链。
        for grade in ALL_GRADES {
            let (mut app, _layer) = app_with_recovery();
            let lower = BlockPos::new(0, 64, 0);
            {
                let mut registry = app.world_mut().resource_mut::<CoffinRegistry>();
                registry.insert(lower, 0, grade);
                assert_eq!(registry.lookup(lower).unwrap().marker_entity, None);
            }

            app.update();

            let marker = app
                .world()
                .resource::<CoffinRegistry>()
                .lookup(lower)
                .unwrap()
                .marker_entity
                .unwrap_or_else(|| panic!("{grade:?}: recovery 应重建 marker 并回写 registry"));
            assert_eq!(
                app.world().get::<EntityKind>(marker).copied(),
                Some(expected_entity_kind(grade)),
                "{grade:?}: marker 的 EntityKind 应对齐档级 raw_id"
            );
            // BongVisualEntity.kind 也应对齐 grade.visual_kind()。
            let visual = app
                .world()
                .get::<BongVisualEntity>(marker)
                .expect("marker should carry BongVisualEntity");
            assert_eq!(
                visual.kind,
                grade.visual_kind(),
                "{grade:?}: BongVisualEntity.kind 应等于 grade.visual_kind()"
            );
        }
    }

    #[test]
    fn recovery_is_idempotent_does_not_duplicate_markers() {
        // 已有存活 marker 的棺，再跑 recovery 不应重建（marker id 不变、不新增实体）。
        let (mut app, _layer) = app_with_recovery();
        let lower = BlockPos::new(2, 64, 2);
        app.world_mut()
            .resource_mut::<CoffinRegistry>()
            .insert(lower, 0, CoffinGrade::Jade);
        app.update();
        let first = app
            .world()
            .resource::<CoffinRegistry>()
            .lookup(lower)
            .unwrap()
            .marker_entity
            .expect("first recovery should spawn marker");

        let marker_count_before = app
            .world_mut()
            .query::<&BongVisualEntity>()
            .iter(app.world())
            .count();

        app.update();

        let second = app
            .world()
            .resource::<CoffinRegistry>()
            .lookup(lower)
            .unwrap()
            .marker_entity
            .expect("marker should remain after second update");
        assert_eq!(
            first, second,
            "marker 存活时 recovery 应幂等：marker id 不变"
        );
        let marker_count_after = app
            .world_mut()
            .query::<&BongVisualEntity>()
            .iter(app.world())
            .count();
        assert_eq!(
            marker_count_before, marker_count_after,
            "幂等 recovery 不应新增 marker 实体（before={marker_count_before}, after={marker_count_after}）"
        );
    }

    #[test]
    fn recovery_respawns_marker_after_despawn() {
        // 状态转换：marker 被 despawn（模拟区块卸载 / 手动 despawn）→ 下次 recovery 重建。
        let (mut app, _layer) = app_with_recovery();
        let lower = BlockPos::new(5, 64, 5);
        app.world_mut()
            .resource_mut::<CoffinRegistry>()
            .insert(lower, 0, CoffinGrade::Stone);
        app.update();
        let first = app
            .world()
            .resource::<CoffinRegistry>()
            .lookup(lower)
            .unwrap()
            .marker_entity
            .expect("first marker");

        // despawn marker，但 registry 仍指向旧（已死）实体。
        app.world_mut().despawn(first);
        app.update();

        let second = app
            .world()
            .resource::<CoffinRegistry>()
            .lookup(lower)
            .unwrap()
            .marker_entity
            .expect("recovery should rebuild after despawn");
        assert_ne!(
            first, second,
            "despawn 后 recovery 应 spawn 新 marker（id 不同于已死的旧 marker）"
        );
        assert!(
            app.world().get::<EntityKind>(second).is_some(),
            "重建的 marker 应是有效实体"
        );
    }

    #[test]
    fn recovery_no_overworld_layer_is_noop() {
        // 错误分支：无 OverworldLayer（极早期 / 测试未建 layer）→ recovery 跳过，不 panic。
        let mut app = App::new();
        app.add_systems(Update, rebuild_missing_coffin_markers);
        app.insert_resource(CoffinRegistry::default());
        // 不 spawn OverworldLayer 实体。
        app.world_mut().resource_mut::<CoffinRegistry>().insert(
            BlockPos::new(0, 64, 0),
            0,
            CoffinGrade::Mundane,
        );

        app.update();

        assert_eq!(
            app.world()
                .resource::<CoffinRegistry>()
                .lookup(BlockPos::new(0, 64, 0))
                .unwrap()
                .marker_entity,
            None,
            "无 layer 时 recovery 应跳过，marker_entity 保持 None"
        );
    }

    #[test]
    fn despawn_coffin_marker_none_is_noop() {
        // 幂等：marker=None 时 despawn 不 panic（命令式调用守门）。
        let mut app = App::new();
        // 用一个临时系统驱动 despawn(None)，验证不 panic。
        fn drive(mut commands: Commands) {
            super::despawn_coffin_marker(&mut commands, None);
        }
        app.add_systems(Update, drive);
        app.update();
    }

    #[test]
    fn registry_registers_and_removes_both_halves() {
        let mut registry = CoffinRegistry::default();
        let lower = BlockPos::new(8, 64, 8);
        let upper = coffin_upper_half(lower);

        assert!(registry.insert(lower, 10, CoffinGrade::Mundane));
        assert_eq!(registry.lookup(lower).unwrap().lower, lower);
        assert_eq!(registry.lookup(upper).unwrap().upper, upper);

        let removed = registry.remove_by_pos(upper).expect("coffin should remove");
        assert_eq!(removed.lower, lower);
        assert!(registry.lookup(lower).is_none());
        assert!(registry.lookup(upper).is_none());
    }

    #[test]
    fn registry_tracks_occupancy_by_player() {
        let mut registry = CoffinRegistry::default();
        let lower = BlockPos::new(8, 64, 8);
        let player = Entity::from_raw(7);

        assert!(registry.insert(lower, 10, CoffinGrade::Mundane));
        assert!(registry.set_occupied(lower, player));
        assert!(!registry.set_occupied(lower, Entity::from_raw(8)));
        assert_eq!(registry.player_in_coffin.get(&player), Some(&lower));
        assert_eq!(registry.lookup(lower).unwrap().occupied_by, Some(player));

        assert_eq!(registry.clear_player(player), Some(lower));
        assert!(!registry.player_in_coffin.contains_key(&player));
        assert_eq!(registry.lookup(lower).unwrap().occupied_by, None);
    }

    #[test]
    fn registry_reclaim_replaces_stale_occupant() {
        let mut registry = CoffinRegistry::default();
        let lower = BlockPos::new(8, 64, 8);
        let stale = Entity::from_raw(7);
        let current = Entity::from_raw(8);

        assert!(registry.insert(lower, 10, CoffinGrade::Mundane));
        assert!(registry.set_occupied(lower, stale));

        registry.reclaim_occupied(lower, current, 20, CoffinGrade::Mundane);

        assert!(!registry.player_in_coffin.contains_key(&stale));
        assert_eq!(registry.player_in_coffin.get(&current), Some(&lower));
        assert_eq!(registry.lookup(lower).unwrap().occupied_by, Some(current));
    }

    #[test]
    fn coffin_lower_restores_from_pinned_player_position() {
        let lower = BlockPos::new(8, 64, 8);
        assert_eq!(
            coffin_lower_from_player_position(coffin_player_position(lower)),
            lower
        );
    }

    #[test]
    fn coffin_grade_lifespan_factors_match_plan_values() {
        // 四档寿元倍率（plan-coffin-tiers-v1 P0 设计值）
        assert!(
            (CoffinGrade::Mundane.lifespan_factor() - 0.9).abs() < f64::EPSILON,
            "Mundane factor should be 0.9, got {}",
            CoffinGrade::Mundane.lifespan_factor()
        );
        assert!(
            (CoffinGrade::Jade.lifespan_factor() - 0.7).abs() < f64::EPSILON,
            "Jade factor should be 0.7, got {}",
            CoffinGrade::Jade.lifespan_factor()
        );
        assert!(
            (CoffinGrade::Stone.lifespan_factor() - 0.5).abs() < f64::EPSILON,
            "Stone factor should be 0.5, got {}",
            CoffinGrade::Stone.lifespan_factor()
        );
        assert!(
            (CoffinGrade::Bronze.lifespan_factor() - 0.3).abs() < f64::EPSILON,
            "Bronze factor should be 0.3, got {}",
            CoffinGrade::Bronze.lifespan_factor()
        );
    }

    #[test]
    fn coffin_lifespan_multiplier_none_is_1() {
        assert!(
            (coffin_lifespan_multiplier(None) - 1.0).abs() < f64::EPSILON,
            "None grade (not in coffin) should return 1.0, got {}",
            coffin_lifespan_multiplier(None)
        );
    }

    #[test]
    fn coffin_lifespan_multiplier_some_delegates_to_grade() {
        assert!(
            (coffin_lifespan_multiplier(Some(CoffinGrade::Mundane)) - 0.9).abs() < f64::EPSILON
        );
        assert!((coffin_lifespan_multiplier(Some(CoffinGrade::Jade)) - 0.7).abs() < f64::EPSILON);
        assert!((coffin_lifespan_multiplier(Some(CoffinGrade::Stone)) - 0.5).abs() < f64::EPSILON);
        assert!((coffin_lifespan_multiplier(Some(CoffinGrade::Bronze)) - 0.3).abs() < f64::EPSILON);
    }

    #[test]
    fn coffin_grade_item_id_roundtrip() {
        for grade in [
            CoffinGrade::Mundane,
            CoffinGrade::Jade,
            CoffinGrade::Stone,
            CoffinGrade::Bronze,
        ] {
            let id = grade.item_id();
            let back = CoffinGrade::from_item_id(id);
            assert_eq!(
                back,
                Some(grade),
                "item_id({grade:?}) = {id} should round-trip via from_item_id"
            );
        }
    }

    #[test]
    fn coffin_grade_from_item_id_rejects_unknown() {
        assert_eq!(
            CoffinGrade::from_item_id("unknown_coffin"),
            None,
            "unknown item id should return None"
        );
        assert_eq!(
            CoffinGrade::from_item_id(""),
            None,
            "empty string should return None"
        );
    }

    #[test]
    fn coffin_grade_default_is_mundane() {
        assert_eq!(CoffinGrade::default(), CoffinGrade::Mundane);
    }

    /// legacy constant 保持不变，下游断言不破坏
    #[test]
    fn coffin_lifespan_factor_legacy_constant_unchanged() {
        assert_eq!(
            COFFIN_LIFESPAN_FACTOR, 0.9,
            "COFFIN_LIFESPAN_FACTOR legacy constant must remain 0.9 for backward compat"
        );
    }

    #[test]
    fn coffin_grade_registry_preserves_grade_on_insert() {
        let mut registry = CoffinRegistry::default();
        let lower = BlockPos::new(0, 64, 0);
        registry.insert(lower, 0, CoffinGrade::Jade);
        let stored = registry.lookup(lower).expect("coffin should be registered");
        assert_eq!(
            stored.grade,
            CoffinGrade::Jade,
            "registry should preserve grade=Jade after insert"
        );
    }

    #[test]
    fn coffin_grade_db_str_roundtrip() {
        for grade in [
            CoffinGrade::Mundane,
            CoffinGrade::Jade,
            CoffinGrade::Stone,
            CoffinGrade::Bronze,
        ] {
            let s = grade.as_db_str();
            let back = CoffinGrade::from_db_str(s);
            assert_eq!(
                back, grade,
                "db_str({grade:?}) = {s} should round-trip via from_db_str"
            );
        }
    }

    #[test]
    fn coffin_grade_from_db_str_unknown_defaults_to_mundane() {
        assert_eq!(CoffinGrade::from_db_str(""), CoffinGrade::Mundane);
        assert_eq!(CoffinGrade::from_db_str("invalid"), CoffinGrade::Mundane);
    }

    // ─────────────────── ECS 集成测试：破坏 / 菜单回收完整链路 ───────────────────
    //
    // 这两个测试锁定 P2 实质重写的核心链路：
    //   破坏/回收 → despawn marker 实体 + registry 移除 + inventory grant
    // 纯函数 compute_coffin_reclaim_drops 已在上面单元测试中覆盖，
    // 这里只断言可观察的 ECS 副作用（实体存活 / registry 状态 / inventory 内容）。

    use crate::inventory::{
        ContainerState, InventoryInstanceIdAllocator, InventoryRevision, ItemCategory, ItemRarity,
        ItemRegistry, ItemTemplate, PlayerInventory, DEFAULT_CAST_DURATION_MS, DEFAULT_COOLDOWN_MS,
        MAIN_PACK_CONTAINER_ID,
    };
    use std::collections::HashMap;
    use valence::protocol::packets::play::GameMessageS2c;
    use valence::testing::{MockClientHelper, ScenarioSingleClient};

    /// 构造只含 ling_mu_ban 和 ling_mu_gun 的 ItemRegistry（mundane 棺配方原料）。
    fn make_coffin_item_registry() -> ItemRegistry {
        let mut templates = HashMap::new();
        for template_id in &["ling_mu_ban", "ling_mu_gun"] {
            templates.insert(
                template_id.to_string(),
                ItemTemplate {
                    id: template_id.to_string(),
                    display_name: template_id.to_string(),
                    category: ItemCategory::Misc,
                    placeable: None,
                    max_stack_count: 64,
                    grid_w: 1,
                    grid_h: 1,
                    base_weight: 0.1,
                    rarity: ItemRarity::Common,
                    spirit_quality_initial: 1.0,
                    description: "test".to_string(),
                    effect: None,
                    cast_duration_ms: DEFAULT_CAST_DURATION_MS,
                    cooldown_ms: DEFAULT_COOLDOWN_MS,
                    weapon_spec: None,
                    forge_station_spec: None,
                    blueprint_scroll_spec: None,
                    inscription_scroll_spec: None,
                    technique_scroll_spec: None,
                    recipe_fragment_spec: None,
                    container_spec: None,
                    shelflife_profile: None,
                    shield_spec: None,
                    shelflife_track: None,
                },
            );
        }
        ItemRegistry::from_map(templates)
    }

    /// 构造一个含 main_pack（4×4 格，宽松容量）的空背包组件。
    fn empty_player_inventory() -> PlayerInventory {
        PlayerInventory {
            triggered_treasures: Vec::new(),
            revision: InventoryRevision(0),
            containers: vec![ContainerState {
                quick_access: false,
                id: MAIN_PACK_CONTAINER_ID.to_string(),
                name: "主背包".to_string(),
                rows: 4,
                cols: 4,
                items: vec![],
                owner_instance_id: None,
            }],
            equipped: HashMap::new(),
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 9999.0,
        }
    }

    /// 在 registry 里注册一格 mundane 棺，spawn 一个带真实 `BongVisualEntity` 组件的
    /// marker 实体（与生产路径 `spawn_visual_marker` 一致），并把 marker id 写回 registry。
    /// 测试断言的是"despawn 了真实 BongVisual marker"而非裸实体，覆盖 charge-1。
    /// 返回 (lower, marker_entity)。
    fn register_mundane_coffin_with_marker(
        world: &mut bevy_ecs::world::World,
        lower: BlockPos,
    ) -> (BlockPos, Entity) {
        let marker = world
            .spawn(BongVisualEntity {
                kind: CoffinGrade::Mundane.visual_kind(),
                source: None,
            })
            .id();
        world
            .resource_mut::<CoffinRegistry>()
            .insert(lower, 0, CoffinGrade::Mundane);
        world
            .resource_mut::<CoffinRegistry>()
            .set_marker_entity(lower, Some(marker));
        (lower, marker)
    }

    /// 构造含 handle_coffin_breaks 系统 + 所有必要 event / resource 的 App。
    /// 玩家实体附带 PlayerInventory + Username。
    fn app_with_break_system() -> (App, Entity) {
        let scenario = ScenarioSingleClient::new();
        let mut app = scenario.app;
        let client_entity = scenario.client;

        // 必要 events（系统读/写）。
        app.add_event::<CoffinBreakRequest>();
        app.add_event::<CoffinStateChanged>();
        app.add_event::<PlaySoundRecipeRequest>();

        // CoffinRegistry 和 craft 配方。
        app.insert_resource(CoffinRegistry::default());
        let mut craft = CraftRegistry::new();
        register_craft_recipes(&mut craft).expect("coffin recipes should register");
        app.insert_resource(craft);

        // ItemRegistry + allocator（供 grant_reclaim_drops_to_inventory）。
        app.insert_resource(make_coffin_item_registry());
        app.insert_resource(InventoryInstanceIdAllocator::default());

        // 给客户端实体追加 PlayerInventory（Username 已由 ClientBundle 携带）。
        app.world_mut()
            .entity_mut(client_entity)
            .insert(empty_player_inventory());

        // 注册被测系统（裸加，无 .after 约束）。
        app.add_systems(Update, handle_coffin_breaks);

        (app, client_entity)
    }

    /// 构造含 handle_coffin_menu_reclaim 系统 + 所有必要 event / resource 的 App。
    fn app_with_reclaim_system() -> (App, Entity) {
        let scenario = ScenarioSingleClient::new();
        let mut app = scenario.app;
        let client_entity = scenario.client;

        app.add_event::<CoffinMenuReclaimRequest>();
        app.add_event::<CoffinStateChanged>();
        app.add_event::<PlaySoundRecipeRequest>();

        app.insert_resource(CoffinRegistry::default());
        let mut craft = CraftRegistry::new();
        register_craft_recipes(&mut craft).expect("coffin recipes should register");
        app.insert_resource(craft);

        app.insert_resource(make_coffin_item_registry());
        app.insert_resource(InventoryInstanceIdAllocator::default());

        app.world_mut()
            .entity_mut(client_entity)
            .insert(empty_player_inventory());

        app.add_systems(Update, handle_coffin_menu_reclaim);

        (app, client_entity)
    }

    #[test]
    fn ecs_coffin_break_despawns_marker_and_grants_partial_materials() {
        // 端到端 ECS 链路：玩家破坏 mundane 棺 →
        //   (a) marker 实体被 despawn；
        //   (b) registry 移除该棺；
        //   (c) 玩家背包收到 Break 返还（材料种类 ⊆ 配方，每种数量 ≤ 配方量）。
        // 棺贴近玩家出生点 (0,0,0)，通过 coffin_target_is_close 近距校验。
        let (mut app, client_entity) = app_with_break_system();
        let lower = BlockPos::new(0, 0, 0);

        let (_lower, marker) = register_mundane_coffin_with_marker(app.world_mut(), lower);

        // CoffinBreakRequest 是显式破坏意图（替代旧 DiggingEvent，marker 实体无方块不可挖）。
        app.world_mut().send_event(CoffinBreakRequest {
            player: client_entity,
            pos: lower,
            tick: 0,
        });

        app.update();

        // (a) marker 实体已被 despawn（world 里取不到）。
        assert!(
            app.world().get_entity(marker).is_none(),
            "破坏后 marker 实体应被 despawn，但 world 里仍可查到 {marker:?}；\
             期望 handle_coffin_breaks 调用 despawn_coffin_marker"
        );

        // (b) registry 里该棺已移除（lower 和 upper 两索引均为 None）。
        let upper = coffin_upper_half(lower);
        assert!(
            app.world()
                .resource::<CoffinRegistry>()
                .lookup(lower)
                .is_none(),
            "破坏后 registry 应移除 lower={lower:?} 索引，但仍存在"
        );
        assert!(
            app.world()
                .resource::<CoffinRegistry>()
                .lookup(upper)
                .is_none(),
            "破坏后 registry 应移除 upper={upper:?} 索引，但仍存在"
        );

        // (c) inventory 收到 Break 返还：种类 ⊆ {ling_mu_ban, ling_mu_gun}，
        //     每种数量 ∈ [0, 配方量]（Break 随机，允许 0 或全量，不断言固定数）。
        let valid_materials: HashMap<&str, u32> = [("ling_mu_ban", 6u32), ("ling_mu_gun", 2u32)]
            .iter()
            .copied()
            .collect();
        let inventory = app
            .world()
            .get::<PlayerInventory>(client_entity)
            .expect("client entity should carry PlayerInventory after break");
        for container in &inventory.containers {
            for placed in &container.items {
                let template_id = placed.instance.template_id.as_str();
                let recipe_max = valid_materials.get(template_id).copied().unwrap_or(0);
                assert!(
                    recipe_max > 0,
                    "Break 返还了配方外材料 {template_id}；期望仅含 ling_mu_ban / ling_mu_gun"
                );
                assert!(
                    placed.instance.stack_count <= recipe_max,
                    "Break 返还 {template_id}×{} 超过配方上限 ×{recipe_max}",
                    placed.instance.stack_count
                );
            }
        }
    }

    #[test]
    fn ecs_coffin_menu_reclaim_despawns_marker_and_grants_full_materials() {
        // 端到端 ECS 链路：玩家菜单回收 mundane 棺 →
        //   (a) marker 实体被 despawn；
        //   (b) registry 移除该棺；
        //   (c) 玩家背包精确收到 Reclaim 全量（ling_mu_ban×6 + ling_mu_gun×2），
        //       锁定「较全返还」语义（与 Break 随机部分返还形成对比）。
        let (mut app, client_entity) = app_with_reclaim_system();
        // ScenarioSingleClient 玩家默认在原点 (0,0,0)；棺紧贴原点（center ≈ 0.5），
        // 距离远小于 COFFIN_INTERACT_MAX_DISTANCE_SQ(36)，通过近距校验。
        let lower = BlockPos::new(0, 0, 0);

        let (_lower, marker) = register_mundane_coffin_with_marker(app.world_mut(), lower);

        app.world_mut().send_event(CoffinMenuReclaimRequest {
            player: client_entity,
            pos: lower,
            tick: 0,
        });

        app.update();

        // (a) marker 实体已被 despawn。
        assert!(
            app.world().get_entity(marker).is_none(),
            "菜单回收后 marker 实体应被 despawn，但 world 里仍可查到 {marker:?}；\
             期望 handle_coffin_menu_reclaim 调用 despawn_coffin_marker"
        );

        // (b) registry 里该棺已移除。
        let upper = coffin_upper_half(lower);
        assert!(
            app.world()
                .resource::<CoffinRegistry>()
                .lookup(lower)
                .is_none(),
            "回收后 registry 应移除 lower={lower:?}，但仍存在"
        );
        assert!(
            app.world()
                .resource::<CoffinRegistry>()
                .lookup(upper)
                .is_none(),
            "回收后 registry 应移除 upper={upper:?}，但仍存在"
        );

        // (c) 背包精确等于 mundane 配方全量：ling_mu_ban×6 + ling_mu_gun×2。
        let mut totals: HashMap<String, u32> = HashMap::new();
        let inventory = app
            .world()
            .get::<PlayerInventory>(client_entity)
            .expect("client entity should carry PlayerInventory after reclaim");
        for container in &inventory.containers {
            for placed in &container.items {
                *totals
                    .entry(placed.instance.template_id.clone())
                    .or_insert(0) += placed.instance.stack_count;
            }
        }
        assert_eq!(
            totals.get("ling_mu_ban").copied().unwrap_or(0),
            6,
            "Reclaim 应全量返还 ling_mu_ban×6（较全返还语义）；实得 {:?}",
            totals.get("ling_mu_ban")
        );
        assert_eq!(
            totals.get("ling_mu_gun").copied().unwrap_or(0),
            2,
            "Reclaim 应全量返还 ling_mu_gun×2（较全返还语义）；实得 {:?}",
            totals.get("ling_mu_gun")
        );
        assert_eq!(
            totals.len(),
            2,
            "Reclaim 应恰好返还 2 种材料（ling_mu_ban + ling_mu_gun），实得 {totals:?}"
        );
    }

    #[test]
    fn ecs_coffin_break_rejected_when_player_too_far() {
        // 安全门控回归：玩家距棺 > COFFIN_INTERACT_MAX_DISTANCE_SQ(36) 时，
        // handle_coffin_breaks 必须拒绝——
        //   (a) registry 仍保有该棺；
        //   (b) marker 实体仍存在；
        //   (c) 背包无返还材料。
        let (mut app, client_entity) = app_with_break_system();
        let lower = BlockPos::new(0, 64, 0);

        let (_lower, marker) = register_mundane_coffin_with_marker(app.world_mut(), lower);

        // 给玩家实体注入 Position，置于远处（x=100，距棺中心约 100 格，>> sqrt(36)=6）。
        app.world_mut()
            .entity_mut(client_entity)
            .insert(Position::new([100.0, 64.0, 100.0]));

        app.world_mut().send_event(CoffinBreakRequest {
            player: client_entity,
            pos: lower,
            tick: 0,
        });

        app.update();

        // (a) registry 里该棺仍在。
        assert!(
            app.world()
                .resource::<CoffinRegistry>()
                .lookup(lower)
                .is_some(),
            "远程 break 被拒后 registry 应仍保有 lower={lower:?}，\
             期望：coffin_target_is_close 返 false → continue 跳过 remove_by_pos"
        );

        // (b) marker 实体仍存在。
        assert!(
            app.world().get_entity(marker).is_some(),
            "远程 break 被拒后 marker 实体应仍存在（{marker:?}），\
             期望：despawn_coffin_marker 未被调用"
        );

        // (c) 背包仍为空（无返还材料）。
        let inventory = app
            .world()
            .get::<PlayerInventory>(client_entity)
            .expect("client entity should have PlayerInventory");
        let total_items: usize = inventory.containers.iter().map(|c| c.items.len()).sum();
        assert_eq!(
            total_items, 0,
            "远程 break 被拒后背包应无任何返还材料，实得 {total_items} 件；\
             期望：break 被 continue 拦截，materials 路径未执行"
        );
    }

    #[test]
    fn ecs_coffin_break_without_inventory_still_removes_coffin() {
        // 边界测试：PlayerInventory=None（玩家背包缺失）时，
        // handle_coffin_breaks 仍应正常 despawn marker + 移除 registry，不 panic，
        // 仅材料静默丢弃（intentional design）。
        let (mut app, client_entity) = app_with_break_system();
        let lower = BlockPos::new(0, 0, 0);

        let (_lower, marker) = register_mundane_coffin_with_marker(app.world_mut(), lower);

        // 移除 PlayerInventory，模拟背包缺失边界。
        app.world_mut()
            .entity_mut(client_entity)
            .remove::<PlayerInventory>();

        app.world_mut().send_event(CoffinBreakRequest {
            player: client_entity,
            pos: lower,
            tick: 0,
        });

        // 断言不 panic（app.update() 不抛出 = 通过）。
        app.update();

        // (a) marker 已 despawn。
        assert!(
            app.world().get_entity(marker).is_none(),
            "无 inventory 时 break 仍应 despawn marker {marker:?}；\
             期望：despawn_coffin_marker 在 inventory 检查前执行"
        );

        // (b) registry 已移除。
        assert!(
            app.world()
                .resource::<CoffinRegistry>()
                .lookup(lower)
                .is_none(),
            "无 inventory 时 break 仍应移除 registry lower={lower:?}；\
             期望：remove_by_pos 在 inventory 检查前执行"
        );
    }

    // ─── plan-coffin-tiers-v1 P3 E 非静默修复：grant_reclaim_drops_to_inventory ────

    #[test]
    fn grant_reclaim_drops_to_full_inventory_returns_false_and_does_not_panic() {
        // 满背包时 grant_reclaim_drops_to_inventory 应返回 false（无一成功发还）且不 panic。
        // 填满背包：1×1 格 * 16 格 = 16 个 item，各占满 stack。
        let item_registry = make_coffin_item_registry();
        let mut allocator = InventoryInstanceIdAllocator::default();

        // 构造一个只有 1 格的背包，然后填满它（stack 上限 64）。
        let mut inventory = PlayerInventory {
            triggered_treasures: Vec::new(),
            revision: InventoryRevision(0),
            containers: vec![ContainerState {
                quick_access: false,
                id: MAIN_PACK_CONTAINER_ID.to_string(),
                name: "主背包".to_string(),
                rows: 1,
                cols: 1,
                items: vec![],
                owner_instance_id: None,
            }],
            equipped: HashMap::new(),
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 9999.0,
        };
        // 先填满唯一的格（塞满 ling_mu_ban×64）。
        add_item_to_player_inventory(
            &mut inventory,
            &item_registry,
            &mut allocator,
            "ling_mu_ban",
            64,
            0,
        )
        .expect("should fit into empty slot");

        // 再尝试发还更多 ling_mu_ban → 格已满，应失败。
        let drops = vec![("ling_mu_ban".to_string(), 6u32)];
        let granted = grant_reclaim_drops_to_inventory(
            &mut inventory,
            &item_registry,
            &mut allocator,
            "test_player",
            &drops,
            None, // 无 client，只测 return value + 不 panic
        );
        assert!(
            !granted,
            "背包满时 grant_reclaim_drops_to_inventory 应返回 false（无一成功发还），\
             期望：ling_mu_ban 格已满（64/64），追加返回 Err，granted_any 保持 false"
        );
    }

    #[test]
    fn grant_reclaim_drops_empty_drops_returns_false() {
        // 边界：空 drops slice → 返回 false（没有任何成功发还）。
        let item_registry = make_coffin_item_registry();
        let mut allocator = InventoryInstanceIdAllocator::default();
        let mut inventory = empty_player_inventory();
        let granted = grant_reclaim_drops_to_inventory(
            &mut inventory,
            &item_registry,
            &mut allocator,
            "test_player",
            &[],
            None,
        );
        assert!(
            !granted,
            "空 drops 时 grant_reclaim_drops_to_inventory 应返回 false（无成功项），\
             期望：循环体未执行，granted_any 保持初始 false"
        );
    }

    #[test]
    fn grant_reclaim_drops_to_empty_inventory_returns_true() {
        // happy path：空背包足够容纳 drops → 返回 true。
        let item_registry = make_coffin_item_registry();
        let mut allocator = InventoryInstanceIdAllocator::default();
        let mut inventory = empty_player_inventory();
        let drops = vec![("ling_mu_ban".to_string(), 3u32)];
        let granted = grant_reclaim_drops_to_inventory(
            &mut inventory,
            &item_registry,
            &mut allocator,
            "test_player",
            &drops,
            None,
        );
        assert!(
            granted,
            "空背包发还 ling_mu_ban×3 应成功，grant_reclaim_drops_to_inventory 应返回 true"
        );
    }

    /// app_with_reclaim_system 的变体：同时返回 MockClientHelper 以便断言 S2C chat 消息。
    fn app_with_reclaim_system_and_helper() -> (
        valence::prelude::App,
        valence::prelude::Entity,
        MockClientHelper,
    ) {
        let scenario = ScenarioSingleClient::new();
        let mut app = scenario.app;
        let client_entity = scenario.client;
        let helper = scenario.helper;

        app.add_event::<CoffinMenuReclaimRequest>();
        app.add_event::<CoffinStateChanged>();
        app.add_event::<PlaySoundRecipeRequest>();

        app.insert_resource(CoffinRegistry::default());
        let mut craft = CraftRegistry::new();
        register_craft_recipes(&mut craft).expect("coffin recipes should register");
        app.insert_resource(craft);

        app.insert_resource(make_coffin_item_registry());
        app.insert_resource(InventoryInstanceIdAllocator::default());

        app.world_mut()
            .entity_mut(client_entity)
            .insert(empty_player_inventory());

        app.add_systems(valence::prelude::Update, handle_coffin_menu_reclaim);

        (app, client_entity, helper)
    }

    // ─── CodeRabbit major B: 满包 → chat 提示非静默路径 ────────────────────────────

    #[test]
    fn ecs_coffin_reclaim_full_inventory_sends_chat_message_and_removes_coffin() {
        // 满背包回收 mundane 棺：
        //   (a) 棺仍被移除（marker despawn + registry 清除）；
        //   (b) 客户端收到 chat 提示（§e[棺] 背包已满…）——锁定"非静默丢失"契约；
        //   (c) 不 panic。
        let lower = BlockPos::new(0, 0, 0);
        let (mut app, client_entity, mut helper) = app_with_reclaim_system_and_helper();

        let (_lower, marker) = register_mundane_coffin_with_marker(app.world_mut(), lower);

        // 把背包填满：用一个只有 1 格的容器并塞满它（ling_mu_ban×64 占满唯一格）。
        // 然后换掉 empty_player_inventory，让 reclaim drops 无处落脚。
        let item_registry = make_coffin_item_registry();
        let mut allocator = InventoryInstanceIdAllocator::default();
        let mut full_inv = PlayerInventory {
            triggered_treasures: Vec::new(),
            revision: InventoryRevision(0),
            containers: vec![ContainerState {
                quick_access: false,
                id: MAIN_PACK_CONTAINER_ID.to_string(),
                name: "主背包".to_string(),
                rows: 1,
                cols: 1,
                items: vec![],
                owner_instance_id: None,
            }],
            equipped: HashMap::new(),
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 9999.0,
        };
        crate::inventory::add_item_to_player_inventory(
            &mut full_inv,
            &item_registry,
            &mut allocator,
            "ling_mu_ban",
            64,
            0,
        )
        .expect("should fill the single slot");
        app.world_mut().entity_mut(client_entity).insert(full_inv);

        app.world_mut().send_event(CoffinMenuReclaimRequest {
            player: client_entity,
            pos: lower,
            tick: 0,
        });

        app.update();

        // (a) 棺 marker 已 despawn，不因满包中断。
        assert!(
            app.world().get_entity(marker).is_none(),
            "满包回收后 marker 实体仍应被 despawn；期望：满包只影响 grant，不阻断 despawn 路径"
        );
        // registry 也已移除。
        assert!(
            app.world()
                .resource::<CoffinRegistry>()
                .lookup(lower)
                .is_none(),
            "满包回收后 registry 仍应移除 lower={lower:?}；期望：remove_by_pos 先于 inventory 检查"
        );

        // (b) 客户端收到聊天提示。
        let messages: Vec<String> = helper
            .collect_received()
            .0
            .into_iter()
            .filter_map(|frame| {
                frame
                    .decode::<GameMessageS2c>()
                    .ok()
                    .map(|p| p.chat.to_legacy_lossy())
            })
            .collect();
        assert!(
            messages
                .iter()
                .any(|m| m.contains("[棺]") && m.contains("背包已满")),
            "满包回收时客户端应收到包含 '[棺]' 和 '背包已满' 的 chat 提示，\
             锁定非静默材料丢失契约；实际 messages={messages:?}"
        );
    }

    #[test]
    fn ecs_coffin_reclaim_without_inventory_still_removes_coffin() {
        // 边界测试：PlayerInventory=None 时，handle_coffin_menu_reclaim 仍应
        // despawn marker + 移除 registry，不 panic，材料静默丢弃。
        let (mut app, client_entity) = app_with_reclaim_system();
        let lower = BlockPos::new(0, 0, 0);

        let (_lower, marker) = register_mundane_coffin_with_marker(app.world_mut(), lower);

        app.world_mut()
            .entity_mut(client_entity)
            .remove::<PlayerInventory>();

        app.world_mut().send_event(CoffinMenuReclaimRequest {
            player: client_entity,
            pos: lower,
            tick: 0,
        });

        app.update();

        // (a) marker 已 despawn。
        assert!(
            app.world().get_entity(marker).is_none(),
            "无 inventory 时 reclaim 仍应 despawn marker {marker:?}；\
             期望：despawn_coffin_marker 在 inventory 检查前执行"
        );

        // (b) registry 已移除。
        assert!(
            app.world()
                .resource::<CoffinRegistry>()
                .lookup(lower)
                .is_none(),
            "无 inventory 时 reclaim 仍应移除 registry lower={lower:?}；\
             期望：remove_by_pos 在 inventory 检查前执行"
        );
    }

    // ─────────────────── plan-coffin-tiers-v1 P4 tests ──────────────────────

    /// P4 §配方注册：3 档新配方全部可从 registry 查到，id 对齐 coffin_recipe_id。
    #[test]
    fn p4_jade_stone_bronze_recipes_registered() {
        let registry = craft_registry_with_coffin();
        for grade in [CoffinGrade::Jade, CoffinGrade::Stone, CoffinGrade::Bronze] {
            let recipe_id = coffin_recipe_id(grade);
            assert!(
                registry.get(&recipe_id).is_some(),
                "P4 配方 `{recipe_id}` 应已注册（grade={grade:?}）"
            );
        }
    }

    /// P4 §配方字段：材料 / qi_cost / time_ticks / station 按 plan §P4 表正确。
    #[test]
    fn p4_recipe_fields_match_plan_spec() {
        let registry = craft_registry_with_coffin();

        // 寒玉棺
        let jade = registry
            .get(&coffin_recipe_id(CoffinGrade::Jade))
            .expect("jade recipe");
        assert_eq!(jade.qi_cost, 2.0, "jade qi_cost");
        assert_eq!(jade.time_ticks, 120 * TICKS_PER_SECOND, "jade time");
        assert_eq!(
            jade.station,
            Some(crate::craft::CraftStationKind::Workbench),
            "jade station"
        );
        assert!(
            jade.materials
                .iter()
                .any(|(id, cnt)| id == "yu_sui" && *cnt == 3),
            "jade 配方应含 yu_sui×3"
        );
        assert!(
            jade.materials
                .iter()
                .any(|(id, cnt)| id == "xue_po_lian" && *cnt == 2),
            "jade 配方应含 xue_po_lian×2"
        );
        assert!(
            jade.materials
                .iter()
                .any(|(id, cnt)| id == "ling_mu_ban" && *cnt == 4),
            "jade 配方应含 ling_mu_ban×4（主材，锁住别被改回不可得材料）"
        );
        assert!(
            jade.unlock_sources.iter().any(|u| matches!(
                u,
                UnlockSource::Scroll { item_template } if item_template == "scroll_jade_coffin"
            )),
            "jade 解锁源应含 Scroll(scroll_jade_coffin)"
        );

        // 玄石棺
        let stone = registry
            .get(&coffin_recipe_id(CoffinGrade::Stone))
            .expect("stone recipe");
        assert_eq!(stone.qi_cost, 4.0, "stone qi_cost");
        assert_eq!(stone.time_ticks, 150 * TICKS_PER_SECOND, "stone time");
        assert_eq!(
            stone.station,
            Some(crate::craft::CraftStationKind::Workbench),
            "stone station"
        );
        assert!(
            stone
                .materials
                .iter()
                .any(|(id, cnt)| id == "wu_yao" && *cnt == 2),
            "stone 配方应含 wu_yao×2"
        );
        assert!(
            stone
                .materials
                .iter()
                .any(|(id, cnt)| id == "xuan_iron" && *cnt == 4),
            "stone 配方应含 xuan_iron×4"
        );
        assert!(
            stone
                .materials
                .iter()
                .any(|(id, cnt)| id == "zhen_shi_chu" && *cnt == 2),
            "stone 配方应含 zhen_shi_chu×2（fix-round-1 替换 zhen_shi_zhong，锁住可得材料）"
        );
        assert!(
            stone.requirements.realm_min.is_none(),
            "stone 配方无境界门控（realm_min=None）"
        );
        assert!(
            stone.unlock_sources.iter().any(|u| matches!(
                u,
                UnlockSource::Scroll { item_template } if item_template == "scroll_stone_coffin"
            )),
            "stone 解锁源应含 Scroll(scroll_stone_coffin)"
        );
        assert!(
            stone.unlock_sources.iter().any(|u| matches!(
                u,
                UnlockSource::Mentor { npc_archetype } if npc_archetype == "array_scribe"
            )),
            "stone 解锁源应含 Mentor(array_scribe)"
        );

        // 青铜棺
        let bronze = registry
            .get(&coffin_recipe_id(CoffinGrade::Bronze))
            .expect("bronze recipe");
        assert_eq!(bronze.qi_cost, 6.0, "bronze qi_cost");
        assert_eq!(bronze.time_ticks, 180 * TICKS_PER_SECOND, "bronze time");
        assert_eq!(
            bronze.station,
            Some(crate::craft::CraftStationKind::Workbench),
            "bronze station"
        );
        assert!(
            bronze
                .materials
                .iter()
                .any(|(id, cnt)| id == "gu_tong_pian" && *cnt == 4),
            "bronze 配方应含 gu_tong_pian×4"
        );
        assert!(
            bronze
                .materials
                .iter()
                .any(|(id, cnt)| id == "xuan_iron" && *cnt == 3),
            "bronze 配方应含 xuan_iron×3"
        );
        assert!(
            bronze
                .materials
                .iter()
                .any(|(id, cnt)| id == "ling_mu_ban" && *cnt == 3),
            "bronze 配方应含 ling_mu_ban×3（fix-round-1 替换 ling_mu_jing，锁住可得材料）"
        );
        assert!(
            bronze
                .materials
                .iter()
                .any(|(id, cnt)| id == "zhen_shi_chu" && *cnt == 2),
            "bronze 配方应含 zhen_shi_chu×2（fix-round-1 替换 zhen_shi_gao，锁住可得材料）"
        );
        assert_eq!(
            bronze.requirements.realm_min,
            Some(crate::cultivation::components::Realm::Induce),
            "bronze 配方 realm_min 应为 Induce（引气境门控）"
        );
        assert!(
            bronze.unlock_sources.iter().any(|u| matches!(
                u,
                UnlockSource::Scroll { item_template } if item_template == "scroll_bronze_coffin"
            )),
            "bronze 解锁源应含 Scroll(scroll_bronze_coffin)"
        );
        assert!(
            bronze.unlock_sources.iter().any(|u| matches!(
                u,
                UnlockSource::Mentor { npc_archetype } if npc_archetype == "hermit_builder"
            )),
            "bronze 解锁源应含 Mentor(hermit_builder)（炼器流 archetype 防漂移）"
        );
    }

    /// P4 §门控单调梯度：倍率/qi_cost/time_ticks 全部严格递增（难度梯度自洽）。
    #[test]
    fn p4_grade_monotonicity() {
        // 倍率递减（越高档寿元消耗越少）
        let factors = [
            CoffinGrade::Mundane.lifespan_factor(),
            CoffinGrade::Jade.lifespan_factor(),
            CoffinGrade::Stone.lifespan_factor(),
            CoffinGrade::Bronze.lifespan_factor(),
        ];
        for i in 0..3 {
            assert!(
                factors[i] > factors[i + 1],
                "lifespan_factor 应严格递减：{:.1} > {:.1} (index {} vs {})",
                factors[i],
                factors[i + 1],
                i,
                i + 1
            );
        }

        // qi_cost 递增
        let registry = craft_registry_with_coffin();
        let qi_costs = [
            registry
                .get(&coffin_recipe_id(CoffinGrade::Mundane))
                .map(|r| r.qi_cost)
                .unwrap_or(0.0),
            registry
                .get(&coffin_recipe_id(CoffinGrade::Jade))
                .map(|r| r.qi_cost)
                .unwrap_or(0.0),
            registry
                .get(&coffin_recipe_id(CoffinGrade::Stone))
                .map(|r| r.qi_cost)
                .unwrap_or(0.0),
            registry
                .get(&coffin_recipe_id(CoffinGrade::Bronze))
                .map(|r| r.qi_cost)
                .unwrap_or(0.0),
        ];
        for i in 0..3 {
            assert!(
                qi_costs[i] < qi_costs[i + 1],
                "qi_cost 应严格递增：{} < {} (grade index {} vs {})",
                qi_costs[i],
                qi_costs[i + 1],
                i,
                i + 1
            );
        }

        // time_ticks 递增
        let times = [
            registry
                .get(&coffin_recipe_id(CoffinGrade::Mundane))
                .map(|r| r.time_ticks)
                .unwrap_or(0),
            registry
                .get(&coffin_recipe_id(CoffinGrade::Jade))
                .map(|r| r.time_ticks)
                .unwrap_or(0),
            registry
                .get(&coffin_recipe_id(CoffinGrade::Stone))
                .map(|r| r.time_ticks)
                .unwrap_or(0),
            registry
                .get(&coffin_recipe_id(CoffinGrade::Bronze))
                .map(|r| r.time_ticks)
                .unwrap_or(0),
        ];
        for i in 0..3 {
            assert!(
                times[i] < times[i + 1],
                "time_ticks 应严格递增：{} < {} (grade index {} vs {})",
                times[i],
                times[i + 1],
                i,
                i + 1
            );
        }
    }

    /// P4 §守恒：3 档配方 output spirit_quality = 0.0，守恒律 trivially 通过。
    #[test]
    fn p4_spirit_quality_conservation_trivial() {
        // coffin item spirit_quality_initial = 0.0 → output_sq = 0.0 → 守恒律不触发断言。
        // 本 test 作为"已确认不触发"的 pin：若未来 coffin 改为有灵质产出，要重新核算。
        let item_registry =
            crate::inventory::load_item_registry().expect("item registry should load");
        for grade in [CoffinGrade::Jade, CoffinGrade::Stone, CoffinGrade::Bronze] {
            let output_id = grade.item_id();
            let output_sq = item_registry
                .get(output_id)
                .map(|t| t.spirit_quality_initial)
                .unwrap_or_else(|| panic!("item `{output_id}` must be in registry"));
            assert_eq!(
                output_sq,
                0.0,
                "coffin item `{output_id}` spirit_quality_initial 预期为 0.0（棺材器物无灵质产出），\
                 若改为有灵质须重算守恒"
            );
        }
    }

    /// P4 §材料模板：yu_sui / wu_yao / gu_tong_pian 都能从 ItemRegistry 查到，
    /// spirit_quality_initial 按 plan §8.1 #2。
    #[test]
    fn p4_new_material_templates_in_registry() {
        let item_registry =
            crate::inventory::load_item_registry().expect("item registry should load");
        let checks = [("yu_sui", 0.8f64), ("wu_yao", 0.85), ("gu_tong_pian", 0.6)];
        for (id, expected_sq) in checks {
            let tmpl = item_registry
                .get(id)
                .unwrap_or_else(|| panic!("item `{id}` not found in registry（P4 新材料）"));
            assert!(
                (tmpl.spirit_quality_initial - expected_sq).abs() < 1e-9,
                "item `{id}` spirit_quality_initial: expect {expected_sq}, got {}",
                tmpl.spirit_quality_initial
            );
        }
    }

    /// P4 §卷轴模板：3 张配方卷轴在 ItemRegistry 中存在。
    #[test]
    fn p4_scroll_templates_in_registry() {
        let item_registry =
            crate::inventory::load_item_registry().expect("item registry should load");
        for scroll_id in [
            "scroll_jade_coffin",
            "scroll_stone_coffin",
            "scroll_bronze_coffin",
        ] {
            assert!(
                item_registry.get(scroll_id).is_some(),
                "scroll item `{scroll_id}` not found in registry（P4 配方卷轴）"
            );
        }
    }

    /// P4 §set_grade：CoffinRegistry.set_grade 正确更新档级（双索引）。
    #[test]
    fn p4_registry_set_grade_updates_both_indices() {
        let mut registry = CoffinRegistry::default();
        let lower = BlockPos::new(10, 64, 10);
        let upper = coffin_upper_half(lower);
        registry.insert(lower, 0, CoffinGrade::Mundane);

        let ok = registry.set_grade(lower, CoffinGrade::Bronze);
        assert!(ok, "set_grade 对已存在的棺应返回 true");
        assert_eq!(
            registry.lookup(lower).unwrap().grade,
            CoffinGrade::Bronze,
            "lower 索引档级应更新为 Bronze"
        );
        assert_eq!(
            registry.lookup(upper).unwrap().grade,
            CoffinGrade::Bronze,
            "upper 索引档级应更新为 Bronze（双索引同步）"
        );
    }

    /// P4 §set_grade：对不存在的棺返回 false。
    #[test]
    fn p4_registry_set_grade_on_missing_returns_false() {
        let mut registry = CoffinRegistry::default();
        assert!(
            !registry.set_grade(BlockPos::new(99, 64, 99), CoffinGrade::Jade),
            "对不存在的棺 set_grade 应返回 false"
        );
    }

    /// P4 §reclaim：jade/stone/bronze 均有配方后，Break/Reclaim 不再返回空列表。
    #[test]
    fn p4_reclaim_drops_non_empty_for_new_grades() {
        let registry = craft_registry_with_coffin();
        for grade in [CoffinGrade::Jade, CoffinGrade::Stone, CoffinGrade::Bronze] {
            // Reclaim 模式（全量返还）：至少有 1 项
            let drops_reclaim =
                compute_coffin_reclaim_drops(&registry, grade, ReclaimMode::Reclaim, 42);
            assert!(
                !drops_reclaim.is_empty(),
                "{grade:?} Reclaim drops should be non-empty now that recipe is registered"
            );
            // 返还物种类 ⊆ 配方原料
            let recipe = registry
                .get(&coffin_recipe_id(grade))
                .expect("recipe exists");
            let material_ids: std::collections::HashSet<&str> =
                recipe.materials.iter().map(|(id, _)| id.as_str()).collect();
            for (tid, _) in &drops_reclaim {
                assert!(
                    material_ids.contains(tid.as_str()),
                    "{grade:?} Reclaim 返还了配方外材料 `{tid}`"
                );
            }
        }
    }
}
