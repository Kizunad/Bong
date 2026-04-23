//! 修仙系统 — plan-cultivation-v1 完整切片（server 侧 P1–P5）。
//!
//! 子模块：
//!   * components       — 状态定义（Cultivation / MeridianSystem / QiColor / Karma / Contamination）
//!   * topology         — 20 经邻接表 Resource
//!   * tick             — QiRegenTick + ZoneQiDrainTick（零和合并实现）
//!   * meridian_open    — MeridianOpenTick（含 MeridianTarget Component）
//!   * breakthrough     — 5 阶升境事务
//!   * tribulation      — 化虚渡劫状态机（Spirit→Void）
//!   * forging          — rate / capacity 独立锻造
//!   * composure        — 心境缓慢回升
//!   * qi_zero_decay    — 爆脉降境 + LIFO 经脉封闭
//!   * color            — QiColorEvolutionTick
//!   * contamination    — 异种真元排异（10:15）
//!   * overload         — 超量流量 → 裂痕
//!   * heal             — 裂痕愈合
//!   * negative_zone    — 负灵域反吸
//!   * death_hooks      — 死亡触发 & 重生惩罚 & 终结清理
//!   * lifespan         — 寿元 / 死亡登记 / 重生概率纯模型
//!   * life_record      — 修炼生平卷
//!   * karma            — 业力极慢衰减
//!   * insight / insight_fallback / insight_apply — 顿悟系统
//!
//! 跨仓库 TODO：
//!   * 客户端 inspect UI + 目标选择对话框（plan §7）
//!   * agent LLM runtime（InsightRequest → InsightOffer 桥）
//!   * 战斗 plan：消费 CultivationDeathTrigger / TribulationFailed / throughput 写入

pub mod breakthrough;
pub mod color;
pub mod components;
pub mod composure;
pub mod contamination;
pub mod death_hooks;
pub mod forging;
pub mod heal;
pub mod insight;
pub mod insight_apply;
pub mod insight_fallback;
pub mod insight_flow;
pub mod karma;
pub mod life_record;
pub mod lifespan;
pub mod meridian_open;
pub mod negative_zone;
pub mod overload;
pub mod qi_zero_decay;
pub mod tick;
pub mod topology;
pub mod tribulation;

use valence::prelude::{
    Added, App, Client, Commands, Entity, IntoSystemConfigs, Query, Res, Update, Username, Without,
};

use self::breakthrough::{breakthrough_system, BreakthroughOutcome, BreakthroughRequest};
use self::color::{qi_color_evolution_tick, PracticeLog};
use self::components::{Contamination, Cultivation, Karma, MeridianSystem, QiColor};
use self::composure::composure_tick;
use self::contamination::contamination_tick;
use self::death_hooks::{
    on_player_revived, on_player_terminated, CultivationDeathTrigger, PlayerRevived,
    PlayerTerminated,
};
use self::forging::{forging_system, ForgeOutcome, ForgeRequest};
use self::heal::meridian_heal_tick;
use self::insight::{
    InsightChosen, InsightOffer, InsightQuota, InsightRequest, InsightTriggerRegistry,
};
use self::insight_apply::{InsightModifiers, UnlockedPerceptions};
use self::insight_flow::{
    apply_insight_chosen, insight_trigger_on_breakthrough, insight_trigger_on_forge,
    process_insight_request,
};
use self::karma::karma_decay_tick;
use self::life_record::LifeRecord;
use self::lifespan::{DeathRegistry, LifespanCapTable, LifespanComponent};
use self::meridian_open::meridian_open_tick;
use self::negative_zone::negative_zone_siphon_tick;
use self::overload::overload_detection_tick;
use self::qi_zero_decay::{qi_zero_decay_tick, RealmRegressed};
use self::tick::{qi_regen_and_zone_drain_tick, CultivationClock};
use self::topology::MeridianTopology;
use self::tribulation::{
    start_tribulation_system, tribulation_failure_system, tribulation_wave_system,
    InitiateXuhuaTribulation, TribulationAnnounce, TribulationFailed, TribulationState,
    TribulationWaveCleared,
};
use crate::cultivation::components::Realm;
use crate::persistence::{load_active_tribulation, PersistenceSettings};
use crate::player::state::canonical_player_id;
use crate::player::state::PlayerState;

pub fn register(app: &mut App) {
    tracing::info!("[bong][cultivation] registering cultivation systems (plan P1–P5)");
    app.insert_resource(MeridianTopology::standard());
    app.insert_resource(CultivationClock::default());
    app.insert_resource(InsightTriggerRegistry::with_defaults());

    // 事件（plan §3/§4/§5 全家桶）
    app.add_event::<BreakthroughRequest>();
    app.add_event::<BreakthroughOutcome>();
    app.add_event::<ForgeRequest>();
    app.add_event::<ForgeOutcome>();
    app.add_event::<RealmRegressed>();
    app.add_event::<CultivationDeathTrigger>();
    app.add_event::<PlayerRevived>();
    app.add_event::<PlayerTerminated>();
    app.add_event::<InitiateXuhuaTribulation>();
    app.add_event::<TribulationAnnounce>();
    app.add_event::<TribulationWaveCleared>();
    app.add_event::<TribulationFailed>();
    app.add_event::<InsightRequest>();
    app.add_event::<InsightOffer>();
    app.add_event::<InsightChosen>();

    // Bevy IntoSystemConfigs 最多 20 个元素；拆两组。
    app.add_systems(
        Update,
        (
            attach_cultivation_to_joined_clients
                .after(crate::player::attach_player_state_to_joined_clients),
            // 核心 tick：回气/扣 zone → 打通 → 事务
            qi_regen_and_zone_drain_tick,
            meridian_open_tick.after(qi_regen_and_zone_drain_tick),
            breakthrough_system.after(meridian_open_tick),
            forging_system.after(breakthrough_system),
            // 稳态演化
            qi_color_evolution_tick,
            composure_tick,
            qi_zero_decay_tick.after(qi_regen_and_zone_drain_tick),
            // plan §2.1 损伤/净化链
            overload_detection_tick.after(meridian_open_tick),
            contamination_tick.after(qi_regen_and_zone_drain_tick),
            meridian_heal_tick.after(overload_detection_tick),
            negative_zone_siphon_tick.after(qi_regen_and_zone_drain_tick),
            // plan §3.2 渡劫
            start_tribulation_system,
            tribulation_wave_system,
            tribulation_failure_system,
            // plan §4 死亡/重生钩子
            on_player_revived,
            on_player_terminated,
            // plan §11-5 业力
            karma_decay_tick,
        ),
    );
    app.add_systems(
        Update,
        (
            // plan §5.4 / §5.5 顿悟流水线
            insight_trigger_on_breakthrough.after(breakthrough_system),
            insight_trigger_on_forge.after(forging_system),
            process_insight_request
                .after(insight_trigger_on_breakthrough)
                .after(insight_trigger_on_forge),
            apply_insight_chosen.after(process_insight_request),
        ),
    );
}

type CultivationAttachFilter = (Added<Client>, Without<Cultivation>);

fn attach_cultivation_to_joined_clients(
    mut commands: Commands,
    settings: Res<PersistenceSettings>,
    joined_clients: Query<(Entity, &Username, Option<&PlayerState>), CultivationAttachFilter>,
) {
    for (entity, username, player_state) in &joined_clients {
        let mut cultivation = Cultivation::default();
        if let Some(player_state) = player_state {
            cultivation.qi_current = player_state.spirit_qi;
            cultivation.qi_max = player_state.spirit_qi_max.max(1.0);
            if let Some(restored_realm) =
                cultivation_realm_from_player_state(player_state.realm.as_str())
            {
                cultivation.realm = restored_realm;
            }
        }

        let canonical_id = canonical_player_id(username.0.as_str());
        let active_tribulation = match load_active_tribulation(&settings, canonical_id.as_str()) {
            Ok(record) => record,
            Err(error) => {
                tracing::warn!(
                    "[bong][cultivation] failed to load active tribulation for {}: {error}",
                    canonical_id,
                );
                None
            }
        };
        let restored_tribulation = active_tribulation.as_ref().map(|record| TribulationState {
            wave_current: record
                .wave_current
                .saturating_add(1)
                .min(record.waves_total),
            waves_total: record.waves_total,
            started_tick: record.started_tick,
        });
        if restored_tribulation.is_some() {
            cultivation.realm = Realm::Spirit;
        }
        let lifespan = LifespanComponent::new(LifespanCapTable::for_player_state_realm(
            player_state.map(|state| state.realm.as_str()),
            cultivation.realm,
        ));

        let mut entity_commands = commands.entity(entity);
        entity_commands.insert((
            cultivation,
            MeridianSystem::default(),
            QiColor::default(),
            Karma::default(),
            PracticeLog::default(),
            Contamination::default(),
            LifeRecord::new(canonical_id.clone()),
            DeathRegistry::new(canonical_id.clone()),
            lifespan,
            InsightQuota::default(),
            UnlockedPerceptions::default(),
            InsightModifiers::new(),
        ));
        if let Some(restored_tribulation) = restored_tribulation {
            entity_commands.insert(restored_tribulation);
        }
        tracing::info!("[bong][cultivation] attached full cultivation bundle to {entity:?}");
    }
}

fn cultivation_realm_from_player_state(realm: &str) -> Option<Realm> {
    match realm {
        "mortal" => Some(Realm::Awaken),
        "qi_refining_1" => Some(Realm::Induce),
        "qi_refining_2" => Some(Realm::Condense),
        "qi_refining_3" | "foundation_establishment_1" => Some(Realm::Spirit),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::combat::components::Lifecycle;
    use crate::cultivation::lifespan::{DeathRegistry, LifespanCapTable, LifespanComponent};
    use crate::persistence::{
        load_active_tribulation, load_ascension_quota, persist_active_tribulation,
        ActiveTribulationRecord, PersistenceSettings,
    };
    use crate::player::state::canonical_player_id;
    use crate::player::state::PlayerState;
    use valence::prelude::App;
    use valence::testing::create_mock_client;

    #[test]
    fn joined_clients_receive_canonical_player_character_id() {
        let mut app = App::new();
        app.insert_resource(PersistenceSettings::default());
        app.add_systems(Update, attach_cultivation_to_joined_clients);

        let (client_bundle, _helper) = create_mock_client("Alice");
        let entity = app.world_mut().spawn(client_bundle).id();

        app.update();

        let life_record = app
            .world()
            .get::<LifeRecord>(entity)
            .expect("joined client should receive a LifeRecord");
        let death_registry = app
            .world()
            .get::<DeathRegistry>(entity)
            .expect("joined client should receive a DeathRegistry");
        let lifespan = app
            .world()
            .get::<LifespanComponent>(entity)
            .expect("joined client should receive a LifespanComponent");

        assert_eq!(life_record.character_id, canonical_player_id("Alice"));
        assert_eq!(death_registry.char_id, canonical_player_id("Alice"));
        assert_eq!(lifespan.cap_by_realm, LifespanCapTable::AWAKEN);
    }

    #[test]
    fn mortal_player_state_starts_with_mortal_lifespan_cap() {
        let mut app = App::new();
        app.insert_resource(PersistenceSettings::default());
        app.add_systems(Update, attach_cultivation_to_joined_clients);

        let (client_bundle, _helper) = create_mock_client("Novice");
        let entity = app
            .world_mut()
            .spawn((
                client_bundle,
                PlayerState {
                    realm: "mortal".to_string(),
                    spirit_qi: 0.0,
                    spirit_qi_max: 10.0,
                    karma: 0.0,
                    experience: 0,
                    inventory_score: 0.0,
                },
            ))
            .id();

        app.update();

        let lifespan = app
            .world()
            .get::<LifespanComponent>(entity)
            .expect("joined client should receive a LifespanComponent");

        assert_eq!(lifespan.cap_by_realm, LifespanCapTable::MORTAL);
    }

    #[test]
    fn joined_clients_restore_active_tribulation_from_persistence() {
        let temp_root = std::env::temp_dir().join(format!(
            "bong-cultivation-tribulation-restore-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock should be after unix epoch")
                .as_nanos(),
        ));
        let db_path = temp_root.join("data").join("bong.db");
        let deceased_dir = temp_root
            .join("library-web")
            .join("public")
            .join("deceased");
        let settings = PersistenceSettings::with_paths(&db_path, &deceased_dir, "cultivation-test");
        crate::persistence::bootstrap_sqlite(settings.db_path(), settings.server_run_id())
            .expect("bootstrap should succeed");
        persist_active_tribulation(
            &settings,
            &ActiveTribulationRecord {
                char_id: canonical_player_id("Alice"),
                wave_current: 2,
                waves_total: 5,
                started_tick: 1440,
            },
        )
        .expect("active tribulation should persist");

        let mut app = App::new();
        app.insert_resource(settings);
        app.add_systems(Update, attach_cultivation_to_joined_clients);

        let (client_bundle, _helper) = create_mock_client("Alice");
        let entity = app
            .world_mut()
            .spawn((
                client_bundle,
                PlayerState {
                    realm: "qi_refining_3".to_string(),
                    spirit_qi: 88.0,
                    spirit_qi_max: 120.0,
                    karma: 0.0,
                    experience: 3000,
                    inventory_score: 0.0,
                },
            ))
            .id();

        app.update();

        let cultivation = app
            .world()
            .get::<Cultivation>(entity)
            .expect("cultivation should attach");
        let tribulation = app
            .world()
            .get::<TribulationState>(entity)
            .expect("tribulation should restore");
        assert_eq!(cultivation.realm, Realm::Spirit);
        assert_eq!(tribulation.wave_current, 3);
        assert_eq!(tribulation.waves_total, 5);
        assert_eq!(tribulation.started_tick, 1440);

        let _ = std::fs::remove_dir_all(temp_root);
    }

    #[test]
    fn joined_clients_cap_restored_auto_pass_wave_at_total_waves() {
        let temp_root = std::env::temp_dir().join(format!(
            "bong-cultivation-tribulation-restore-cap-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock should be after unix epoch")
                .as_nanos(),
        ));
        let db_path = temp_root.join("data").join("bong.db");
        let deceased_dir = temp_root
            .join("library-web")
            .join("public")
            .join("deceased");
        let settings = PersistenceSettings::with_paths(&db_path, &deceased_dir, "cultivation-test");
        crate::persistence::bootstrap_sqlite(settings.db_path(), settings.server_run_id())
            .expect("bootstrap should succeed");
        persist_active_tribulation(
            &settings,
            &ActiveTribulationRecord {
                char_id: canonical_player_id("Azure"),
                wave_current: 5,
                waves_total: 5,
                started_tick: 1888,
            },
        )
        .expect("active tribulation should persist");

        let mut app = App::new();
        app.insert_resource(settings);
        app.add_systems(Update, attach_cultivation_to_joined_clients);

        let (client_bundle, _helper) = create_mock_client("Azure");
        let entity = app
            .world_mut()
            .spawn((
                client_bundle,
                PlayerState {
                    realm: "qi_refining_3".to_string(),
                    spirit_qi: 120.0,
                    spirit_qi_max: 160.0,
                    karma: 0.0,
                    experience: 4800,
                    inventory_score: 0.0,
                },
            ))
            .id();

        app.update();

        let tribulation = app
            .world()
            .get::<TribulationState>(entity)
            .expect("tribulation should restore");
        assert_eq!(tribulation.wave_current, 5);
        assert_eq!(tribulation.waves_total, 5);
        assert_eq!(tribulation.started_tick, 1888);

        let _ = std::fs::remove_dir_all(temp_root);
    }

    #[test]
    fn restored_tribulation_completion_clears_active_row_and_awards_quota() {
        let temp_root = std::env::temp_dir().join(format!(
            "bong-cultivation-tribulation-restore-complete-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock should be after unix epoch")
                .as_nanos(),
        ));
        let db_path = temp_root.join("data").join("bong.db");
        let deceased_dir = temp_root
            .join("library-web")
            .join("public")
            .join("deceased");
        let settings = PersistenceSettings::with_paths(&db_path, &deceased_dir, "cultivation-test");
        crate::persistence::bootstrap_sqlite(settings.db_path(), settings.server_run_id())
            .expect("bootstrap should succeed");
        persist_active_tribulation(
            &settings,
            &ActiveTribulationRecord {
                char_id: canonical_player_id("Azure"),
                wave_current: 4,
                waves_total: 5,
                started_tick: 2880,
            },
        )
        .expect("active tribulation should persist");

        let mut app = App::new();
        app.insert_resource(settings.clone());
        app.add_event::<tribulation::TribulationWaveCleared>();
        app.add_event::<crate::skill::events::SkillCapChanged>();
        app.add_systems(
            Update,
            (
                attach_cultivation_to_joined_clients,
                tribulation::tribulation_wave_system,
            ),
        );

        let (client_bundle, _helper) = create_mock_client("Azure");
        let entity = app
            .world_mut()
            .spawn((
                client_bundle,
                PlayerState {
                    realm: "qi_refining_3".to_string(),
                    spirit_qi: 120.0,
                    spirit_qi_max: 160.0,
                    karma: 0.0,
                    experience: 4800,
                    inventory_score: 0.0,
                },
                Lifecycle {
                    character_id: canonical_player_id("Azure"),
                    death_count: 0,
                    fortune_remaining: 1,
                    last_death_tick: None,
                    last_revive_tick: None,
                    near_death_deadline_tick: None,
                    weakened_until_tick: None,
                    state: crate::combat::components::LifecycleState::Alive,
                },
            ))
            .id();

        app.update();

        let restored = app
            .world()
            .get::<tribulation::TribulationState>(entity)
            .expect("tribulation should restore");
        assert_eq!(restored.wave_current, 5);
        assert_eq!(restored.waves_total, 5);

        app.world_mut()
            .resource_mut::<valence::prelude::Events<tribulation::TribulationWaveCleared>>()
            .send(tribulation::TribulationWaveCleared { entity, wave: 5 });

        app.update();

        let cultivation = app
            .world()
            .get::<Cultivation>(entity)
            .expect("cultivation should still be attached");
        assert_eq!(cultivation.realm, Realm::Void);
        assert!(
            app.world()
                .get::<tribulation::TribulationState>(entity)
                .is_none(),
            "tribulation state should be removed after ascension"
        );

        let active = load_active_tribulation(&settings, canonical_player_id("Azure").as_str())
            .expect("active tribulation query should succeed");
        assert!(active.is_none(), "active tribulation row should be cleared");

        let quota = load_ascension_quota(&settings).expect("quota load should succeed");
        assert_eq!(quota.occupied_slots, 1);

        let _ = std::fs::remove_dir_all(temp_root);
    }
}
