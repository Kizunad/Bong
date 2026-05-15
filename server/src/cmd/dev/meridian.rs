use valence::command::graph::CommandGraphBuilder;
use valence::command::handler::CommandResultEvent;
use valence::command::parsers::{CommandArg, CommandArgParseError, ParseInput};
use valence::command::{AddCommand, Command};
use valence::message::SendMessage;
use valence::prelude::{App, Client, EventReader, Query, Res, Update};
use valence::protocol::packets::play::command_tree_s2c::Parser;

use crate::cultivation::components::{Cultivation, MeridianId, MeridianSystem};
use crate::cultivation::life_record::{BiographyEntry, LifeRecord};
use crate::cultivation::meridian_open::MERIDIAN_CAPACITY_ON_OPEN;
use crate::cultivation::tick::CultivationClock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MeridianArg(pub MeridianId);

impl CommandArg for MeridianArg {
    fn parse_arg(input: &mut ParseInput) -> Result<Self, CommandArgParseError> {
        let raw = String::parse_arg(input)?;
        parse_meridian_id(raw.as_str()).map(Self).ok_or_else(|| {
            CommandArgParseError::InvalidArgument {
                expected: "known meridian id".to_string(),
                got: raw,
            }
        })
    }

    fn display() -> Parser {
        String::display()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeridianCmd {
    Open { id: MeridianId },
    OpenAll,
    List,
}

impl Command for MeridianCmd {
    fn assemble_graph(graph: &mut CommandGraphBuilder<Self>) {
        let meridian = graph.root().literal("meridian").id();

        graph
            .at(meridian)
            .literal("open")
            .argument("id")
            .with_parser::<MeridianArg>()
            .with_executable(|input| MeridianCmd::Open {
                id: MeridianArg::parse_arg(input)
                    .expect("brigadier should pre-validate meridian id")
                    .0,
            });

        graph
            .at(meridian)
            .literal("open_all")
            .with_executable(|_| MeridianCmd::OpenAll);

        graph
            .at(meridian)
            .literal("list")
            .with_executable(|_| MeridianCmd::List);
    }
}

pub fn register(app: &mut App) {
    app.add_command::<MeridianCmd>()
        .add_systems(Update, handle_meridian);
}

pub fn handle_meridian(
    mut events: EventReader<CommandResultEvent<MeridianCmd>>,
    clock: Option<Res<CultivationClock>>,
    mut players: Query<(
        &mut Cultivation,
        &mut MeridianSystem,
        Option<&mut LifeRecord>,
        &mut Client,
    )>,
) {
    let now = clock.as_deref().map(|clock| clock.tick).unwrap_or_default();
    for event in events.read() {
        let Ok((mut cultivation, mut meridians, mut life, mut client)) =
            players.get_mut(event.executor)
        else {
            continue;
        };

        match event.result {
            MeridianCmd::Open { id } => {
                let opened = force_open_meridian(
                    &mut cultivation,
                    &mut meridians,
                    life.as_deref_mut(),
                    id,
                    now,
                );
                tracing::warn!(
                    "[dev-cmd] bypass worldview rule: force open meridian {:?}",
                    id
                );
                if opened {
                    client
                        .send_chat_message(format!("[dev] opened meridian {}", meridian_label(id)));
                } else {
                    client.send_chat_message(format!(
                        "[dev] meridian {} already open",
                        meridian_label(id)
                    ));
                }
            }
            MeridianCmd::OpenAll => {
                let mut opened = 0usize;
                for id in MeridianId::ALL {
                    if force_open_meridian(
                        &mut cultivation,
                        &mut meridians,
                        life.as_deref_mut(),
                        id,
                        now,
                    ) {
                        opened += 1;
                    }
                }
                tracing::warn!("[dev-cmd] bypass worldview rule: force open all meridians");
                client.send_chat_message(format!(
                    "[dev] opened {opened} meridian(s); total opened={}; realm remains {:?} (open_all does not auto-breakthrough)",
                    meridians.opened_count(),
                    cultivation.realm
                ));
            }
            MeridianCmd::List => {
                let opened = meridians
                    .iter()
                    .filter(|meridian| meridian.opened)
                    .map(|meridian| {
                        format!(
                            "{} progress={:.2} cap={:.1}",
                            meridian_label(meridian.id),
                            meridian.open_progress,
                            meridian.flow_capacity
                        )
                    })
                    .collect::<Vec<_>>();
                let body = if opened.is_empty() {
                    "none".to_string()
                } else {
                    opened.join(", ")
                };
                client.send_chat_message(format!("[dev] opened meridians: {body}"));
            }
        }
    }
}

pub fn force_open_meridian(
    cultivation: &mut Cultivation,
    meridians: &mut MeridianSystem,
    life: Option<&mut LifeRecord>,
    id: MeridianId,
    tick: u64,
) -> bool {
    if meridians.get(id).opened {
        return false;
    }

    let meridian = meridians.get_mut(id);
    meridian.opened = true;
    meridian.opened_at = tick;
    meridian.open_progress = 1.0;
    meridian.flow_capacity = meridian.flow_capacity.max(MERIDIAN_CAPACITY_ON_OPEN);
    cultivation.qi_max += MERIDIAN_CAPACITY_ON_OPEN;

    if let Some(life) = life {
        life.push(BiographyEntry::MeridianOpened { id, tick });
        if life.spirit_root_first.is_none() {
            life.spirit_root_first = Some(id);
        }
    }
    true
}

pub fn parse_meridian_id(raw: &str) -> Option<MeridianId> {
    let normalized = raw.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "lung" | "fei" => Some(MeridianId::Lung),
        "large_intestine" | "largeintestine" | "li" | "dachang" => Some(MeridianId::LargeIntestine),
        "stomach" | "wei" => Some(MeridianId::Stomach),
        "spleen" | "pi" => Some(MeridianId::Spleen),
        "heart" | "xin" => Some(MeridianId::Heart),
        "small_intestine" | "smallintestine" | "si" | "xiaochang" => {
            Some(MeridianId::SmallIntestine)
        }
        "bladder" | "pangguang" => Some(MeridianId::Bladder),
        "kidney" | "shen" => Some(MeridianId::Kidney),
        "pericardium" | "xinbao" => Some(MeridianId::Pericardium),
        "triple_energizer" | "tripleenergizer" | "sanjiao" | "te" => {
            Some(MeridianId::TripleEnergizer)
        }
        "gallbladder" | "gall_bladder" | "dan" => Some(MeridianId::Gallbladder),
        "liver" | "gan" => Some(MeridianId::Liver),
        "ren" | "renmai" => Some(MeridianId::Ren),
        "du" | "dumai" => Some(MeridianId::Du),
        "chong" | "chongmai" => Some(MeridianId::Chong),
        "dai" | "daimai" => Some(MeridianId::Dai),
        "yinqiao" | "yin_qiao" => Some(MeridianId::YinQiao),
        "yangqiao" | "yang_qiao" => Some(MeridianId::YangQiao),
        "yinwei" | "yin_wei" => Some(MeridianId::YinWei),
        "yangwei" | "yang_wei" => Some(MeridianId::YangWei),
        _ => None,
    }
}

pub fn meridian_label(id: MeridianId) -> &'static str {
    match id {
        MeridianId::Lung => "lung",
        MeridianId::LargeIntestine => "large_intestine",
        MeridianId::Stomach => "stomach",
        MeridianId::Spleen => "spleen",
        MeridianId::Heart => "heart",
        MeridianId::SmallIntestine => "small_intestine",
        MeridianId::Bladder => "bladder",
        MeridianId::Kidney => "kidney",
        MeridianId::Pericardium => "pericardium",
        MeridianId::TripleEnergizer => "triple_energizer",
        MeridianId::Gallbladder => "gallbladder",
        MeridianId::Liver => "liver",
        MeridianId::Ren => "ren",
        MeridianId::Du => "du",
        MeridianId::Chong => "chong",
        MeridianId::Dai => "dai",
        MeridianId::YinQiao => "yin_qiao",
        MeridianId::YangQiao => "yang_qiao",
        MeridianId::YinWei => "yin_wei",
        MeridianId::YangWei => "yang_wei",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cmd::dev::test_support::{run_update, spawn_test_client};
    use valence::prelude::Events;

    fn setup_app() -> App {
        let mut app = App::new();
        app.insert_resource(CultivationClock { tick: 42 });
        app.add_event::<CommandResultEvent<MeridianCmd>>();
        app.add_systems(Update, handle_meridian);
        app
    }

    fn send(app: &mut App, player: valence::prelude::Entity, result: MeridianCmd) {
        app.world_mut()
            .resource_mut::<Events<CommandResultEvent<MeridianCmd>>>()
            .send(CommandResultEvent {
                result,
                executor: player,
                modifiers: Default::default(),
            });
    }

    fn spawn_cultivator(app: &mut App) -> valence::prelude::Entity {
        let player = spawn_test_client(app, "Alice", [0.0, 0.0, 0.0]);
        app.world_mut().entity_mut(player).insert((
            Cultivation::default(),
            MeridianSystem::default(),
            LifeRecord::default(),
        ));
        player
    }

    #[test]
    fn parse_meridian_id_covers_all_canonical_ids_and_aliases() {
        for id in MeridianId::ALL {
            assert_eq!(parse_meridian_id(meridian_label(id)), Some(id));
            assert_eq!(
                parse_meridian_id(&meridian_label(id).to_ascii_uppercase()),
                Some(id)
            );
        }
        assert_eq!(
            parse_meridian_id("large_intestine"),
            Some(MeridianId::LargeIntestine)
        );
        assert_eq!(parse_meridian_id("ren"), Some(MeridianId::Ren));
        assert_eq!(parse_meridian_id("du"), Some(MeridianId::Du));
        assert_eq!(parse_meridian_id("missing"), None);
    }

    #[test]
    fn meridian_open_mutates_state_once_and_writes_life_record() {
        let mut app = setup_app();
        let player = spawn_cultivator(&mut app);

        send(
            &mut app,
            player,
            MeridianCmd::Open {
                id: MeridianId::Lung,
            },
        );
        run_update(&mut app);
        send(
            &mut app,
            player,
            MeridianCmd::Open {
                id: MeridianId::Lung,
            },
        );
        run_update(&mut app);

        let cultivation = app.world().get::<Cultivation>(player).unwrap();
        let meridians = app.world().get::<MeridianSystem>(player).unwrap();
        let life = app.world().get::<LifeRecord>(player).unwrap();
        assert!(meridians.get(MeridianId::Lung).opened);
        assert_eq!(meridians.get(MeridianId::Lung).open_progress, 1.0);
        assert_eq!(meridians.get(MeridianId::Lung).opened_at, 42);
        assert_eq!(cultivation.qi_max, 20.0);
        assert_eq!(life.biography.len(), 1);
        assert_eq!(life.spirit_root_first, Some(MeridianId::Lung));
    }

    #[test]
    fn meridian_open_all_opens_twenty_channels_and_is_idempotent() {
        let mut app = setup_app();
        let player = spawn_cultivator(&mut app);
        let pre_realm = app.world().get::<Cultivation>(player).unwrap().realm;

        send(&mut app, player, MeridianCmd::OpenAll);
        run_update(&mut app);
        send(&mut app, player, MeridianCmd::OpenAll);
        run_update(&mut app);

        let cultivation = app.world().get::<Cultivation>(player).unwrap();
        let meridians = app.world().get::<MeridianSystem>(player).unwrap();
        let life = app.world().get::<LifeRecord>(player).unwrap();
        assert_eq!(meridians.opened_count(), 20);
        assert_eq!(
            cultivation.realm, pre_realm,
            "expected /meridian open_all to keep realm unchanged, actual realm={:?}",
            cultivation.realm
        );
        assert_eq!(cultivation.qi_max, 210.0);
        assert_eq!(life.biography.len(), 20);
    }

    #[test]
    fn meridian_open_does_not_overwrite_existing_spirit_root_first() {
        let mut app = setup_app();
        let player = spawn_cultivator(&mut app);

        send(
            &mut app,
            player,
            MeridianCmd::Open {
                id: MeridianId::Lung,
            },
        );
        run_update(&mut app);
        send(
            &mut app,
            player,
            MeridianCmd::Open {
                id: MeridianId::Heart,
            },
        );
        run_update(&mut app);

        let life = app.world().get::<LifeRecord>(player).unwrap();
        assert_eq!(life.biography.len(), 2);
        assert_eq!(life.spirit_root_first, Some(MeridianId::Lung));
    }

    #[test]
    fn meridian_list_does_not_mutate_state() {
        let mut app = setup_app();
        let player = spawn_cultivator(&mut app);

        send(&mut app, player, MeridianCmd::List);
        run_update(&mut app);

        assert_eq!(
            app.world()
                .get::<MeridianSystem>(player)
                .unwrap()
                .opened_count(),
            0
        );
    }
}
