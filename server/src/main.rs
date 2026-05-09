#[allow(dead_code)]
mod alchemy;
mod audio;
mod botany;
mod cmd;
mod combat;
// craft：plan-craft-v1 P0+P1 通用手搓底盘。register() 注入 5 示例配方 + resources +
// events；P2/P3（client UI + agent narration + 三渠道 hook）由 plan vN+1 接入。
// 当前未在 Update systems 内消费 CraftStartedEvent / CraftCompletedEvent，
// 等 P2/P3 接入前保留 #[allow(dead_code)]。
#[allow(dead_code)]
mod craft;
#[allow(dead_code)]
mod cultivation;
#[allow(dead_code)]
mod death_lifecycle;
#[allow(dead_code)]
mod economy;
#[allow(dead_code)]
mod fauna;
#[allow(dead_code)]
mod forge;
// identity：P0 锁定数据模型 + persistence；P1 起 /identity slash / consumer / scorer
// 等会逐步消费这些 API，初期保留 #[allow(dead_code)]，每个 P 接入后再收口。
#[allow(dead_code)]
mod identity;
mod inventory;
#[allow(dead_code)]
mod lingtian;
// mineral：M3 注册 MineralRegistry + MineralOreIndex + DiggingEvent listener；
// M2 worldgen 接入前 OreIndex 始终空，listener 对所有 block 静默 no-op。
#[allow(dead_code)]
mod mineral;
#[allow(dead_code)]
mod mob;
mod network;
mod npc;
mod persistence;
mod player;
#[allow(dead_code)]
mod qi_physics;
// preview：worldgen-snapshot harness 用的 server-side teleport hook。仅在
// BONG_PREVIEW_MODE=1 env 下激活实际 system；register() 一定会注册 event 类型
// 让 chat_collector 编译通过。
mod preview;
#[allow(dead_code)]
mod schema;
mod skin;
mod social;
mod spiritwood;
// shelflife：M3a 注册 DecayProfileRegistry resource；compute_* / container_* 等
// 辅助仍未被 system 调用（M5 消费侧接入前）— 故保留 #[allow(dead_code)]。
#[allow(dead_code)]
mod shelflife;
mod skill;
#[allow(dead_code)]
mod tools;
mod world;
#[allow(dead_code)]
mod worldgen;
mod zhenfa;
#[allow(dead_code)]
mod zhenfa_hooks;

use crossbeam_channel::unbounded;
use network::agent_bridge::{
    spawn_mock_bridge_daemon, AgentCommand, GameEvent, NetworkBridgeResource,
};
use persistence::{
    bootstrap_sqlite, export_zone_persistence, import_zone_persistence, PersistenceSettings,
    ZoneExportBundle,
};
use player::state::{
    export_player_bundle, import_player_bundle, PlayerExportBundle, PlayerStatePersistence,
};
use valence::log::LogPlugin;
use valence::prelude::*;

fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_target(false)
        .with_writer(std::io::stderr)
        .try_init();
}

fn main() {
    init_tracing();

    if let Err(code) = run_cli(std::env::args()) {
        std::process::exit(code);
    }

    run_server();
}

fn run_server() {
    let (tx_to_game, rx_from_agent) = unbounded::<AgentCommand>();
    let (tx_to_agent, rx_from_game) = unbounded::<GameEvent>();

    std::mem::drop(spawn_mock_bridge_daemon(tx_to_game, rx_from_game));

    let mut app = App::new();
    app.insert_resource(NetworkSettings {
        connection_mode: ConnectionMode::Offline,
        ..Default::default()
    })
    .insert_resource(NetworkBridgeResource::new(tx_to_agent, rx_from_agent))
    .add_plugins(DefaultPlugins.build().disable::<LogPlugin>());

    world::register(&mut app);
    player::register(&mut app);
    qi_physics::register(&mut app);
    skin::register(&mut app);
    inventory::register(&mut app);
    botany::register(&mut app);
    cmd::register(&mut app);
    skill::register(&mut app);
    tools::register(&mut app);
    cultivation::register(&mut app);
    fauna::register(&mut app);
    alchemy::register(&mut app);
    craft::register(&mut app);
    audio::register(&mut app);
    combat::register(&mut app);
    social::register(&mut app);
    identity::register(&mut app);
    death_lifecycle::register(&mut app);
    spiritwood::register(&mut app);
    forge::register(&mut app);
    lingtian::register(&mut app);
    mineral::register(&mut app);
    shelflife::register(&mut app);
    economy::register(&mut app);
    npc::register(&mut app);
    zhenfa::register(&mut app);
    network::register(&mut app);
    persistence::register(&mut app);
    preview::register(&mut app);

    app.run();
}

fn run_cli(args: impl Iterator<Item = String>) -> Result<(), i32> {
    let mut args = args.skip(1);
    let Some(command) = args.next() else {
        return Ok(());
    };

    match command.as_str() {
        "export" => {
            let Some(target) = args.next() else {
                eprintln!("用法: bong-server export zones | bong-server export --player <name>");
                return Err(2);
            };

            match target.as_str() {
                "zones" => {
                    if args.next().is_some() {
                        eprintln!("用法: bong-server export zones");
                        return Err(2);
                    }
                    let settings = PersistenceSettings::default();
                    if let Err(error) = bootstrap_sqlite(settings.db_path(), "cli-export-zones") {
                        eprintln!("初始化导出数据库失败: {error}");
                        return Err(1);
                    }

                    match export_zone_persistence(&settings) {
                        Ok(bundle) => {
                            let rendered = serde_json::to_string_pretty(&bundle)
                                .expect("zone export should serialize");
                            println!("{rendered}");
                            Err(0)
                        }
                        Err(error) => {
                            eprintln!("导出 zones 失败: {error}");
                            Err(1)
                        }
                    }
                }
                "--player" => {
                    let Some(username) = args.next() else {
                        eprintln!("用法: bong-server export --player <name>");
                        return Err(2);
                    };
                    if args.next().is_some() {
                        eprintln!("用法: bong-server export --player <name>");
                        return Err(2);
                    }

                    let persistence = PlayerStatePersistence::default();
                    if let Err(error) = bootstrap_sqlite(persistence.db_path(), "cli-export-player")
                    {
                        eprintln!("初始化导出数据库失败: {error}");
                        return Err(1);
                    }

                    match export_player_bundle(&persistence, username.as_str()) {
                        Ok(bundle) => {
                            let rendered = serde_json::to_string_pretty(&bundle)
                                .expect("player export should serialize");
                            println!("{rendered}");
                            Err(0)
                        }
                        Err(error) => {
                            eprintln!("导出 player 失败: {error}");
                            Err(1)
                        }
                    }
                }
                _ => {
                    eprintln!("未知导出目标: {target}");
                    Err(2)
                }
            }
        }
        "import" => {
            if !cli_dev_mode_enabled() {
                eprintln!("导入命令仅允许在 dev 模式下执行（设置 BONG_DEV_MODE=1）");
                return Err(2);
            }

            let Some(first_arg) = args.next() else {
                eprintln!("用法: bong-server import zones --file <path> | bong-server import --file <path>");
                return Err(2);
            };

            let (target, path) = if first_arg == "--file" {
                let Some(path) = args.next() else {
                    eprintln!("用法: bong-server import --file <path>");
                    return Err(2);
                };
                if args.next().is_some() {
                    eprintln!("用法: bong-server import --file <path>");
                    return Err(2);
                }
                (None, path)
            } else {
                let Some(flag) = args.next() else {
                    eprintln!("用法: bong-server import zones --file <path>");
                    return Err(2);
                };
                let Some(path) = args.next() else {
                    eprintln!("用法: bong-server import zones --file <path>");
                    return Err(2);
                };
                if flag != "--file" || args.next().is_some() {
                    eprintln!("用法: bong-server import zones --file <path>");
                    return Err(2);
                }
                (Some(first_arg), path)
            };

            let raw = match std::fs::read_to_string(&path) {
                Ok(raw) => raw,
                Err(error) => {
                    eprintln!("读取导入文件失败: {error}");
                    return Err(1);
                }
            };

            match target.as_deref() {
                Some("zones") => {
                    let settings = PersistenceSettings::default();
                    if let Err(error) = bootstrap_sqlite(settings.db_path(), "cli-import-zones") {
                        eprintln!("初始化导入数据库失败: {error}");
                        return Err(1);
                    }

                    let bundle: ZoneExportBundle = match serde_json::from_str(&raw) {
                        Ok(bundle) => bundle,
                        Err(error) => {
                            eprintln!("解析导入文件失败: {error}");
                            return Err(1);
                        }
                    };

                    match import_zone_persistence(&settings, &bundle) {
                        Ok(()) => Err(0),
                        Err(error) => {
                            eprintln!("导入 zones 失败: {error}");
                            Err(1)
                        }
                    }
                }
                Some(other) => {
                    eprintln!("未知导入目标: {other}");
                    Err(2)
                }
                None => {
                    let kind: serde_json::Value = match serde_json::from_str(&raw) {
                        Ok(value) => value,
                        Err(error) => {
                            eprintln!("解析导入文件失败: {error}");
                            return Err(1);
                        }
                    };
                    let Some(kind) = kind.get("kind").and_then(|value| value.as_str()) else {
                        eprintln!("解析导入文件失败: 缺少 kind 字段");
                        return Err(1);
                    };

                    match kind {
                        "zones_export_v1" => {
                            let settings = PersistenceSettings::default();
                            if let Err(error) =
                                bootstrap_sqlite(settings.db_path(), "cli-import-zones")
                            {
                                eprintln!("初始化导入数据库失败: {error}");
                                return Err(1);
                            }
                            let bundle: ZoneExportBundle = match serde_json::from_str(&raw) {
                                Ok(bundle) => bundle,
                                Err(error) => {
                                    eprintln!("解析导入文件失败: {error}");
                                    return Err(1);
                                }
                            };
                            match import_zone_persistence(&settings, &bundle) {
                                Ok(()) => Err(0),
                                Err(error) => {
                                    eprintln!("导入 zones 失败: {error}");
                                    Err(1)
                                }
                            }
                        }
                        "player_export_v1" => {
                            let persistence = PlayerStatePersistence::default();
                            if let Err(error) =
                                bootstrap_sqlite(persistence.db_path(), "cli-import-player")
                            {
                                eprintln!("初始化导入数据库失败: {error}");
                                return Err(1);
                            }
                            let bundle: PlayerExportBundle = match serde_json::from_str(&raw) {
                                Ok(bundle) => bundle,
                                Err(error) => {
                                    eprintln!("解析导入文件失败: {error}");
                                    return Err(1);
                                }
                            };
                            match import_player_bundle(&persistence, &bundle) {
                                Ok(()) => Err(0),
                                Err(error) => {
                                    eprintln!("导入 player 失败: {error}");
                                    Err(1)
                                }
                            }
                        }
                        other => {
                            eprintln!("未知导入 kind: {other}");
                            Err(2)
                        }
                    }
                }
            }
        }
        _ => Ok(()),
    }
}

fn cli_dev_mode_enabled() -> bool {
    matches!(
        std::env::var("BONG_DEV_MODE").ok().as_deref(),
        Some("1") | Some("true") | Some("TRUE") | Some("yes") | Some("YES")
    )
}

#[cfg(test)]
mod cli_tests {
    use std::ffi::OsString;
    use std::sync::{Mutex, MutexGuard};

    use super::{cli_dev_mode_enabled, run_cli};

    static CLI_ENV_MUTEX: Mutex<()> = Mutex::new(());

    struct ScopedEnvVar {
        key: &'static str,
        previous: Option<OsString>,
        _lock: MutexGuard<'static, ()>,
    }

    impl ScopedEnvVar {
        fn set(key: &'static str, value: Option<&str>) -> Self {
            let lock = CLI_ENV_MUTEX
                .lock()
                .expect("cli env mutex should not be poisoned");
            let previous = std::env::var_os(key);
            if let Some(value) = value {
                std::env::set_var(key, value);
            } else {
                std::env::remove_var(key);
            }
            Self {
                key,
                previous,
                _lock: lock,
            }
        }
    }

    impl Drop for ScopedEnvVar {
        fn drop(&mut self) {
            if let Some(previous) = self.previous.take() {
                std::env::set_var(self.key, previous);
            } else {
                std::env::remove_var(self.key);
            }
        }
    }

    #[test]
    fn export_zones_cli_rejects_unknown_export_target() {
        let result = run_cli(
            [
                "bong-server".to_string(),
                "export".to_string(),
                "player".to_string(),
            ]
            .into_iter(),
        );
        assert_eq!(result, Err(2));
    }

    #[test]
    fn export_player_cli_requires_name_argument() {
        let result = run_cli(
            [
                "bong-server".to_string(),
                "export".to_string(),
                "--player".to_string(),
            ]
            .into_iter(),
        );
        assert_eq!(result, Err(2));
    }

    #[test]
    fn export_player_cli_rejects_extra_arguments() {
        let result = run_cli(
            [
                "bong-server".to_string(),
                "export".to_string(),
                "--player".to_string(),
                "Azure".to_string(),
                "extra".to_string(),
            ]
            .into_iter(),
        );
        assert_eq!(result, Err(2));
    }

    #[test]
    fn export_zones_cli_rejects_missing_target() {
        let result = run_cli(["bong-server".to_string(), "export".to_string()].into_iter());
        assert_eq!(result, Err(2));
    }

    #[test]
    fn import_zones_cli_requires_dev_mode() {
        let _guard = ScopedEnvVar::set("BONG_DEV_MODE", None);
        let result = run_cli(
            [
                "bong-server".to_string(),
                "import".to_string(),
                "zones".to_string(),
                "--file".to_string(),
                "/tmp/zones.json".to_string(),
            ]
            .into_iter(),
        );
        assert_eq!(result, Err(2));
    }

    #[test]
    fn import_zones_cli_rejects_missing_file_argument() {
        let _guard = ScopedEnvVar::set("BONG_DEV_MODE", Some("1"));
        let result = run_cli(
            [
                "bong-server".to_string(),
                "import".to_string(),
                "zones".to_string(),
            ]
            .into_iter(),
        );
        assert_eq!(result, Err(2));
    }

    #[test]
    fn import_cli_accepts_kind_routed_player_bundle_path() {
        let _guard = ScopedEnvVar::set("BONG_DEV_MODE", Some("1"));
        let result = run_cli(
            [
                "bong-server".to_string(),
                "import".to_string(),
                "--file".to_string(),
                "/tmp/player-export.json".to_string(),
            ]
            .into_iter(),
        );
        assert!(matches!(result, Err(1) | Err(2)));
    }

    #[test]
    fn cli_dev_mode_enabled_accepts_common_truthy_values() {
        let _guard = ScopedEnvVar::set("BONG_DEV_MODE", Some("true"));
        assert!(cli_dev_mode_enabled());
    }
}
