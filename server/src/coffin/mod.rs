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
use crate::reach::DistanceRule;
use crate::schema::server_data::{CoffinGradeV1, CoffinStateV1, ServerDataPayloadV1, ServerDataV1};
use crate::world::dimension::{CurrentDimension, DimensionKind};
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
            .before(crate::network::audio_event_emit::emit_audio_play_payloads)
            // fix-spec-1901-v2 §4.2 — 进棺/出棺/破棺/回收会直接写玩家 `Position`，
            // 纳入统一移动 commit set（在灵田 post-transfer validation 之前落地）。
            .in_set(crate::world::movement_commit::AuthoritativePositionCommitSet),
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
            // fix-spec-1901-v2 §4.2 — 每 tick 把棺内玩家钉回棺位（Position 直写），
            // 同样纳入统一移动 commit set。
            pin_coffin_players
                .in_set(crate::world::movement_commit::AuthoritativePositionCommitSet),
            emit_coffin_state_payloads,
            emit_coffin_state_to_joined_clients
                .after(crate::player::attach_player_state_to_joined_clients),
        ),
    );
}

pub fn register_craft_recipes(registry: &mut CraftRegistry) -> Result<(), RegistryError> {
    // 凡木棺 ×0.9 — 手搓（station: None），Scroll 解锁
    // 寒玉/玄石/青铜棺配方由 craft 数据资产统一注册（plan-coffin-tiers-v1 P4）
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
            Option<&CurrentDimension>,
        ),
        With<Client>,
    >,
) {
    for event in events.read() {
        let Ok((
            username,
            mut client,
            player_state,
            cultivation,
            mut inventory,
            position,
            dimension,
        )) = players.get_mut(event.player)
        else {
            tracing::warn!(
                "[bong][coffin] place rejected: player {:?} has no inventory/client state",
                event.player
            );
            continue;
        };

        // plan-bughunt-coffin-dimension-gate-v1：棺注册表/marker 恒定挂在主世界，异维玩家
        // 靠裸坐标数值巧合不得操作。维度组件缺失同样 fail-closed 拒绝。
        if !coffin_requires_overworld(dimension) {
            tracing::warn!(
                "[bong][coffin] place rejected for `{}`: player not in overworld (dimension={:?})",
                username.0,
                dimension
            );
            client.send_chat_message(COFFIN_DIMENSION_REJECTION_MESSAGE);
            continue;
        }

        if !coffin_target_is_close(position, event.pos) {
            tracing::warn!(
                "[bong][coffin] place rejected for `{}`: target {:?} too far",
                username.0,
                event.pos
            );
            send_coffin_place_rejected(&mut client, &format!("目标 {:?} 太远", event.pos));
            continue;
        }

        let Some(instance) = inventory_item_by_instance(&inventory, event.item_instance_id) else {
            tracing::warn!(
                "[bong][coffin] place rejected for `{}`: missing item instance {}",
                username.0,
                event.item_instance_id
            );
            send_coffin_place_rejected(&mut client, "物品实例不存在");
            continue;
        };
        let Some(grade) = CoffinGrade::from_item_id(&instance.template_id) else {
            tracing::warn!(
                "[bong][coffin] place rejected for `{}`: item `{}` is not a coffin",
                username.0,
                instance.template_id
            );
            send_coffin_place_rejected(&mut client, "物品不是延寿棺");
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
            send_coffin_place_rejected(
                &mut client,
                &format!("位置 {:?}/{:?} 已有棺材", event.pos, upper),
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
                send_coffin_place_rejected(&mut client, "目标位置不为空");
                continue;
            }
        }

        if let Err(error) = consume_item_instance_once(&mut inventory, event.item_instance_id) {
            tracing::warn!(
                "[bong][coffin] place rejected for `{}`: consume failed: {error}",
                username.0
            );
            send_coffin_place_rejected(&mut client, "物品实例不可用");
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
            send_coffin_place_rejected(&mut client, "位置登记冲突");
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
            &mut Client,
            Option<&CurrentDimension>,
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

        let Ok((mut position, flags, username, lifespan, current_coffin, mut client, dimension)) =
            players.get_mut(event.player)
        else {
            continue;
        };

        // plan-bughunt-coffin-dimension-gate-v1：进棺目标恒在主世界，异维玩家靠裸坐标数值
        // 巧合不得钻入。维度组件缺失同样 fail-closed 拒绝。
        if !coffin_requires_overworld(dimension) {
            tracing::warn!(
                "[bong][coffin] enter rejected: player {:?} not in overworld (dimension={:?})",
                event.player,
                dimension
            );
            client.send_chat_message(COFFIN_DIMENSION_REJECTION_MESSAGE);
            continue;
        }

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
            Option<&CurrentDimension>,
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
        // plan-bughunt-coffin-dimension-gate-v1：破坏目标恒在主世界，异维玩家靠裸坐标数值
        // 巧合不得破坏。维度组件缺失同样 fail-closed 拒绝。放在近距校验之前，避免异维玩家
        // 单靠数值凑近就绕过维度隔离。
        if let Ok((.., dimension)) = players.get(event.player) {
            if !coffin_requires_overworld(dimension) {
                tracing::warn!(
                    "[bong][coffin] break rejected: player {:?} not in overworld (dimension={:?})",
                    event.player,
                    dimension
                );
                if let Ok((_, _, mut client, ..)) = players.get_mut(event.player) {
                    client.send_chat_message(COFFIN_DIMENSION_REJECTION_MESSAGE);
                }
                continue;
            }
        }
        // 近距校验（与放置 / 进棺 / 回收一致），防远程破坏。
        // 玩家无 Position 则跳过校验（测试容忍；生产 ClientBundle 恒带 Position）。
        if let Ok((.., Some(position), _, _, _)) = players.get(event.player) {
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
            if let Ok((_, username, _, _, _, position, flags, lifespan, _)) =
                players.get_mut(occupant)
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
            Option<&CurrentDimension>,
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
        // plan-bughunt-coffin-dimension-gate-v1：回收目标恒在主世界，异维玩家靠裸坐标数值
        // 巧合不得回收。维度组件缺失同样 fail-closed 拒绝。放在 registry 查找之前。
        if let Ok((.., dimension)) = players.get(event.player) {
            if !coffin_requires_overworld(dimension) {
                tracing::warn!(
                    "[bong][coffin] reclaim rejected: player {:?} not in overworld (dimension={:?})",
                    event.player,
                    dimension
                );
                if let Ok((_, _, mut client, ..)) = players.get_mut(event.player) {
                    client.send_chat_message(COFFIN_DIMENSION_REJECTION_MESSAGE);
                }
                continue;
            }
        }
        let Some(coffin) = registry.lookup(event.pos) else {
            tracing::warn!(
                "[bong][coffin] reclaim rejected: no registered coffin at {:?}",
                event.pos
            );
            continue;
        };
        // 近距校验（与放置 / 进棺一致），防远程回收。玩家无 Position 则跳过校验（测试容忍）。
        if let Ok((.., Some(position), _, _, _)) = players.get(event.player) {
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
            if let Ok((_, username, _, _, _, position, flags, lifespan, _)) =
                players.get_mut(occupant)
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
        //
        // F21 fix: when persistence + username are both available but only `LifespanComponent`
        // is missing (e.g. already removed by the time a disconnect handler runs), fall back to
        // `clear_coffin_flag_for_username` — a narrow UPDATE that clears `in_coffin` without
        // needing a `LifespanComponent`. When persistence or username themselves are missing
        // there is nothing to key the UPDATE on, so keep the old no-op+warn behaviour.
        if grade.is_none() {
            match (player_persistence, username, lifespan) {
                (Some(player_persistence), Some(username), None) => {
                    if let Err(error) = crate::player::state::clear_coffin_flag_for_username(
                        player_persistence,
                        username.0.as_str(),
                    ) {
                        tracing::warn!(
                            "[bong][coffin] F21 fallback clear_coffin_flag_for_username failed \
                             for `{}`: {error} — player may re-pin to coffin on reconnect",
                            username.0
                        );
                    } else {
                        tracing::warn!(
                            "[bong][coffin] LifespanComponent missing on coffin-exit persist for \
                             `{}`; cleared in_coffin via F21 fallback (no-lifespan narrow UPDATE) \
                             instead of leaving a stale in_coffin=true",
                            username.0
                        );
                    }
                }
                _ => {
                    tracing::warn!(
                        "[bong][coffin] cannot clear SQLite in_coffin (grade=None): \
                         persistence={} username={} lifespan={} — player may re-pin to coffin on reconnect",
                        player_persistence.is_some(),
                        username.is_some(),
                        lifespan.is_some(),
                    );
                }
            }
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

/// 检查玩家位置是否符合延寿棺交互 reach profile。
///
/// 延寿棺的四条交互链路（放置、进棺、破坏、菜单回收）都通过这个 adapter，
/// 距离 metric、边界和非有限坐标的 fail-closed 语义统一由共享
/// [`DistanceRule::NEARBY_INTERACT`] 定义。
pub fn is_coffin_target_in_range(player_pos: DVec3, target: BlockPos) -> bool {
    let target_center = DVec3::new(
        f64::from(target.x) + 0.5,
        f64::from(target.y) + 0.5,
        f64::from(target.z) + 0.5,
    );
    DistanceRule::NEARBY_INTERACT.allows(player_pos, target_center)
}

fn coffin_target_is_close(position: &Position, target: BlockPos) -> bool {
    is_coffin_target_in_range(position.get(), target)
}

/// plan-bughunt-coffin-dimension-gate-v1 — 普通延寿棺的 `CoffinRegistry` / marker 恒定挂在
/// 主世界 `OverworldLayer`；place / enter / break / menu_reclaim 四条链路都必须校验玩家
/// 当前维度是主世界，否则裸坐标数值巧合就能跨维操作主世界的棺。`CurrentDimension` 组件缺失
/// 时 fail-closed 拒绝（不得隐式当作主世界处理，对齐 `supply_coffin::authority` 的先例）。
fn coffin_requires_overworld(dimension: Option<&CurrentDimension>) -> bool {
    matches!(dimension, Some(CurrentDimension(DimensionKind::Overworld)))
}

/// 拒绝反馈复用仓库既有的 `client.send_chat_message` 回执惯例（对齐
/// `supply_coffin::interact::open_authority_rejection_message`），不另造第二套反馈机制。
const COFFIN_DIMENSION_REJECTION_MESSAGE: &str = "§c[棺] 你不在主世界，无法操作延寿棺。";

/// 放置被拒的显式 client 回执（chat 通道固定前缀）。review finding [2]：bot/验证不得用
/// /give 的快照当作放置完成屏障（`handle_give` / `emit_changed_inventory_snapshots` /
/// `handle_coffin_place_requests` 之间没有 Bevy 依赖链，give→snapshot→coffin handler 的
/// 合法顺序会让「a_id 仍在」的快照早于放置处理产出，把放置误判为被拒）。改为：接受侧由
/// `coffin_place_consumed` 消费快照回执；拒绝侧由本 chat 回执（请求特定、被拒绝即同步
/// 发出）作为确定的完成信号，bot 按请求语义判定，不依赖包序快照推导。
const COFFIN_PLACE_REJECTION_MESSAGE_PREFIX: &str = "§c[棺] 放置被拒：";

fn send_coffin_place_rejected(client: &mut Client, reason: &str) {
    client.send_chat_message(format!("{COFFIN_PLACE_REJECTION_MESSAGE_PREFIX}{reason}"));
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
#[path = "../coffin_tests.rs"]
mod tests;
