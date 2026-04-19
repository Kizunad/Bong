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
use persistence::{bootstrap_sqlite, export_zone_persistence, PersistenceSettings};
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
        _ => Ok(()),
    }
}

#[cfg(test)]
mod cli_tests {
    use super::run_cli;

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
}
