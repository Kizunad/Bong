pub mod bone_coin;
pub mod butcher;
pub mod components;
// plan-daozhan-v1 P0 — 道伥核心数据结构与 spawn 触发逻辑
pub mod daozhan;
// plan-dying-elder-v1 P0 — 垂死大能核心数据结构与 spawn 触发逻辑
pub mod drop;
pub mod dying_elder;
pub mod experience;
pub mod ghost;
pub mod ghost_narration;
pub mod hybrid_beast;
pub mod migration;
pub mod mimic_spider;
pub mod rat_phase;
pub mod visual;

use valence::prelude::{App, IntoSystemConfigs};

pub fn register(app: &mut App) {
    // plan-fauna-stitched-beast-v1 P0：注册事件（系统在 P1/P2/P3 添加）
    app.add_event::<hybrid_beast::HybridBeastFormationEvent>();
    app.add_event::<hybrid_beast::CoreAbsorptionHallucinationEvent>();
    // plan-fauna-stitched-beast-v1 P1：注册融合系统 + ZoneBeastHungerTracker resource
    hybrid_beast::register_p1(app);
    // plan-fauna-stitched-beast-v1 P2：注册灵压狂暴吸收系统
    hybrid_beast::register_p2(app);
    app.add_event::<butcher::ButcherRequest>();
    app.add_event::<bone_coin::BoneCoinCraftRequest>();
    app.add_event::<bone_coin::BoneCoinCrafted>();
    app.add_event::<migration::ZoneQiCriticalEvent>();
    app.add_event::<migration::MigrationEvent>();
    app.insert_resource(migration::FaunaMigrationState::default());
    app.insert_resource(ghost::GhostZoneRegistry::default());
    app.add_event::<rat_phase::RatPhaseChangeEvent>();
    app.add_systems(
        valence::prelude::Update,
        (
            rat_phase::pressure_sensor_tick_system,
            rat_phase::emit_rat_swarm_aura_vfx_system.after(rat_phase::pressure_sensor_tick_system),
            rat_phase::apply_rat_phase_change_system,
            rat_phase::advance_transitioning_phase_system,
            rat_phase::apply_rat_phase_visual_system
                .after(rat_phase::apply_rat_phase_change_system)
                .after(rat_phase::advance_transitioning_phase_system),
            rat_phase::release_drained_qi_on_death_system.before(drop::fauna_drop_system),
            experience::emit_fauna_spawn_vfx_system,
            experience::emit_fauna_spawn_ambient_audio_system,
            experience::emit_fauna_attack_audio_system,
            experience::emit_fauna_death_vfx_audio_system.before(drop::fauna_drop_system),
            experience::emit_rat_bite_audio_system,
            migration::fauna_migration_system,
            migration::migration_trigger_system.after(migration::fauna_migration_system),
            migration::migration_move_system.after(migration::migration_trigger_system),
            migration::migration_to_beast_tide_system.after(migration::migration_move_system),
            butcher::handle_butcher_requests,
            bone_coin::handle_bone_coin_craft_requests,
            drop::fauna_drop_system,
        ),
    );
    // plan-neg-domain-fauna-v1 P0：诡影系统（单独注册，避免超过 Bevy 元组系统上限）
    app.add_systems(
        valence::prelude::Update,
        (
            ghost::ghost_spawn_system,
            ghost::ghost_drift_system.after(ghost::ghost_spawn_system),
            ghost::ghost_contact_system.after(ghost::ghost_drift_system),
        ),
    );
    // plan-neg-domain-fauna-v1 P3：Narration hints（诡影接触 + 噬灵藓踩踏，per-session 首次提示）
    app.init_resource::<ghost_narration::GhostNarrationSessionSeen>();
    app.init_resource::<ghost_narration::MossNarrationSessionSeen>();
    app.add_systems(
        valence::prelude::Update,
        (
            ghost_narration::ghost_contact_narration_system.after(ghost::ghost_contact_system),
            ghost_narration::moss_drain_narration_system,
        ),
    );
    // plan-fauna-mimic-spider-v1 P0：拟态灰烬蛛状态机 + Disguised qi 吸收 + 死亡归还
    mimic_spider::register(app);
    // plan-daozhan-v1 P1：Mimicry big-brain Scorer/Action 注册
    daozhan::register_p1(app);
    // plan-daozhan-v1 P2：Ambush big-brain Scorer/Action 注册（背对/低真元触发 + QiTransfer 守恒）
    daozhan::register_p2(app);
    // plan-daozhan-v1 P3：天道凝结 + 死亡 qi 全额归还（神识识破在 spiritual_sense/push.rs 接入）
    daozhan::register_p3(app);
    // plan-dying-elder-v1 P0：垂死大能 spawn timer + spawn 系统
    dying_elder::register_p0(app);
}
