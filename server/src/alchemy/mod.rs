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

use valence::prelude::{
    bevy_ecs, Added, App, Client, Commands, Entity, Event, Query, Username, With, Without,
};

#[allow(unused_imports)]
pub use furnace::AlchemyFurnace;
#[allow(unused_imports)]
pub use learned::LearnedRecipes;
#[allow(unused_imports)]
pub use pill::{can_take_pill, consume_pill, overdose_penalty, PillEffect};
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
    app.add_systems(valence::prelude::Update, attach_alchemy_to_joined_clients);
}

/// 玩家加入时挂 AlchemyFurnace + LearnedRecipes。MVP 简化:每个玩家自带一个虚拟炉
/// (而非按 BlockEntity 绑炉),plan §1.2 多炉并行 / BlockEntity 持久化留 plan-persistence-v1。
pub(crate) fn attach_alchemy_to_joined_clients(
    mut commands: Commands,
    joined: Query<(Entity, &Username), (Added<Username>, With<Client>, Without<LearnedRecipes>)>,
) {
    for (entity, username) in &joined {
        let mut learned = LearnedRecipes::default();
        // MVP 默认开局已悟一张教学方(开脉丹)
        learned.learn("kai_mai_pill_v0".into());
        let mut furnace = AlchemyFurnace::new(1);
        furnace.owner = Some(username.0.clone());
        commands.entity(entity).insert((furnace, learned));
        tracing::info!(
            "[bong][alchemy] attached AlchemyFurnace + LearnedRecipes to {entity:?} ({username:?})"
        );
    }
}

#[cfg(test)]
mod integration_tests {
    //! 端到端联跑：registry 加载 → session → 服药 → contamination_tick 排异。

    use super::*;
    use crate::cultivation::components::{ColorKind, Contamination, Cultivation};

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
                &[("bai_cao".into(), 2), ("ling_shui".into(), 1)],
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
        let gained = consume_pill(&pill_effect, &mut contam, &mut cult, 1000);
        assert_eq!(gained, 24.0);
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
}
