//! 炼丹系统 — plan-alchemy-v1 完整 server 切片（P0–P5）。
//!
//! 子模块：
//!   * recipe    — 配方 JSON registry + 精确/残缺匹配
//!   * furnace   — AlchemyFurnace Component
//!   * session   — AlchemySession / Intervention / tick 推进
//!   * outcome   — DeviationSummary → OutcomeBucket + side_effect_pool 抽取
//!   * resolver  — 完整结算流水线（精确 vs 残缺 vs 乱投，写 LifeRecord）
//!   * pill      — 服药 → Contamination 注入（复用 cultivation contamination_tick）
//!   * learned   — LearnedRecipes Component（卷轴学习/翻页）
//!
//! 本 plan **不含炼器** — 炼器走 plan-forge-v1。
//!
//! 跨 plan 钩子：
//!   * plan-botany-v1：§3.2 placeholder 材料（kai_mai_cao / ling_shui / ...）等 botany 落地后替换
//!   * plan-inventory-v1：pill item / 丹方残卷 item / 材料 item 需登记到 ItemRegistry
//!   * plan-cultivation-v1：pill 效果 → meridian_progress_bonus 待接
//!   * plan-HUD-v1 §10：快捷使用栏消费 pill item
//!
//! MVP 跳过（留给后续切片）：
//!   * BlockEntity 持久化（plan §1.3 离线持续性）
//!   * 客户端 Screen（plan §3.3 B 层 owo-lib UI）
//!   * Redis channel（bong:alchemy/*）+ agent schema 对齐
//!   * 品阶 / 铭文 / 开光 / AutoProfile

pub mod furnace;
pub mod learned;
pub mod outcome;
pub mod pill;
pub mod recipe;
pub mod resolver;
pub mod session;
pub mod skill_hook;

use std::collections::HashSet;

use valence::prelude::{
    bevy_ecs, Added, App, BlockPos, BlockState, ChunkLayer, Client, Commands, Entity, Event,
    EventReader, Query, Username, With, Without,
};

use crate::inventory::{consume_item_instance_once, inventory_item_by_instance, PlayerInventory};
use crate::network::inventory_snapshot_emit::send_inventory_snapshot_to_client;
use crate::player::state::PlayerState;

type JoinedClientsWithoutRecipes<'a> = (Entity, &'a Username);
type JoinedClientsWithoutRecipesFilter = (Added<Username>, With<Client>, Without<LearnedRecipes>);

#[allow(unused_imports)]
pub use furnace::{furnace_tier_from_item_id, AlchemyFurnace};
#[allow(unused_imports)]
pub use learned::LearnedRecipes;
#[allow(unused_imports)]
pub use pill::{
    can_take_pill, consume_pill, overdose_penalty, PillConsumeOutcome, PillEffect, SPOIL_TOXIN_MULT,
};
#[allow(unused_imports)]
pub use recipe::{Recipe, RecipeId, RecipeRegistry};
#[allow(unused_imports)]
pub use resolver::{record_attempt_in_life, resolve, ResolvedOutcome};
#[allow(unused_imports)]
pub use session::AlchemySession;
pub use session::Intervention;

/// plan §4 数据契约：`StartAlchemyRequest` — client → server 起炉。
#[derive(Debug, Clone, Event)]
pub struct StartAlchemyRequest {
    pub furnace: valence::prelude::Entity,
    pub recipe_id: RecipeId,
    pub caster_id: String,
}

/// plan §4 数据契约：`InterventionRequest` — client → server 调温/注 qi/中途投料。
#[derive(Debug, Clone, Event)]
pub struct InterventionRequest {
    pub furnace: valence::prelude::Entity,
    pub caster_id: String,
    pub intervention: Intervention,
}

/// plan §4 数据契约：`AlchemyOutcome` — session 结算后广播。
#[derive(Debug, Clone, Event)]
pub struct AlchemyOutcomeEvent {
    pub furnace: valence::prelude::Entity,
    pub caster_id: String,
    pub outcome: ResolvedOutcome,
}

/// plan §1.2 — 玩家手持炉类物品右键地面，客户端发 `AlchemyFurnacePlace` 后
/// server 转译为本事件，由 `handle_alchemy_furnace_place` 消费：消耗 1 个物品
/// → spawn `AlchemyFurnace` entity → 刷方块。
#[derive(Debug, Clone, Event)]
pub struct PlaceFurnaceRequest {
    pub player: Entity,
    pub pos: BlockPos,
    pub item_instance_id: u64,
}

/// 注册到主 App。
///
/// 资源加载 + 事件注册 + attach system(玩家加入时挂 AlchemyFurnace + LearnedRecipes)。
pub fn register(app: &mut App) {
    tracing::info!("[bong][alchemy] registering alchemy subsystem (plan-alchemy-v1 P0)");
    let registry = recipe::load_recipe_registry().unwrap_or_else(|error| {
        panic!("[bong][alchemy] failed to load recipe registry: {error}");
    });
    tracing::info!(
        "[bong][alchemy] loaded {} recipe(s) from assets/alchemy/recipes",
        registry.len()
    );
    app.insert_resource(registry);
    app.add_event::<StartAlchemyRequest>();
    app.add_event::<InterventionRequest>();
    app.add_event::<AlchemyOutcomeEvent>();
    app.add_event::<PlaceFurnaceRequest>();
    app.add_systems(
        valence::prelude::Update,
        (
            attach_alchemy_to_joined_clients,
            handle_alchemy_furnace_place,
        ),
    );
}

/// plan §1.2 — 消费 `PlaceFurnaceRequest`：
///   1. 拒绝在同一坐标叠放（ECS 已有炉 **或** 同 tick 另一个请求已刚放 → warn + skip）
///   2. 按 `item_instance_id` 查背包物品、按 `furnace_tier_from_item_id` 决定 tier
///   3. 消耗一个物品（`consume_item_instance_once`）
///   4. `commands.spawn(AlchemyFurnace::placed(pos, tier))`（玩家多炉并行）
///   5. 把目标方块刷成 `FURNACE`
///   6. 推一次 inventory snapshot 让 client UI 同步
///
/// 纯内存：炉状态不落盘，服务器重启 = 炉丢失（见 reminder.md）。
#[allow(clippy::too_many_arguments)]
pub fn handle_alchemy_furnace_place(
    mut events: EventReader<PlaceFurnaceRequest>,
    mut commands: Commands,
    mut inventories: Query<&mut PlayerInventory>,
    mut layers: Query<&mut ChunkLayer>,
    existing: Query<&AlchemyFurnace>,
    mut clients: Query<(&Username, &mut Client, &PlayerState)>,
) {
    // 同 tick 已放下的坐标：`commands.spawn` 要等帧末 apply，所以 `existing` 查询看不到
    // 本系统循环内上一次放的炉。若不自己记一下，两个同 pos 请求同 tick 到就会都过
    // 检查、都 spawn，产生重叠炉（Codex P1）。
    let mut placed_this_tick: HashSet<(i32, i32, i32)> = HashSet::new();

    for req in events.read() {
        let pos_key = (req.pos.x, req.pos.y, req.pos.z);
        if placed_this_tick.contains(&pos_key)
            || existing.iter().any(|f| f.block_pos() == Some(req.pos))
        {
            tracing::warn!(
                "[bong][alchemy] place_furnace rejected: pos={:?} already occupied by another furnace",
                req.pos
            );
            continue;
        }
        let Ok(mut inv) = inventories.get_mut(req.player) else {
            tracing::warn!(
                "[bong][alchemy] place_furnace rejected: player={:?} has no PlayerInventory",
                req.player
            );
            continue;
        };
        let Some(instance) = inventory_item_by_instance(&inv, req.item_instance_id) else {
            tracing::warn!(
                "[bong][alchemy] place_furnace rejected: instance_id={} not in inventory of {:?}",
                req.item_instance_id,
                req.player
            );
            continue;
        };
        let Some(tier) = furnace_tier_from_item_id(&instance.template_id) else {
            tracing::warn!(
                "[bong][alchemy] place_furnace rejected: item `{}` is not a furnace",
                instance.template_id
            );
            continue;
        };
        if let Err(err) = consume_item_instance_once(&mut inv, req.item_instance_id) {
            tracing::warn!(
                "[bong][alchemy] place_furnace rejected: consume instance_id={} failed: {err}",
                req.item_instance_id
            );
            continue;
        }
        let mut furnace = AlchemyFurnace::placed(req.pos, tier);
        let owner_name = clients.get(req.player).ok().map(|(u, _, _)| u.0.clone());
        furnace.owner = owner_name;
        commands.spawn(furnace);
        placed_this_tick.insert(pos_key);
        if let Ok(mut layer) = layers.get_single_mut() {
            layer.set_block(req.pos, BlockState::FURNACE);
        }
        // Codex P2 — 消耗物品后立即回推 snapshot，避免客户端 UI 残留旧物品导致
        // 二次误发相同 instance_id 的请求。`inv` 已 bump_revision，取最新快照即可。
        if let Ok((username, mut client, player_state)) = clients.get_mut(req.player) {
            send_inventory_snapshot_to_client(
                req.player,
                &mut client,
                username.0.as_str(),
                &inv,
                player_state,
                "alchemy_furnace_place_consumed",
            );
        }
        tracing::info!(
            "[bong][alchemy] place_furnace ok: player={:?} pos={:?} tier={} from item=`{}`",
            req.player,
            req.pos,
            tier,
            instance.template_id
        );
    }
}

/// 玩家加入时只挂 `LearnedRecipes`（plan §1.4 方子学习）。
///
/// plan §1.2：炉必须由玩家手持物品右键地面放置（`ClientRequestV1::AlchemyFurnacePlace`）。
/// 没有世界炉 = 炼不了丹 — 不再给玩家挂自带虚拟炉作保底。
pub(crate) fn attach_alchemy_to_joined_clients(
    mut commands: Commands,
    joined: Query<JoinedClientsWithoutRecipes<'_>, JoinedClientsWithoutRecipesFilter>,
) {
    for (entity, username) in &joined {
        let mut learned = LearnedRecipes::default();
        // MVP 默认开局已悟一张教学方(开脉丹)
        learned.learn("kai_mai_pill_v0".into());
        commands.entity(entity).insert(learned);
        tracing::info!("[bong][alchemy] attached LearnedRecipes to {entity:?} ({username:?})");
    }
}

#[cfg(test)]
mod integration_tests {
    //! 端到端联跑：registry 加载 → session → 服药 → contamination_tick 排异；
    //! plan §1.2 放置炉事件 → ECS spawn + inventory consume。

    use std::collections::HashMap;

    use super::*;
    use crate::cultivation::components::{ColorKind, Contamination, Cultivation};
    use crate::inventory::{
        ContainerState, InventoryRevision, ItemInstance, ItemRarity, PlacedItemState,
        PlayerInventory,
    };
    use valence::prelude::{App, Update};

    #[test]
    fn full_loop_perfect_hui_yuan_then_contamination_purge() {
        let registry = recipe::load_recipe_registry().unwrap();
        let recipe = registry.get("hui_yuan_pill_v0").unwrap().clone();

        // 1. 炉体 + session
        let mut furnace = AlchemyFurnace::new(1);
        let mut session = AlchemySession::new(recipe.id.clone(), "alice".into());
        session
            .feed_stage(
                &recipe,
                0,
                &[("bai_cao".into(), 2, 1.0), ("ling_shui".into(), 1, 1.0)],
            )
            .unwrap();
        session.apply_intervention(Intervention::AdjustTemp(0.45));
        session.apply_intervention(Intervention::InjectQi(8.0));
        for _ in 0..recipe.fire_profile.target_duration_ticks {
            session.tick();
        }
        furnace.start_session(session.clone()).unwrap();
        let ended = furnace.end_session().unwrap();

        // 2. 结算
        let outcome = resolve(&ended, &recipe, &registry);
        let pill_effect = match outcome {
            ResolvedOutcome::Pill {
                toxin_amount,
                toxin_color,
                qi_gain,
                ..
            } => PillEffect {
                toxin_amount,
                toxin_color,
                qi_gain,
                meridian_progress_bonus: None,
            },
            other => panic!("expected pill outcome, got {other:?}"),
        };

        // 3. 服药 → 污染注入
        let mut contam = Contamination::default();
        let mut cult = Cultivation {
            qi_current: 10.0,
            qi_max: 100.0,
            ..Default::default()
        };
        assert!(pill::can_take_pill(&contam, pill_effect.toxin_color));
        let outcome = consume_pill(
            &pill_effect,
            &mut contam,
            &mut cult,
            1000,
            crate::shelflife::SpoilCheckOutcome::NotApplicable,
            false,
            crate::shelflife::AgePeakCheck::NotApplicable,
        );
        assert_eq!(outcome.qi_gained, 24.0);
        assert!(!outcome.blocked);
        assert_eq!(contam.entries.len(), 1);

        // 4. 同色再吃：未到阈值仍可
        assert!(pill::can_take_pill(&contam, ColorKind::Mellow));

        // 5. 再灌一颗后爆阈值（toxin_amount ~0.2 × 6 > 1.0）
        for _ in 0..6 {
            contam
                .entries
                .push(crate::cultivation::components::ContamSource {
                    amount: 0.2,
                    color: ColorKind::Mellow,
                    attacker_id: None,
                    introduced_at: 1100,
                });
        }
        assert!(!pill::can_take_pill(&contam, ColorKind::Mellow));
    }

    #[test]
    fn register_installs_recipe_registry_resource() {
        let mut app = App::new();
        register(&mut app);
        let registry = app
            .world()
            .get_resource::<RecipeRegistry>()
            .expect("RecipeRegistry should be inserted");
        assert!(registry.len() >= 3);
    }

    // ─── plan §1.2 放置炉 ECS 流 ───────────────────────────────────

    fn item_instance(instance_id: u64, template_id: &str, stack: u32) -> ItemInstance {
        ItemInstance {
            instance_id,
            template_id: template_id.into(),
            display_name: template_id.into(),
            grid_w: 2,
            grid_h: 2,
            weight: 8.0,
            rarity: ItemRarity::Common,
            description: String::new(),
            stack_count: stack,
            spirit_quality: 1.0,
            durability: 1.0,
            freshness: None,
        }
    }

    fn inventory_with(instance: ItemInstance) -> PlayerInventory {
        let container = ContainerState {
            id: "main_pack".into(),
            name: "main_pack".into(),
            rows: 4,
            cols: 4,
            items: vec![PlacedItemState {
                row: 0,
                col: 0,
                instance,
            }],
        };
        PlayerInventory {
            revision: InventoryRevision(0),
            containers: vec![container],
            equipped: HashMap::new(),
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 45.0,
        }
    }

    fn build_place_app() -> App {
        let mut app = App::new();
        app.add_event::<PlaceFurnaceRequest>()
            .add_systems(Update, handle_alchemy_furnace_place);
        app
    }

    #[test]
    fn place_furnace_spawns_entity_and_consumes_item() {
        let mut app = build_place_app();
        let player = app
            .world_mut()
            .spawn(inventory_with(item_instance(42, "furnace_fantie", 1)))
            .id();
        let pos = valence::prelude::BlockPos::new(-12, 64, 38);

        app.world_mut().send_event(PlaceFurnaceRequest {
            player,
            pos,
            item_instance_id: 42,
        });
        app.update();

        // entity spawned with pos + tier
        let placed: Vec<_> = app
            .world_mut()
            .query::<&AlchemyFurnace>()
            .iter(app.world())
            .cloned()
            .collect();
        assert_eq!(placed.len(), 1);
        assert_eq!(placed[0].block_pos(), Some(pos));
        assert_eq!(placed[0].tier, 1);

        // item consumed (stack 1 → removed)
        let inv = app.world().get::<PlayerInventory>(player).unwrap();
        assert!(inv.containers[0].items.is_empty());
    }

    #[test]
    fn place_furnace_rejects_at_occupied_pos() {
        let mut app = build_place_app();
        let player = app
            .world_mut()
            .spawn(inventory_with(item_instance(43, "furnace_fantie", 2)))
            .id();
        let pos = valence::prelude::BlockPos::new(0, 64, 0);

        // 预先放一座
        app.world_mut().spawn(AlchemyFurnace::placed(pos, 1));

        app.world_mut().send_event(PlaceFurnaceRequest {
            player,
            pos,
            item_instance_id: 43,
        });
        app.update();

        // 仍然只有一座炉
        let count = app
            .world_mut()
            .query::<&AlchemyFurnace>()
            .iter(app.world())
            .count();
        assert_eq!(count, 1);
        // 物品没被消耗
        let inv = app.world().get::<PlayerInventory>(player).unwrap();
        assert_eq!(inv.containers[0].items[0].instance.stack_count, 2);
    }

    #[test]
    fn place_furnace_rejects_non_furnace_item() {
        let mut app = build_place_app();
        let player = app
            .world_mut()
            .spawn(inventory_with(item_instance(44, "hoe_iron", 1)))
            .id();
        let pos = valence::prelude::BlockPos::new(5, 64, 5);

        app.world_mut().send_event(PlaceFurnaceRequest {
            player,
            pos,
            item_instance_id: 44,
        });
        app.update();

        assert_eq!(
            app.world_mut()
                .query::<&AlchemyFurnace>()
                .iter(app.world())
                .count(),
            0
        );
        // 物品保留（未消耗）
        let inv = app.world().get::<PlayerInventory>(player).unwrap();
        assert_eq!(inv.containers[0].items[0].instance.stack_count, 1);
    }

    #[test]
    fn place_furnace_multi_same_player_ok() {
        let mut app = build_place_app();
        let player = app
            .world_mut()
            .spawn(inventory_with(item_instance(45, "furnace_fantie", 3)))
            .id();

        for (i, pos) in [
            valence::prelude::BlockPos::new(1, 64, 1),
            valence::prelude::BlockPos::new(2, 64, 2),
            valence::prelude::BlockPos::new(3, 64, 3),
        ]
        .into_iter()
        .enumerate()
        {
            app.world_mut().send_event(PlaceFurnaceRequest {
                player,
                pos,
                item_instance_id: 45,
            });
            app.update();
            let count = app
                .world_mut()
                .query::<&AlchemyFurnace>()
                .iter(app.world())
                .count();
            assert_eq!(count, i + 1, "after {} placements", i + 1);
        }
        // 栈已空
        let inv = app.world().get::<PlayerInventory>(player).unwrap();
        assert!(inv.containers[0].items.is_empty());
    }

    #[test]
    fn place_furnace_rejects_same_tick_duplicate_pos() {
        // Codex P1 回归：同 tick 两个 PlaceFurnaceRequest 指向同 pos，应只放下一座。
        let mut app = build_place_app();
        let player = app
            .world_mut()
            .spawn(inventory_with(item_instance(47, "furnace_fantie", 3)))
            .id();
        let pos = valence::prelude::BlockPos::new(7, 64, 7);

        // 两个事件在同一帧入队
        app.world_mut().send_event(PlaceFurnaceRequest {
            player,
            pos,
            item_instance_id: 47,
        });
        app.world_mut().send_event(PlaceFurnaceRequest {
            player,
            pos,
            item_instance_id: 47,
        });
        app.update();

        let count = app
            .world_mut()
            .query::<&AlchemyFurnace>()
            .iter(app.world())
            .count();
        assert_eq!(
            count, 1,
            "same-tick duplicate pos must only spawn one furnace"
        );
        // 只消耗一个（3 → 2）
        let inv = app.world().get::<PlayerInventory>(player).unwrap();
        assert_eq!(inv.containers[0].items[0].instance.stack_count, 2);
    }

    #[test]
    fn place_furnace_rejects_unknown_instance_id() {
        let mut app = build_place_app();
        let player = app
            .world_mut()
            .spawn(inventory_with(item_instance(46, "furnace_fantie", 1)))
            .id();

        app.world_mut().send_event(PlaceFurnaceRequest {
            player,
            pos: valence::prelude::BlockPos::new(9, 64, 9),
            item_instance_id: 9999, // 不存在
        });
        app.update();

        assert_eq!(
            app.world_mut()
                .query::<&AlchemyFurnace>()
                .iter(app.world())
                .count(),
            0
        );
        let inv = app.world().get::<PlayerInventory>(player).unwrap();
        assert_eq!(inv.containers[0].items[0].instance.stack_count, 1);
    }
}
