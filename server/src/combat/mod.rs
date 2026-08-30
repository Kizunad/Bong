pub mod anqi_v2;
pub mod anticheat;
pub mod arm_wound;
pub mod armor;
pub mod armor_sync;
pub mod baomai_v3;
pub mod baomai_v4;
pub mod body_conditioning;
pub mod body_mass;
pub mod carrier;
pub mod components;
#[cfg(test)]
mod death_event_attacker_chain_test;
pub mod debug;
pub mod decay;
pub mod dugu_v2;
pub mod events;
pub mod foreign_qi_resistance;
pub mod guard_log;
pub mod jiemai;
pub mod knockback;
pub mod lifecycle;
pub mod needle;
pub mod player_attack;
pub mod projectile;
pub mod rat_bite;
pub mod raycast;
pub mod realm_gap;
pub mod resolve;
pub mod shield_block;
pub mod status;
pub mod style_telemetry;
pub mod sword_basics;
pub mod tuike;
pub mod tuike_v2;
pub mod weapon;
pub mod woliu;
pub mod woliu_v2;
pub mod yidao;
pub mod zhenmai_v2;

use std::path::Path;
use valence::prelude::{
    bevy_ecs, Added, App, Client, Commands, Entity, GameMode, IntoSystemConfigs,
    IntoSystemSetConfigs, Or, Query, SystemSet, Update, Username, Without,
};

#[cfg(test)]
mod tests;

use crate::npc::brain::canonical_npc_id;
use crate::npc::lifecycle::NpcArchetype;
use crate::npc::spawn::NpcMarker;
use crate::player::state::{
    canonical_player_id, load_current_character_id, load_player_lifecycle_slice,
    load_player_shrine_anchor_slice, player_character_id, PlayerStatePersistence,
};

use self::anticheat::{
    load_anticheat_config, AntiCheatConfig, AntiCheatCounter, AntiCheatViolationEvent,
    DEFAULT_ANTICHEAT_CONFIG_PATH,
};
use self::body_mass::{BodyMass, Stance};
use self::components::{CombatState, DerivedAttrs, Lifecycle, Stamina, StatusEffects, Wounds};
use self::events::{
    ApplyStatusEffectIntent, AttackIntent, CombatEvent, DeathCinematicPublished, DeathEvent,
    DeathInsightRequested, DebugCombatCommand, DefenseIntent, RevivalActionIntent,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemSet)]
pub enum CombatSystemSet {
    Intent,
    Physics,
    Resolve,
    Emit,
}

#[derive(Debug, Clone, Default)]
pub struct CombatClock {
    pub tick: u64,
}

impl valence::prelude::Resource for CombatClock {}

pub fn is_damageable(entity: Entity, game_modes: &Query<&GameMode>) -> bool {
    game_modes
        .get(entity)
        .map_or(true, |game_mode| *game_mode == GameMode::Survival)
}

type JoinedClientsWithoutCombatBundle<'a> = (valence::prelude::Entity, &'a Username);
type JoinedClientsWithoutCombatBundleFilter = (
    Or<(
        Added<Client>,
        Added<crate::cultivation::known_techniques::KnownTechniquesReconnectReady>,
    )>,
    Without<Wounds>,
    Without<crate::cultivation::known_techniques::KnownTechniquesReconnectBlocked>,
);

pub(crate) fn attach_combat_bundle_to_joined_clients(
    mut commands: Commands,
    joined_clients: Query<
        JoinedClientsWithoutCombatBundle<'_>,
        JoinedClientsWithoutCombatBundleFilter,
    >,
    player_persistence: Option<valence::prelude::Res<PlayerStatePersistence>>,
    combat_clock: Option<valence::prelude::Res<CombatClock>>,
) {
    // bughunt player-lifecycle-relog-death-consequence-wipe（OPUS 返工要求 1）：
    // `CombatClock` 每次进程重启都从 0 重新计数，读档时必须把这个"当前 tick"传给
    // `load_player_lifecycle_slice`，用于把落盘时刻记录的绝对 tick deadline 折算到当前
    // tick 空间（见 `player::state::translate_lifecycle_deadline_tick_across_restart`）。
    // 缺省（未注册 CombatClock 资源的最小化测试 app）时按 0 处理，与生产环境启动瞬间的
    // 真实初值一致。
    let current_combat_clock_tick = combat_clock.as_deref().map_or(0, |clock| clock.tick);
    for (entity, username) in &joined_clients {
        let persistence = player_persistence.as_deref();
        let spawn_anchor = persistence.and_then(|persistence| {
            load_player_shrine_anchor_slice(persistence, username.0.as_str())
                .ok()
                .flatten()
        });
        let character_id = persistence
            .and_then(|persistence| {
                load_current_character_id(persistence, username.0.as_str())
                    .ok()
                    .flatten()
            })
            .map(|current_char_id| player_character_id(username.0.as_str(), &current_char_id))
            .unwrap_or_else(|| canonical_player_id(username.0.as_str()));

        // bughunt player-lifecycle-relog-death-consequence-wipe：断线重连必须复用上次落盘
        // 的死亡/复活状态机（state / fortune_remaining / awaiting_decision / 各 deadline
        // tick），不能盲插 Lifecycle::default()——否则濒死 (NearDeath) / 待复活
        // (AwaitingRevival) 玩家断线重连即可白嫖满状态"新角色"，完全绕过渡劫概率判定
        // 与每角色仅 3 次的运气消耗（fortune_remaining）。deadline 均为绝对 tick 值，
        // `load_player_lifecycle_slice` 已经按 `current_combat_clock_tick` 把它们折算到
        // 当前 tick 空间（跨重启也不例外），near_death_tick /
        // auto_confirm_revival_decisions 会在下一 tick 自然按折算后的 deadline 继续结算，
        // 无需在这里重放决策逻辑。character_id 不匹配（老档 / 已转生到新角色）时视为
        // "无可复用的存档"，回退默认值。
        let persisted_lifecycle = persistence.and_then(|persistence| {
            match load_player_lifecycle_slice(
                persistence,
                username.0.as_str(),
                current_combat_clock_tick,
            ) {
                Ok(lifecycle) => lifecycle,
                Err(error) => {
                    // bughunt player-lifecycle-relog-death-consequence-wipe（OPUS 返工要求
                    // 4）：反序列化失败（坏行/损坏 JSON）不能静默吞掉——那样会悄悄回退到
                    // Lifecycle::default() 满状态，与本 bug 同一失效类（濒死/待复活状态被
                    // 无声抹除）。这里必须 warn! 留痕，再回退默认值（回退本身是唯一可行的
                    // 兜底：拒绝加入服务器同样不可接受）。
                    tracing::warn!(
                        "[bong][combat][lifecycle] failed to load persisted Lifecycle for `{}`, \
                         falling back to Lifecycle::default(): {error}",
                        username.0,
                    );
                    None
                }
            }
        });
        let lifecycle = match persisted_lifecycle {
            Some(mut loaded) if loaded.character_id == character_id => {
                // spawn_anchor 由独立的 player_shrine 表维护、可能比 Lifecycle JSON 快照更
                // 新，这里始终以刚查出的权威值覆盖，避免读到断连时刻的过期灵龛坐标。
                loaded.spawn_anchor = spawn_anchor;
                loaded
            }
            _ => Lifecycle {
                character_id: character_id.clone(),
                spawn_anchor,
                ..Default::default()
            },
        };

        commands.entity(entity).insert((
            Wounds::default(),
            Stamina::default(),
            CombatState::default(),
            StatusEffects::default(),
            DerivedAttrs::default(),
            BodyMass::default(),
            Stance::default(),
            AntiCheatCounter::default(),
            carrier::CarrierStore::default(),
            anqi_v2::ContainerSlot::default(),
            player_attack::PlayerAttackCooldown::default(),
            lifecycle,
        ));
    }
}

type JoinedNpcsWithoutCombatBundle<'a> = (valence::prelude::Entity, Option<&'a NpcArchetype>);
type JoinedNpcsWithoutCombatBundleFilter = (Added<NpcMarker>, Without<Wounds>);

fn attach_combat_bundle_to_joined_npcs(
    mut commands: Commands,
    joined_npcs: Query<JoinedNpcsWithoutCombatBundle<'_>, JoinedNpcsWithoutCombatBundleFilter>,
) {
    for (entity, archetype) in &joined_npcs {
        commands.entity(entity).insert((
            Wounds::default(),
            Stamina::default(),
            CombatState::default(),
            StatusEffects::default(),
            DerivedAttrs::default(),
            BodyMass::for_npc_archetype(archetype.copied().unwrap_or_default()),
            Stance::default(),
            carrier::CarrierStore::default(),
            anqi_v2::ContainerSlot::default(),
            Lifecycle {
                character_id: canonical_npc_id(entity),
                ..Default::default()
            },
        ));
    }
}

type RuntimeNpcsWithoutBodyMass<'a> = (valence::prelude::Entity, Option<&'a NpcArchetype>);
type RuntimeNpcsWithoutBodyMassFilter = (Added<NpcMarker>, Without<BodyMass>);

fn attach_body_mass_to_runtime_npcs(
    mut commands: Commands,
    joined_npcs: Query<RuntimeNpcsWithoutBodyMass<'_>, RuntimeNpcsWithoutBodyMassFilter>,
) {
    for (entity, archetype) in &joined_npcs {
        commands.entity(entity).insert((
            BodyMass::for_npc_archetype(archetype.copied().unwrap_or_default()),
            Stance::default(),
        ));
    }
}

pub fn register(app: &mut App) {
    tracing::info!("[bong][combat] registering combat skeleton systems");

    // plan-armor-v1 §1.1：启动期加载护甲 profile 蓝图（template_id -> ArmorProfile）。
    // 失败不 panic: 允许空 registry（未配置护甲数据时不会有减免）。
    let armor_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join(armor::DEFAULT_ARMOR_PROFILES_DIR);
    let armor_registry = armor::ArmorProfileRegistry::load_dir(armor_dir).unwrap_or_else(|e| {
        tracing::error!("[bong][combat][armor] armor profile load failed: {e}");
        armor::ArmorProfileRegistry::new()
    });
    let mut armor_registry = armor_registry;
    if let Err(error) = crate::armor::mundane::register_mundane_armors(&mut armor_registry) {
        tracing::error!("[bong][combat][armor] mundane armor registration failed: {error}");
    }
    tracing::info!(
        "[bong][combat][armor] loaded {} armor profile(s)",
        armor_registry.len()
    );
    app.insert_resource(armor_registry);

    let anticheat_config_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join(DEFAULT_ANTICHEAT_CONFIG_PATH);
    let anticheat_config = load_anticheat_config(anticheat_config_path).unwrap_or_else(|error| {
        tracing::error!("[bong][anticheat] config load failed, using defaults: {error}");
        AntiCheatConfig::default()
    });
    app.insert_resource(anticheat_config);

    app.insert_resource(CombatClock::default());
    app.add_event::<AttackIntent>();
    app.add_event::<DefenseIntent>();
    app.add_event::<ApplyStatusEffectIntent>();
    app.add_event::<CombatEvent>();
    app.add_event::<DeathEvent>();
    app.add_event::<DeathInsightRequested>();
    app.add_event::<DeathCinematicPublished>();
    app.add_event::<style_telemetry::StyleBalanceTelemetryEvent>();
    app.add_event::<RevivalActionIntent>();
    app.add_event::<DebugCombatCommand>();
    app.add_event::<AntiCheatViolationEvent>();
    app.add_event::<knockback::KnockbackEvent>();
    app.add_event::<rat_bite::RatBiteEvent>();
    app.add_event::<needle::ShootNeedleIntent>();
    app.add_event::<needle::QiNeedleChargedEvent>();
    carrier::register(app);
    anqi_v2::register(app);
    app.add_event::<tuike::ShedEvent>();
    app.add_event::<tuike::FalseSkinForgeRequest>();
    app.add_event::<body_conditioning::GuangboTicaoPracticeEvent>();

    app.configure_sets(
        Update,
        (
            CombatSystemSet::Intent,
            CombatSystemSet::Physics,
            CombatSystemSet::Resolve,
            CombatSystemSet::Emit,
        )
            .chain(),
    );
    woliu::register(app);
    yidao::register(app);
    woliu_v2::register(app);
    zhenmai_v2::register(app);
    dugu_v2::register(app);
    baomai_v3::register(app);
    baomai_v4::register(app);
    tuike_v2::register(app);
    app.add_systems(
        Update,
        (
            sword_basics::sword_qi_store_tick.in_set(CombatSystemSet::Physics),
            sword_basics::sword_infuse_completion_tick.in_set(CombatSystemSet::Physics),
        ),
    );

    app.add_systems(
        Update,
        (
            attach_combat_bundle_to_joined_clients
                .after(crate::player::attach_player_state_to_joined_clients)
                // plan-remains-suite：cultivation join attach 可能把已终结角色轮换成
                // 新 current_char_id；combat 的 Lifecycle 必须读取轮换后的持久化状态。
                .after(crate::cultivation::attach_cultivation_to_joined_clients)
                .in_set(CombatSystemSet::Intent),
            attach_combat_bundle_to_joined_npcs.in_set(CombatSystemSet::Intent),
            debug::tick_combat_clock.in_set(CombatSystemSet::Intent),
            resolve::apply_defense_intents.in_set(CombatSystemSet::Intent),
            status::status_effect_apply_tick
                .in_set(CombatSystemSet::Intent)
                .after(resolve::apply_defense_intents),
            lifecycle::wound_bleed_tick.in_set(CombatSystemSet::Physics),
            lifecycle::stamina_tick.in_set(CombatSystemSet::Physics),
            lifecycle::combat_state_tick.in_set(CombatSystemSet::Physics),
            status::status_effect_tick.in_set(CombatSystemSet::Physics),
            status::attribute_aggregate_tick.in_set(CombatSystemSet::Physics),
            resolve::resolve_attack_intents.in_set(CombatSystemSet::Resolve),
            lifecycle::sync_combat_state_from_events
                .in_set(CombatSystemSet::Resolve)
                .after(resolve::resolve_attack_intents),
            lifecycle::death_arbiter_tick
                .in_set(CombatSystemSet::Resolve)
                .in_set(crate::npc::lifecycle::NpcTerminalSystemSet::Stage)
                .after(resolve::resolve_attack_intents)
                // kill.rs 测试里必须显式 .after(handle_kill) 才能跑通——生产注册缺这一条，
                // handle_kill 无序时与仲裁器同 tick 交错，DeathEvent 写在读之后、
                // 随 buffer swap 被吞（实测：/kill self 队列了 DeathEvent 却永远不处理，
                // 无 NearDeath、无死亡屏）。Bevy 0.13+ 事件只在写入当 tick 对"写之后的
                // 读者"可见，跨 tick 由双缓冲交换丢弃。
                .after(crate::cmd::dev::kill::handle_kill),
            lifecycle::near_death_tick
                .in_set(CombatSystemSet::Resolve)
                .in_set(crate::npc::lifecycle::NpcTerminalSystemSet::Stage)
                .after(lifecycle::death_arbiter_tick)
                .after(rat_bite::apply_rat_bite_qi_drain),
            lifecycle::handle_revival_action_intents
                .in_set(CombatSystemSet::Resolve)
                .after(lifecycle::near_death_tick)
                // fix-spec-1901-v2 §4.2 — 复活/新建角色直接写玩家 `Position`，
                // 纳入统一移动 commit set（与 CombatSystemSet::Resolve 并存，
                // 不改变本链内顺序）。
                .in_set(crate::world::movement_commit::AuthoritativePositionCommitSet),
            lifecycle::auto_confirm_revival_decisions
                .in_set(CombatSystemSet::Resolve)
                .after(lifecycle::handle_revival_action_intents),
            debug::drain_combat_events_for_debug
                .in_set(CombatSystemSet::Emit)
                .after(resolve::resolve_attack_intents),
            // plan §13 C1 调试命令消费 — 放 Intent 阶段，早于 WoundBleedTick，
            // 使 /health set / /wound add 当 tick 即可被后续 tick 系统感知。
            debug::apply_debug_combat_commands.in_set(CombatSystemSet::Intent),
            // plan-weapon-v1 §2.3: 装备槽 → Weapon component 同步。放 Intent 阶段,
            // 让 resolve 阶段查 Weapon 时已经是当前 tick 的最新装备状态。
            weapon::sync_weapon_component_from_equipped.in_set(CombatSystemSet::Intent),
            // plan-armor-v1 §1.3: 装备槽(四护甲槽) → DerivedAttrs.defense_profile。
            armor_sync::sync_armor_to_derived_attrs.in_set(CombatSystemSet::Intent),
        ),
    );
    // 活跃战斗窗口逐 tick 精确到期；拆开注册避免超过 Bevy 0.14 系统元组上限。
    app.add_systems(
        Update,
        lifecycle::combat_window_expiry_tick
            .in_set(CombatSystemSet::Physics)
            .after(lifecycle::combat_state_tick),
    );
    // bughunt player-lifecycle-relog-death-consequence-wipe（OPUS 返工要求 2）：断线时正
    // 处于 AwaitingRevival 的角色重连后必须重新收到死亡屏/DeathCinematic，不能静默"裸奔"
    // 在这个阻断攻防、又会被 auto_confirm_revival_decisions 强制结算的状态里。独立
    // add_systems 调用是为了不撑爆上面那个已经 20 项、贴着 Bevy 0.14 tuple-arity 上限的
    // 元组（见文件内既有 "Separate add_systems call to stay below Bevy 0.14 tuple-arity
    // limits" 注释）。
    app.add_systems(
        Update,
        lifecycle::reemit_death_screen_for_reconnected_awaiting_revival_clients
            .in_set(CombatSystemSet::Intent)
            .after(attach_combat_bundle_to_joined_clients),
    );
    app.add_systems(
        Update,
        lifecycle::health_regen_tick
            .in_set(CombatSystemSet::Physics)
            .after(lifecycle::wound_bleed_tick)
            .after(status::status_effect_tick)
            .after(status::attribute_aggregate_tick)
            .after(baomai_v4::scar_circuit::scar_circuit_derive_system)
            .after(baomai_v4::iron_cocoon::iron_cocoon_passive_system)
            .after(baomai_v4::resonance_lock::resonance_lock_tick_system)
            .after(body_conditioning::body_conditioning_aggregate),
    );
    app.add_systems(
        Update,
        (
            body_conditioning::consume_guangbo_practice_events.in_set(CombatSystemSet::Intent),
            body_conditioning::body_conditioning_aggregate
                .in_set(CombatSystemSet::Physics)
                .after(status::attribute_aggregate_tick),
        ),
    );
    app.add_systems(
        Update,
        attach_body_mass_to_runtime_npcs.in_set(CombatSystemSet::Intent),
    );
    app.add_systems(
        Update,
        (
            // plan-knockback-physics-v1 P0: inventory/armor weight and stance feed the unified formula.
            body_mass::sync_body_mass_from_inventory,
            body_mass::sync_stance_from_runtime,
        )
            .in_set(CombatSystemSet::Intent),
    );
    app.add_systems(
        Update,
        status::combat_pill_stamina_status_tick
            .in_set(CombatSystemSet::Physics)
            .before(lifecycle::stamina_tick),
    );
    app.add_systems(
        Update,
        rat_bite::apply_rat_bite_qi_drain
            .in_set(CombatSystemSet::Resolve)
            .in_set(crate::npc::spawn::ambient_scheduler::AmbientTerminalSystemSet::PostRecycle)
            .after(resolve::resolve_attack_intents),
    );
    // plan-ambient-threat-v1 P2: 鼠咬打断打坐（对齐兽潮咬击既有语义），独立于守恒扣减。
    app.add_systems(
        Update,
        rat_bite::interrupt_meditation_on_rat_bite
            .in_set(CombatSystemSet::Resolve)
            .after(resolve::resolve_attack_intents),
    );
    // plan-onboarding-loop-v1 P1.2: 首次受击自学闪身步。
    app.add_systems(
        Update,
        crate::cultivation::first_hit_dash::first_hit_dash_insight
            .in_set(CombatSystemSet::Emit)
            .after(resolve::resolve_attack_intents),
    );
    app.add_systems(
        Update,
        player_attack::handle_player_attack.in_set(CombatSystemSet::Intent),
    );
    app.add_systems(
        Update,
        sword_basics::track_sword_proficiency_from_hits
            .in_set(CombatSystemSet::Emit)
            .after(resolve::resolve_attack_intents),
    );
    // Separate add_systems call to stay below Bevy 0.14 tuple-arity limits.
    app.add_systems(
        Update,
        (
            tuike::handle_false_skin_forge_requests,
            tuike::sync_false_skin_from_inventory,
        )
            .chain()
            .in_set(CombatSystemSet::Intent),
    );
    app.add_systems(
        Update,
        tuike::record_shed_events_in_life_record
            .in_set(CombatSystemSet::Emit)
            .after(resolve::resolve_attack_intents),
    );
    app.add_systems(
        Update,
        (
            needle::resolve_shoot_needle_intents.in_set(CombatSystemSet::Intent),
            needle::despawn_expired_qi_needles.in_set(CombatSystemSet::Physics),
        ),
    );
    app.add_systems(
        Update,
        anticheat::emit_anticheat_threshold_reports
            .in_set(CombatSystemSet::Resolve)
            .after(resolve::resolve_attack_intents),
    );
    app.add_systems(
        Update,
        style_telemetry::collect_hunyuan_pvp_telemetry
            .in_set(CombatSystemSet::Emit)
            .after(lifecycle::death_arbiter_tick),
    );
    app.add_systems(
        Update,
        style_telemetry::publish_style_balance_telemetry_events
            .in_set(CombatSystemSet::Emit)
            .after(style_telemetry::collect_hunyuan_pvp_telemetry),
    );
    app.insert_resource(style_telemetry::StyleUsageCounter::default());
    app.add_systems(
        Update,
        style_telemetry::track_style_tendency
            .in_set(CombatSystemSet::Emit)
            .after(style_telemetry::collect_hunyuan_pvp_telemetry),
    );

    // plan-territory-v1 P2 — PvP 击杀 → 影响力争夺
    // EventReader<DeathEvent> after death_arbiter_tick（保证 DeathEvent 已 emit）。
    app.add_systems(
        Update,
        crate::world::territory::territory_pvp_influence_system
            .in_set(CombatSystemSet::Emit)
            .after(lifecycle::death_arbiter_tick),
    );

    // plan-shield-block-v1 P1 — 盾牌格挡持续状态系统
    app.add_event::<shield_block::RaiseShieldIntent>();
    app.add_event::<shield_block::LowerShieldIntent>();
    app.add_systems(
        Update,
        (
            shield_block::raise_shield_handler.in_set(CombatSystemSet::Intent),
            shield_block::lower_shield_handler
                .in_set(CombatSystemSet::Intent)
                .after(shield_block::raise_shield_handler),
        ),
    );
    app.add_systems(
        Update,
        shield_block::cleanup_shield_on_death
            .in_set(CombatSystemSet::Resolve)
            .after(lifecycle::death_arbiter_tick),
    );
    app.add_systems(
        Update,
        shield_block::cleanup_shield_on_disconnect
            .in_set(CombatSystemSet::Intent)
            .before(crate::player::despawn_disconnected_clients),
    );
    // plan-shield-block-v1 P2 — 体力归零强制放盾（在 Physics set 内 stamina_tick 之后运行）。
    // 注：stamina_tick ∈ Physics，force_lower 也注册到 Physics set 并 .after(stamina_tick)，
    // 避免跨 set 约束（Intent→Physics chain 已确保 set 间全序，跨 set .after 会产生环）。
    app.add_systems(
        Update,
        shield_block::force_lower_shield_on_stamina_exhausted
            .in_set(CombatSystemSet::Physics)
            .after(lifecycle::stamina_tick),
    );
    // plan-shield-block-v1 P4 — 体力低预警 narration（每 80 ticks 一次，防刷屏）。
    // 注册在 Physics set 内 stamina_tick 之后，确保 stamina.current 已更新。
    app.add_systems(
        Update,
        shield_block::shield_low_stamina_narration_tick
            .in_set(CombatSystemSet::Physics)
            .after(lifecycle::stamina_tick),
    );
}
