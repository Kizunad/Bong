use valence::command::graph::CommandGraphBuilder;
use valence::command::handler::CommandResultEvent;
use valence::command::parsers::CommandArg;
use valence::command::{AddCommand, Command};
use valence::message::SendMessage;
use valence::prelude::{
    App, Client, Entity, EventReader, EventWriter, Query, Res, Update, Username,
};

use crate::cultivation::tick::CultivationClock;
use crate::social::events::SparringInviteRequest;

/// dev-only `/sparring invite <username>`：从执行者向指定用户名玩家发起切磋邀请。
///
/// 切磋邀请的生产侧是 S2S（agent_cmd / NPC 侧）专属，bot e2e 无从触达——协议级场景
/// 因此只能回执伪造的 invite_id、从未消费过服务器真实下发的 SparringInvite payload。
/// 本命令补上 dev 通道：把 `SparringInviteRequest` 送进同一事件流，由
/// `dispatch_sparring_invites` 走标准路径向 target 下发 SparringInvite。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SparringCmd {
    Invite { target: String },
}

impl Command for SparringCmd {
    fn assemble_graph(graph: &mut CommandGraphBuilder<Self>) {
        graph
            .root()
            .literal("sparring")
            .literal("invite")
            .argument("target")
            .with_parser::<String>()
            .with_executable(|input| SparringCmd::Invite {
                target: String::parse_arg(input).unwrap(),
            });
    }
}

pub fn register(app: &mut App) {
    // SparringInviteRequest 属 social 插件；cmd-only 测试 App 不装 social，dev 命令
    // 的事件必须自注册才能让 handle_sparring_invite 在 test_command_app 里跑起来
    // （bevy 0.14 add_event 幂等，生产侧 social 已注册时为 no-op）。
    app.add_event::<SparringInviteRequest>()
        .add_command::<SparringCmd>()
        .add_systems(Update, handle_sparring_invite);
}

pub fn handle_sparring_invite(
    mut events: EventReader<CommandResultEvent<SparringCmd>>,
    clock: Option<Res<CultivationClock>>,
    mut invites: EventWriter<SparringInviteRequest>,
    usernames: Query<(Entity, &Username)>,
    mut clients: Query<&mut Client>,
) {
    let tick = clock.as_deref().map(|clock| clock.tick).unwrap_or_default();
    for event in events.read() {
        let SparringCmd::Invite { target } = &event.result;
        let Some((target_entity, _)) = usernames
            .iter()
            .find(|(_, username)| username.0 == target.as_str())
        else {
            if let Ok(mut client) = clients.get_mut(event.executor) {
                client.send_chat_message(format!(
                    "[dev] sparring invite failed: no player named `{target}`"
                ));
            }
            continue;
        };
        if target_entity == event.executor {
            if let Ok(mut client) = clients.get_mut(event.executor) {
                client.send_chat_message("[dev] sparring invite failed: cannot invite self");
            }
            continue;
        }
        invites.send(SparringInviteRequest {
            initiator: event.executor,
            target: target_entity,
            terms: "点到为止".to_string(),
            tick,
        });
        if let Ok(mut client) = clients.get_mut(event.executor) {
            client.send_chat_message(format!("[dev] sparring invite → {target}"));
        }
    }
}
