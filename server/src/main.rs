mod combat;
#[allow(dead_code)]
mod cultivation;
mod inventory;
mod network;
mod npc;
mod persistence;
mod player;
#[allow(dead_code)]
mod schema;
mod world;

use crossbeam_channel::unbounded;
use network::agent_bridge::{
    spawn_mock_bridge_daemon, AgentCommand, GameEvent, NetworkBridgeResource,
};
use persistence::{
    bootstrap_sqlite, export_zone_persistence, import_zone_persistence, PersistenceSettings,
    ZoneExportBundle,
};
use valence::log::LogPlugin;
use valence::prelude::*;

fn init_tracing() {
    let _ = tracing_subscriber::fmt().with_target(false).try_init();
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
    inventory::register(&mut app);
    cultivation::register(&mut app);
    combat::register(&mut app);
    npc::register(&mut app);
    network::register(&mut app);
    persistence::register(&mut app);

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
                eprintln!("用法: bong-server export zones");
                return Err(2);
            };
            if args.next().is_some() {
                eprintln!("用法: bong-server export zones");
                return Err(2);
            }

            match target.as_str() {
                "zones" => {
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

            let Some(target) = args.next() else {
                eprintln!("用法: bong-server import zones --file <path>");
                return Err(2);
            };
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

            match target.as_str() {
                "zones" => {
                    let settings = PersistenceSettings::default();
                    if let Err(error) = bootstrap_sqlite(settings.db_path(), "cli-import-zones") {
                        eprintln!("初始化导入数据库失败: {error}");
                        return Err(1);
                    }

                    let raw = match std::fs::read_to_string(&path) {
                        Ok(raw) => raw,
                        Err(error) => {
                            eprintln!("读取导入文件失败: {error}");
                            return Err(1);
                        }
                    };
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
                _ => {
                    eprintln!("未知导入目标: {target}");
                    Err(2)
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

    use super::{cli_dev_mode_enabled, run_cli};

    struct ScopedEnvVar {
        key: &'static str,
        previous: Option<OsString>,
    }

    impl ScopedEnvVar {
        fn set(key: &'static str, value: Option<&str>) -> Self {
            let previous = std::env::var_os(key);
            if let Some(value) = value {
                std::env::set_var(key, value);
            } else {
                std::env::remove_var(key);
            }
            Self { key, previous }
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
    fn cli_dev_mode_enabled_accepts_common_truthy_values() {
        let _guard = ScopedEnvVar::set("BONG_DEV_MODE", Some("true"));
        assert!(cli_dev_mode_enabled());
    }
}
