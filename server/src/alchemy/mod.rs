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
//!   * plan-botany-v1：正典药材名（ci_she_hao / hui_yuan_zhi / ...）直接入配方
//!   * plan-inventory-v1：pill item / 丹方残卷 item / 材料 item 需登记到 ItemRegistry
//!   * plan-cultivation-v1：pill 效果 → meridian_progress_bonus 待接
//!   * plan-HUD-v1 §10：快捷使用栏消费 pill item
//!
//! MVP 跳过（留给后续切片）：
//!   * BlockEntity 持久化（plan §1.3 离线持续性）
//!   * 客户端 Screen（plan §3.3 B 层 owo-lib UI）
//!   * Redis channel（bong:alchemy/*）+ agent schema 对齐
//!   * 品阶 / 铭文 / 开光 / AutoProfile

pub mod auto_profile;
pub mod danxin;
pub mod furnace;
pub mod learned;
pub mod outcome;
pub mod pill;
pub mod quality;
pub mod recipe;
pub mod recipe_fragment;
pub mod resolver;
pub mod session;
pub mod side_effect_apply;
pub mod skill_hook;

use std::collections::HashSet;

use valence::prelude::{
    bevy_ecs, Added, App, BlockPos, BlockState, ChunkLayer, Client, Commands, Entity, Event,
    EventReader, EventWriter, Query, Res, Update, Username, With, Without,
};

use crate::combat::components::{BodyPart, Lifecycle, LifecycleState, Wound, WoundKind, Wounds};
use crate::combat::events::{CombatEvent, DeathEvent};
use crate::combat::CombatClock;
use crate::cultivation::components::Cultivation;
use crate::cultivation::overload::MeridianOverloadEvent;
use crate::inventory::{
    consume_item_instance_once, inventory_item_by_instance, inventory_item_by_instance_borrow,
    AlchemyItemData, PlayerInventory,
};
use crate::network::inventory_snapshot_emit::send_inventory_snapshot_to_client;
use crate::player::state::{canonical_player_id, PlayerState};
use crate::skill::components::SkillId;
use crate::skill::events::{SkillXpGain, XpGainSource};
use crate::world::dimension::DimensionKind;
use crate::world::zone::ZoneRegistry;

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

pub const MIN_ZONE_QI_TO_ALCHEMY: f64 = 0.3;

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
    pub recipe_id: Option<String>,
    pub bucket: outcome::OutcomeBucket,
    pub outcome: ResolvedOutcome,
    pub elapsed_ticks: u32,
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

#[derive(Debug, Clone, Event)]
pub struct LearnRecipeFragmentIntent {
    pub player: Entity,
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
    app.add_event::<LearnRecipeFragmentIntent>();
    app.add_event::<auto_profile::InjectQiIntent>();
    app.add_event::<danxin::DanxinIdentifyIntent>();
    app.add_event::<danxin::AlchemyInsightEvent>();
    app.add_systems(
        Update,
        (
            attach_alchemy_to_joined_clients,
            handle_start_alchemy_requests,
            handle_recipe_fragment_learning,
            auto_profile::inject_qi_to_furnace_reserve,
            auto_profile::tick_auto_profiles,
            danxin::handle_danxin_identify_intents,
            handle_alchemy_furnace_place,
            emit_alchemy_skill_xp_from_outcomes,
        ),
    );
}

fn handle_recipe_fragment_learning(
    mut events: EventReader<LearnRecipeFragmentIntent>,
    recipes: Res<RecipeRegistry>,
    mut inventories: Query<&mut PlayerInventory>,
    mut learned_q: Query<&mut LearnedRecipes>,
) {
    for event in events.read() {
        let fragment = inventories.get(event.player).ok().and_then(|inventory| {
            inventory_item_by_instance_borrow(inventory, event.item_instance_id).and_then(|item| {
                match item.alchemy.as_ref() {
                    Some(AlchemyItemData::RecipeFragment { fragment }) => Some(fragment.clone()),
                    _ => None,
                }
            })
        });
        let Some(fragment) = fragment else {
            tracing::warn!(
                "[bong][alchemy] recipe fragment learn rejected: player={:?} item={}",
                event.player,
                event.item_instance_id
            );
            continue;
        };
        let (Ok(mut inventory), Ok(mut learned)) = (
            inventories.get_mut(event.player),
            learned_q.get_mut(event.player),
        ) else {
            continue;
        };

        let Some(recipe) = recipes.get(&fragment.recipe_id) else {
            tracing::warn!(
                "[bong][alchemy] recipe fragment item={} references unknown recipe `{}`",
                event.item_instance_id,
                fragment.recipe_id
            );
            continue;
        };

        let result = learned.learn_fragment(fragment, recipe);
        if matches!(
            result,
            learned::LearnResult::Learned | learned::LearnResult::FragmentMerged
        ) {
            if let Err(error) = consume_item_instance_once(&mut inventory, event.item_instance_id) {
                tracing::warn!(
                    "[bong][alchemy] recipe fragment consume failed after learn: item={} error={error}",
                    event.item_instance_id
                );
            }
        }
    }
}

fn emit_alchemy_skill_xp_from_outcomes(
    mut events: EventReader<AlchemyOutcomeEvent>,
    players: Query<(Entity, &Username), With<Client>>,
    mut skill_xp_events: EventWriter<SkillXpGain>,
) {
    for event in events.read() {
        let Some((player_entity, _)) = players.iter().find(|(_, username)| {
            event.caster_id == username.0
                || canonical_player_id(username.0.as_str()) == event.caster_id
        }) else {
            continue;
        };

        skill_xp_events.send(SkillXpGain {
            char_entity: player_entity,
            skill: SkillId::Alchemy,
            amount: skill_hook::xp_for_bucket(event.bucket),
            source: XpGainSource::Action {
                plan_id: "alchemy",
                action: alchemy_action_for_bucket(event.bucket),
            },
        });
    }
}

fn alchemy_action_for_bucket(bucket: outcome::OutcomeBucket) -> &'static str {
    match bucket {
        outcome::OutcomeBucket::Perfect => "craft_perfect",
        outcome::OutcomeBucket::Good => "craft_good",
        outcome::OutcomeBucket::Flawed => "craft_flawed",
        outcome::OutcomeBucket::Waste => "craft_waste",
        outcome::OutcomeBucket::Explode => "craft_explode",
    }
}

fn handle_start_alchemy_requests(
    mut requests: EventReader<StartAlchemyRequest>,
    recipes: Res<RecipeRegistry>,
    zones: Option<Res<ZoneRegistry>>,
    mut furnaces: Query<&mut AlchemyFurnace>,
) {
    for request in requests.read() {
        let Some(recipe) = recipes.get(&request.recipe_id) else {
            tracing::warn!(
                "[bong][alchemy] start rejected: unknown recipe `{}`",
                request.recipe_id
            );
            continue;
        };
        let zone_qi = zones
            .as_deref()
            .and_then(|zones| zones.find_zone_by_name(crate::world::zone::DEFAULT_SPAWN_ZONE_NAME))
            .or_else(|| {
                zones
                    .as_deref()
                    .and_then(|zones| zones.find_zone(DimensionKind::Overworld, Default::default()))
            })
            .map(|zone| zone.spirit_qi)
            .unwrap_or(0.0);
        if zone_qi < MIN_ZONE_QI_TO_ALCHEMY {
            tracing::warn!(
                "[bong][alchemy] start rejected: zone spirit_qi {:.3} below {:.3} for recipe `{}`",
                zone_qi,
                MIN_ZONE_QI_TO_ALCHEMY,
                request.recipe_id
            );
            continue;
        }

        let Ok(mut furnace) = furnaces.get_mut(request.furnace) else {
            tracing::warn!(
                "[bong][alchemy] start rejected: furnace {:?} missing",
                request.furnace
            );
            continue;
        };
        if !furnace.can_run(recipe.furnace_tier_min) {
            tracing::warn!(
                "[bong][alchemy] start rejected: furnace {:?} tier/integrity cannot run `{}`",
                request.furnace,
                request.recipe_id
            );
            continue;
        }
        let session = AlchemySession::new(request.recipe_id.clone(), request.caster_id.clone());
        if let Err(error) = furnace.start_session(session) {
            tracing::warn!(
                "[bong][alchemy] start rejected: furnace {:?} recipe `{}`: {error}",
                request.furnace,
                request.recipe_id
            );
        }
    }
}

pub(crate) fn apply_alchemy_explode_outcomes(
    clock: Res<CombatClock>,
    mut events: EventReader<AlchemyOutcomeEvent>,
    players: Query<(Entity, &Username), With<Client>>,
    mut wounds: Query<(&mut Wounds, Option<&Lifecycle>)>,
    mut combat_events: EventWriter<CombatEvent>,
    mut death_events: EventWriter<DeathEvent>,
    mut overload_events: EventWriter<MeridianOverloadEvent>,
) {
    for event in events.read() {
        let ResolvedOutcome::Explode {
            damage,
            meridian_crack,
        } = event.outcome
        else {
            continue;
        };

        let Some((player_entity, username)) = players.iter().find(|(_, username)| {
            event.caster_id == username.0
                || canonical_player_id(username.0.as_str()) == event.caster_id
        }) else {
            tracing::warn!(
                "[bong][alchemy] explode outcome for unknown caster `{}` from furnace {:?}",
                event.caster_id,
                event.furnace
            );
            continue;
        };

        let Ok((mut wounds, lifecycle)) = wounds.get_mut(player_entity) else {
            tracing::warn!(
                "[bong][alchemy] explode outcome caster {:?} `{}` has no Wounds",
                player_entity,
                username.0
            );
            continue;
        };

        let damage = damage.max(0.0) as f32;
        if damage <= f32::EPSILON && meridian_crack <= f64::EPSILON {
            continue;
        }

        let was_alive = wounds.health_current > 0.0;
        if damage > f32::EPSILON {
            wounds.health_current = (wounds.health_current - damage).clamp(0.0, wounds.health_max);
            wounds.entries.push(Wound {
                location: BodyPart::Chest,
                kind: WoundKind::Burn,
                severity: damage,
                bleeding_per_sec: 0.0,
                created_at_tick: clock.tick,
                inflicted_by: Some("alchemy_explode".to_string()),
            });

            combat_events.send(CombatEvent {
                attacker: event.furnace,
                target: player_entity,
                resolved_at_tick: clock.tick,
                body_part: BodyPart::Chest,
                wound_kind: WoundKind::Burn,
                damage,
                contam_delta: 0.0,
                description: format!(
                    "alchemy_explode furnace {:?} -> {} for {:.1} damage",
                    event.furnace, username.0, damage
                ),
                defense_kind: None,
                defense_effectiveness: None,
                defense_contam_reduced: None,
                defense_wound_severity: None,
            });
        }

        if meridian_crack > f64::EPSILON {
            overload_events.send(MeridianOverloadEvent {
                entity: player_entity,
                severity: meridian_crack,
            });
        }

        if was_alive
            && wounds.health_current <= 0.0
            && !lifecycle.is_some_and(|lifecycle| {
                matches!(
                    lifecycle.state,
                    LifecycleState::NearDeath | LifecycleState::Terminated
                )
            })
        {
            death_events.send(DeathEvent {
                target: player_entity,
                cause: format!("alchemy_explode:{}", event.caster_id),
                attacker: None,
                attacker_player_id: None,
                at_tick: clock.tick,
            });
        }
    }
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
    mut layers: Query<&mut ChunkLayer, With<crate::world::dimension::OverworldLayer>>,
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
                &Cultivation::default(),
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
#[allow(clippy::type_complexity)]
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
    use crate::combat::components::{WoundKind, Wounds};
    use crate::combat::events::{CombatEvent, DeathEvent};
    use crate::combat::CombatClock;
    use crate::cultivation::components::{ColorKind, Contamination, Cultivation};
    use crate::cultivation::overload::MeridianOverloadEvent;
    use crate::inventory::{
        ContainerState, InventoryRevision, ItemInstance, ItemRarity, PlacedItemState,
        PlayerInventory,
    };
    use crate::skill::events::SkillXpGain;
    use valence::prelude::{App, Events, Update};
    use valence::testing::create_mock_client;

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
                &[
                    ("hui_yuan_zhi".into(), 2, 1.0),
                    ("ling_shui".into(), 1, 1.0),
                ],
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
            mineral_id: None,
            charges: None,
            forge_quality: None,
            forge_color: None,
            forge_side_effects: Vec::new(),
            forge_achieved_tier: None,
            alchemy: None,
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
    fn alchemy_outcome_event_emits_skill_xp_gain() {
        let mut app = App::new();
        app.add_event::<AlchemyOutcomeEvent>();
        app.add_event::<SkillXpGain>();
        app.add_systems(Update, emit_alchemy_skill_xp_from_outcomes);

        let (client_bundle, _helper) = create_mock_client("Azure");
        let player = app.world_mut().spawn(client_bundle).id();
        let furnace = app.world_mut().spawn_empty().id();

        app.world_mut().send_event(AlchemyOutcomeEvent {
            furnace,
            caster_id: canonical_player_id("Azure"),
            recipe_id: Some("hui_yuan_pill_v0".to_string()),
            bucket: outcome::OutcomeBucket::Perfect,
            outcome: ResolvedOutcome::Pill {
                recipe_id: "hui_yuan_pill_v0".into(),
                pill: "hui_yuan_pill".into(),
                quality: 1.0,
                toxin_amount: 0.2,
                toxin_color: ColorKind::Mellow,
                qi_gain: Some(24.0),
                quality_tier: 5,
                effect_multiplier: 1.5,
                consecrated: false,
                side_effect: None,
                flawed_path: false,
            },
            elapsed_ticks: 120,
        });

        app.update();

        let emitted: Vec<_> = app
            .world_mut()
            .resource_mut::<Events<SkillXpGain>>()
            .drain()
            .collect();
        assert_eq!(emitted.len(), 1);
        assert_eq!(emitted[0].char_entity, player);
        assert_eq!(emitted[0].skill, SkillId::Alchemy);
        assert_eq!(emitted[0].amount, 6);
        match &emitted[0].source {
            XpGainSource::Action { plan_id, action } => {
                assert_eq!(*plan_id, "alchemy");
                assert_eq!(*action, "craft_perfect");
            }
            other => panic!("expected action source, got {other:?}"),
        }
    }

    #[test]
    fn explode_applies_damage_to_caster() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 42 });
        app.add_event::<AlchemyOutcomeEvent>();
        app.add_event::<CombatEvent>();
        app.add_event::<DeathEvent>();
        app.add_event::<MeridianOverloadEvent>();
        app.add_systems(Update, apply_alchemy_explode_outcomes);

        let (client_bundle, _helper) = create_mock_client("Azure");
        let player = app
            .world_mut()
            .spawn((
                client_bundle,
                Wounds {
                    health_current: 30.0,
                    health_max: 100.0,
                    ..Default::default()
                },
            ))
            .id();
        let furnace = app.world_mut().spawn_empty().id();

        app.world_mut().send_event(AlchemyOutcomeEvent {
            furnace,
            caster_id: canonical_player_id("Azure"),
            recipe_id: Some("kai_mai_pill_v0".to_string()),
            bucket: outcome::OutcomeBucket::Explode,
            outcome: ResolvedOutcome::Explode {
                damage: 12.0,
                meridian_crack: 0.2,
            },
            elapsed_ticks: 120,
        });
        app.update();

        let wounds = app.world().get::<Wounds>(player).unwrap();
        assert_eq!(wounds.health_current, 18.0);
        assert_eq!(wounds.entries.len(), 1);
        assert_eq!(wounds.entries[0].kind, WoundKind::Burn);

        assert_eq!(app.world().resource::<Events<CombatEvent>>().len(), 1);
        let overload = app.world().resource::<Events<MeridianOverloadEvent>>();
        let overload_event = overload.iter_current_update_events().next().unwrap();
        assert_eq!(overload_event.entity, player);
        assert!((overload_event.severity - 0.2).abs() < 1e-9);
        assert!(app.world().resource::<Events<DeathEvent>>().is_empty());
    }

    #[test]
    fn explode_emits_death_when_lethal() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 43 });
        app.add_event::<AlchemyOutcomeEvent>();
        app.add_event::<CombatEvent>();
        app.add_event::<DeathEvent>();
        app.add_event::<MeridianOverloadEvent>();
        app.add_systems(Update, apply_alchemy_explode_outcomes);

        let (client_bundle, _helper) = create_mock_client("Azure");
        let player = app
            .world_mut()
            .spawn((
                client_bundle,
                Wounds {
                    health_current: 5.0,
                    health_max: 100.0,
                    ..Default::default()
                },
            ))
            .id();
        let furnace = app.world_mut().spawn_empty().id();

        app.world_mut().send_event(AlchemyOutcomeEvent {
            furnace,
            caster_id: "Azure".to_string(),
            recipe_id: Some("kai_mai_pill_v0".to_string()),
            bucket: outcome::OutcomeBucket::Explode,
            outcome: ResolvedOutcome::Explode {
                damage: 12.0,
                meridian_crack: 0.0,
            },
            elapsed_ticks: 120,
        });
        app.update();

        assert_eq!(
            app.world().get::<Wounds>(player).unwrap().health_current,
            0.0
        );
        let deaths = app.world().resource::<Events<DeathEvent>>();
        let death = deaths.iter_current_update_events().next().unwrap();
        assert_eq!(death.target, player);
        assert!(death.cause.starts_with("alchemy_explode:"));
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
