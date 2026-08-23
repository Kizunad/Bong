use valence::command::graph::CommandGraphBuilder;
use valence::command::handler::CommandResultEvent;
use valence::command::{AddCommand, Command};
use valence::message::SendMessage;
use valence::prelude::{App, Client, EventReader, EventWriter, Query, Res, Update};

use crate::combat::components::{Lifecycle, LifecycleState, Wounds};
use crate::combat::events::DeathEvent;
use crate::cultivation::tick::CultivationClock;

/// dev-only `/kill self` 的死亡 cause 标签——与 combat 里正常死亡 cause 同族格式
/// （如 `"melee:offline:Foo"` / `"bleed_out"`），供 death_arbiter / biography / death insight
/// 按 cause 字符串分类展示时区分「开发指令自杀」与真实战斗/修炼死因。
const DEV_KILL_CAUSE: &str = "dev_kill";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KillCmd {
    Self_,
}

impl Command for KillCmd {
    fn assemble_graph(graph: &mut CommandGraphBuilder<Self>) {
        graph
            .root()
            .literal("kill")
            .literal("self")
            .with_executable(|_| KillCmd::Self_);
    }
}

pub fn register(app: &mut App) {
    app.add_event::<DeathEvent>()
        .add_command::<KillCmd>()
        .add_systems(Update, handle_kill);
}

/// `/kill self` 用途是「触发玩家死亡/复活事件链路」（见 CLAUDE.md dev test commands 表），
/// 因此必须走标准死亡管线：打空血量 + 发标准 DeathEvent，让 death_arbiter_tick 自然接手
/// 进 NearDeath → AwaitingRevival（死亡屏）。不再旁路直发 PlayerTerminated / 手动移除
/// cultivation 组件——那样会跳过整条 NearDeath/死亡屏链路，导致这条 dev 命令测不了真实链路
/// （修复前的 bug：见 plan/bughunt 记录）。
pub fn handle_kill(
    mut events: EventReader<CommandResultEvent<KillCmd>>,
    clock: Option<Res<CultivationClock>>,
    mut deaths: EventWriter<DeathEvent>,
    mut players: Query<(&Lifecycle, &mut Wounds, &mut Client)>,
) {
    let tick = clock.as_deref().map(|clock| clock.tick).unwrap_or_default();
    for event in events.read() {
        let Ok((lifecycle, mut wounds, mut client)) = players.get_mut(event.executor) else {
            continue;
        };
        if lifecycle.state != LifecycleState::Alive {
            client.send_chat_message(format!(
                "[dev] kill self ignored: lifecycle={:?}",
                lifecycle.state
            ));
            continue;
        }

        wounds.health_current = 0.0;
        deaths.send(DeathEvent {
            target: event.executor,
            cause: DEV_KILL_CAUSE.to_string(),
            attacker: Some(event.executor),
            attacker_player_id: None,
            at_tick: tick,
        });
        tracing::warn!(
            "[dev-cmd] kill self: queued standard DeathEvent (cause={DEV_KILL_CAUSE}) for {:?}",
            event.executor
        );
        client.send_chat_message("[dev] kill self queued DeathEvent");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cmd::dev::test_support::{run_update, spawn_test_client};
    use crate::combat::lifecycle::death_arbiter_tick;
    use crate::combat::CombatClock;
    use crate::cultivation::components::Cultivation;
    use crate::cultivation::death_hooks::{CultivationDeathTrigger, PlayerTerminated};
    use crate::cultivation::life_record::{BiographyEntry, LifeRecord};
    use crate::network::vfx_event_emit::VfxEventRequest;
    use crate::persistence::{bootstrap_sqlite, PersistenceSettings};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};
    use valence::prelude::{Events, IntoSystemConfigs};

    fn unique_temp_dir(test_name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "bong-dev-kill-{test_name}-{}-{nanos}",
            std::process::id()
        ))
    }

    fn persistence_settings(test_name: &str) -> (PersistenceSettings, PathBuf) {
        let root = unique_temp_dir(test_name);
        let db_path = root.join("data").join("bong.db");
        let run_id = format!("dev-kill-{test_name}");
        bootstrap_sqlite(&db_path, &run_id).expect("sqlite bootstrap should succeed");
        (PersistenceSettings::with_db_path(&db_path, run_id), root)
    }

    /// 只跑 handle_kill 本身（不接死亡链路下游），用于锁住"这条 dev 命令做了什么"的窄契约：
    /// 打空血量 + 发一条标准 DeathEvent，不直接碰 Lifecycle/持久化。
    fn setup_app(test_name: &str) -> (App, PathBuf) {
        let (settings, root) = persistence_settings(test_name);
        let mut app = App::new();
        app.insert_resource(CultivationClock { tick: 77 });
        app.insert_resource(settings);
        app.add_event::<CommandResultEvent<KillCmd>>();
        app.add_event::<DeathEvent>();
        app.add_systems(Update, handle_kill);
        (app, root)
    }

    /// 接上 death_arbiter_tick，跑完整链路：dev kill → DeathEvent → NearDeath。
    fn setup_full_pipeline_app(test_name: &str) -> (App, PathBuf) {
        let (settings, root) = persistence_settings(test_name);
        let mut app = App::new();
        app.insert_resource(CultivationClock { tick: 77 });
        app.insert_resource(CombatClock { tick: 77 });
        app.insert_resource(settings);
        app.add_event::<CommandResultEvent<KillCmd>>();
        app.add_event::<DeathEvent>();
        app.add_event::<CultivationDeathTrigger>();
        app.add_event::<PlayerTerminated>();
        app.add_event::<VfxEventRequest>();
        app.add_systems(Update, (handle_kill, death_arbiter_tick.after(handle_kill)));
        (app, root)
    }

    fn spawn_player(app: &mut App, lifecycle: Lifecycle) -> valence::prelude::Entity {
        let player = spawn_test_client(app, "Alice", [0.0, 0.0, 0.0]);
        app.world_mut().entity_mut(player).insert((
            lifecycle,
            Wounds::default(),
            LifeRecord::new("offline:Alice"),
            Cultivation::default(),
        ));
        player
    }

    fn send(app: &mut App, player: valence::prelude::Entity) {
        app.world_mut()
            .resource_mut::<Events<CommandResultEvent<KillCmd>>>()
            .send(CommandResultEvent {
                result: KillCmd::Self_,
                executor: player,
                modifiers: Default::default(),
            });
    }

    #[test]
    fn kill_self_zeroes_health_and_emits_standard_death_event() {
        let (mut app, root) = setup_app("emits-death-event");
        let player = spawn_player(&mut app, Lifecycle::default());

        send(&mut app, player);
        run_update(&mut app);

        let wounds = app.world().get::<Wounds>(player).unwrap();
        assert_eq!(
            wounds.health_current, 0.0,
            "期望 health_current 被打到 0 因为 dev kill 要触发标准濒死路径的血量判定；实际 {}",
            wounds.health_current
        );

        let death_events = app.world().resource::<Events<DeathEvent>>();
        let collected = death_events
            .get_reader()
            .read(death_events)
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(
            collected.len(),
            1,
            "期望恰好一条 DeathEvent 因为一次 kill self 只应触发一次死亡；实际 {} 条",
            collected.len()
        );
        assert_eq!(collected[0].target, player);
        assert_eq!(
            collected[0].attacker,
            Some(player),
            "期望 attacker=target=self（dev kill 是自我了结，非他人击杀）；实际 {:?}",
            collected[0].attacker
        );
        assert_eq!(collected[0].cause, DEV_KILL_CAUSE);
        assert_eq!(collected[0].at_tick, 77);

        // handle_kill 自身不应直接改写 Lifecycle——状态转换交给 death_arbiter_tick，
        // 否则又会退化回旁路死亡链路的旧 bug。
        assert_eq!(
            app.world().get::<Lifecycle>(player).unwrap().state,
            LifecycleState::Alive,
            "期望 handle_kill 不直接修改 lifecycle.state（应由 death_arbiter_tick 消费 DeathEvent 后转换）"
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn kill_self_is_noop_when_already_near_death() {
        let (mut app, root) = setup_app("noop-near-death");
        let mut lifecycle = Lifecycle::default();
        lifecycle.enter_near_death(10);
        let player = spawn_player(&mut app, lifecycle);

        send(&mut app, player);
        run_update(&mut app);

        assert_eq!(
            app.world().resource::<Events<DeathEvent>>().len(),
            0,
            "期望 NearDeath 状态下 kill self 不再发新的 DeathEvent（避免和 Bug 1 一样的重入问题）"
        );
        let wounds = app.world().get::<Wounds>(player).unwrap();
        assert_eq!(
            wounds.health_current,
            Wounds::default().health_current,
            "期望血量不被 kill self 改动因为整条命令应在状态检查处提前 continue"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn kill_self_is_noop_when_wounds_component_missing() {
        // 边界：entity 没有 Wounds（例如尚未完成初始化的连接中间态）——查询该 tuple 拿不到
        // entity 应该静默跳过这一条 CommandResultEvent，而不是 panic。
        let (mut app, root) = setup_app("missing-wounds");
        let player = spawn_test_client(&mut app, "Alice", [0.0, 0.0, 0.0]);
        app.world_mut()
            .entity_mut(player)
            .insert(Lifecycle::default());

        send(&mut app, player);
        run_update(&mut app);

        assert_eq!(
            app.world().resource::<Events<DeathEvent>>().len(),
            0,
            "期望缺 Wounds 组件时 kill self 静默跳过（Query::get_mut 失败 continue），不发 DeathEvent"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn kill_self_is_noop_when_already_terminated() {
        let (mut app, root) = setup_app("noop-terminated");
        let mut lifecycle = Lifecycle::default();
        lifecycle.terminate(10);
        let player = spawn_player(&mut app, lifecycle);

        send(&mut app, player);
        run_update(&mut app);

        assert_eq!(
            app.world().resource::<Events<DeathEvent>>().len(),
            0,
            "期望已终结（Terminated）玩家 kill self 是 noop，不应再发死亡事件"
        );
        assert_eq!(
            app.world()
                .get::<Lifecycle>(player)
                .unwrap()
                .last_death_tick,
            Some(10),
            "期望 last_death_tick 保持终结时刻不变（kill self 不应覆写既有死亡记录）"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn kill_self_reaches_near_death_through_standard_pipeline_keeping_cultivation() {
        // 回归契约：dev kill 必须走标准死亡管线进 NearDeath（死亡屏前置状态），
        // 而不是旧 bug 里直接跳到 Terminated；cultivation 组件必须还在（旧 bug 会移除它）；
        // biography 必须留下一条 cause="dev_kill" 的 NearDeath 记录，这样这条 dev 命令才真的
        // 测得到 NearDeath → 死亡屏 → 复活/终结的完整链路。
        let (mut app, root) = setup_full_pipeline_app("reaches-near-death");
        let player = spawn_player(&mut app, Lifecycle::default());

        send(&mut app, player);
        run_update(&mut app);

        let lifecycle = app.world().get::<Lifecycle>(player).unwrap();
        assert_eq!(
            lifecycle.state,
            LifecycleState::NearDeath,
            "期望 dev kill 落在 NearDeath（死亡屏前置状态）因为 CLAUDE.md 声明 /kill self 要触发死亡/复活链路；实际 {:?}",
            lifecycle.state
        );

        assert!(
            app.world().get::<Cultivation>(player).is_some(),
            "期望 cultivation 组件仍在——标准死亡管线只在 AwaitingRevival 决策失败/主动终结时才会真正终结角色，\
             不应在刚进 NearDeath 时就被移除"
        );

        let life_record = app.world().get::<LifeRecord>(player).unwrap();
        assert!(
            matches!(
                life_record.biography.last(),
                Some(BiographyEntry::NearDeath { cause, .. }) if cause == DEV_KILL_CAUSE
            ),
            "期望 biography 尾条是 cause=\"dev_kill\" 的 NearDeath 记录；实际 {:?}",
            life_record.biography.last()
        );

        let _ = fs::remove_dir_all(root);
    }
}
