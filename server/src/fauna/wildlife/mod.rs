//! 配置驱动的狮、鹫、马：共用修炼与战斗底盘，分别执行爆发、拉扯和群体冲锋。

pub mod brain;
pub mod config;
pub mod motion;
pub mod skills;
pub mod spawn;

#[cfg(test)]
mod tests;

use valence::prelude::{App, IntoSystemConfigs, Update};

pub fn register(app: &mut App) {
    let catalog = config::WildlifeCatalog::load();
    catalog
        .validate_skills(
            app.world()
                .resource::<crate::cultivation::known_techniques::TechniqueRegistry>(),
            app.world()
                .resource::<crate::cultivation::skill_registry::SkillRegistry>(),
            app.world().resource::<crate::body_plan::RaceRegistry>(),
        )
        .unwrap_or_else(|error| panic!("野生生物技能配置无效: {error}"));
    app.insert_resource(catalog);
    spawn::register(app);
    brain::register(app);
    app.add_systems(
        Update,
        (
            motion::move_wildlife
                .after(crate::npc::movement::apply_pending_knockback_system)
                .before(crate::npc::movement::movement_ability_tick_system)
                .before(crate::npc::navigator::navigator_tick_system),
            skills::tick_casts.after(motion::move_wildlife),
        )
            .in_set(crate::combat::CombatSystemSet::Intent),
    );
}
