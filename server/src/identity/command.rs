//! `/identity` slash command（plan-identity-v1 P1）。
//!
//! 子命令矩阵：
//!
//! | 命令 | 副作用 | precondition |
//! |---|---|---|
//! | `/identity list` | 仅打印玩家所有 identity（active / frozen 标记 + reputation_score）| 无 |
//! | `/identity new <display_name>` | 创建新 identity 并切到该 identity | 灵龛内 + 切换冷却已过 |
//! | `/identity switch <id>` | 切到 frozen identity（unfreeze）/ 当前 active identity 冻结 | 灵龛内 + 切换冷却已过 |
//! | `/identity rename <new_name>` | 改当前 active identity 的 display_name | 灵龛内（**无切换冷却**）|
//!
//! 业务逻辑 (`apply_*`) 与系统胶水（[`handle_identity_command`]）分离——纯函数易于
//! 用饱和单测把所有边界 / 错误分支锁住。

use valence::command::graph::CommandGraphBuilder;
use valence::command::handler::CommandResultEvent;
use valence::command::parsers::{CommandArg, ParseInput};
use valence::command::{AddCommand, Command};
use valence::message::SendMessage;
use valence::prelude::{
    App, Client, DVec3, Entity, EventReader, EventWriter, Position, Query, Res, Update, With,
};

use super::events::{IdentityCreatedEvent, IdentitySwitchedEvent};
use super::precondition::{check_within_own_niche, NichePreconditionError};
use super::{reputation_score, IdentityId, IdentityProfile, PlayerIdentities};
use crate::combat::components::Lifecycle;
use crate::npc::movement::GameTick;
use crate::persistence::identity as identity_db;
use crate::persistence::PersistenceSettings;
use crate::social::SpiritNicheRegistry;

/// 单条 chat 消息（handler 把它送到 client）。
const COOLDOWN_REJECTION_MSG: &str = "身份未稳，候片刻再易容";

/// 命令解析结果——valence_command 把客户端输入解析成它，然后由 [`handle_identity_command`]
/// 系统消费并改 [`PlayerIdentities`]。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdentityCmd {
    List,
    New { display_name: String },
    Switch { id: u32 },
    Rename { new_name: String },
}

impl Command for IdentityCmd {
    fn assemble_graph(graph: &mut CommandGraphBuilder<Self>) {
        let identity = graph.root().literal("identity").id();

        graph
            .at(identity)
            .literal("list")
            .with_executable(|_| IdentityCmd::List);

        graph
            .at(identity)
            .literal("new")
            .argument("display_name")
            .with_parser::<String>()
            .with_executable(|input: &mut ParseInput| IdentityCmd::New {
                display_name: String::parse_arg(input).unwrap_or_default(),
            });

        graph
            .at(identity)
            .literal("switch")
            .argument("id")
            .with_parser::<i32>()
            .with_executable(|input: &mut ParseInput| {
                let raw = i32::parse_arg(input).unwrap_or(-1);
                IdentityCmd::Switch {
                    // 负数（如 `/identity switch -1`）保持负数 → 转 u32 强制 wrap，
                    // 后续 apply_switch 通过 UnknownIdentityId 拒绝；不再静默 clamp 到 0。
                    id: i32_to_identity_id_raw(raw),
                }
            });

        graph
            .at(identity)
            .literal("rename")
            .argument("new_name")
            .with_parser::<String>()
            .with_executable(|input: &mut ParseInput| IdentityCmd::Rename {
                new_name: String::parse_arg(input).unwrap_or_default(),
            });
    }
}

/// 把 valence_command 解出来的 i32 id 转成保留语义的 u32：
/// - 0..=i32::MAX → 直接转 u32（合法 id）
/// - 负数 → 映射到一个**保证不在玩家 identity 列表里**的特殊值（u32::MAX），
///   让 apply_switch 走 `UnknownIdentityId` 拒绝路径，不再被 `.max(0)` 当成 id 0 误执行。
pub(crate) fn i32_to_identity_id_raw(raw: i32) -> u32 {
    if raw < 0 {
        u32::MAX
    } else {
        raw as u32
    }
}

/// 注册 `/identity` 命令 + handler system + 关联事件类型。
pub fn register(app: &mut App) {
    app.add_event::<IdentityCreatedEvent>()
        .add_event::<IdentitySwitchedEvent>()
        .add_command::<IdentityCmd>()
        .add_systems(Update, handle_identity_command);
}

/// list 子命令的业务输出（一组 chat lines）。
pub fn apply_list(identities: &PlayerIdentities) -> Vec<String> {
    let mut lines = Vec::with_capacity(identities.identities.len() + 1);
    lines.push(format!(
        "持有身份 {} 个（当前 active: id={}）",
        identities.identities.len(),
        identities.active_identity_id.0
    ));
    for profile in &identities.identities {
        let active_marker = if profile.id == identities.active_identity_id {
            " *"
        } else {
            ""
        };
        let frozen_marker = if profile.frozen { " [冷藏]" } else { "" };
        lines.push(format!(
            "  id={} name=\"{}\" reputation={}{}{}",
            profile.id.0,
            profile.display_name,
            reputation_score(profile),
            active_marker,
            frozen_marker,
        ));
    }
    lines
}

/// 错误码 + 玩家可读消息。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdentityCmdError {
    NotInOwnNiche,
    EmptyCharId,
    CooldownNotPassed { remaining_ticks: u64 },
    DisplayNameEmpty,
    DisplayNameTooLong,
    UnknownIdentityId(IdentityId),
    AlreadyActive(IdentityId),
}

impl IdentityCmdError {
    pub fn message(&self) -> String {
        match self {
            Self::NotInOwnNiche => NichePreconditionError::NotInOwnNiche.message().to_string(),
            Self::EmptyCharId => NichePreconditionError::EmptyCharId.message().to_string(),
            Self::CooldownNotPassed { .. } => COOLDOWN_REJECTION_MSG.to_string(),
            Self::DisplayNameEmpty => "身份名不可为空".to_string(),
            Self::DisplayNameTooLong => "身份名过长（最多 32 字符）".to_string(),
            Self::UnknownIdentityId(id) => format!("未知身份 id={}", id.0),
            Self::AlreadyActive(id) => format!("身份 id={} 已是 active", id.0),
        }
    }
}

const MAX_DISPLAY_NAME_LEN: usize = 32;

/// new 子命令成功的输出：新建的 identity + 被冻结的旧 active id。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewOutcome {
    pub created_id: IdentityId,
    pub display_name: String,
    pub previous_active: IdentityId,
}

pub fn apply_new(
    identities: &mut PlayerIdentities,
    display_name: &str,
    char_id: &str,
    pos: DVec3,
    now_tick: u64,
    niche_registry: &SpiritNicheRegistry,
) -> Result<NewOutcome, IdentityCmdError> {
    require_within_own_niche(char_id, pos, niche_registry)?;
    require_cooldown_passed(identities, now_tick)?;
    let trimmed = display_name.trim();
    if trimmed.is_empty() {
        return Err(IdentityCmdError::DisplayNameEmpty);
    }
    if trimmed.chars().count() > MAX_DISPLAY_NAME_LEN {
        return Err(IdentityCmdError::DisplayNameTooLong);
    }

    let new_id = identities.next_id();
    let previous_active = identities.active_identity_id;
    if let Some(prev) = identities.active_mut() {
        prev.frozen = true;
    }
    let mut profile = IdentityProfile::new(new_id, trimmed, now_tick);
    profile.frozen = false;
    identities.identities.push(profile);
    identities.active_identity_id = new_id;
    identities.last_switch_tick = now_tick;

    Ok(NewOutcome {
        created_id: new_id,
        display_name: trimmed.to_string(),
        previous_active,
    })
}

/// switch 子命令成功的输出：from / to id。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SwitchOutcome {
    pub from: IdentityId,
    pub to: IdentityId,
}

pub fn apply_switch(
    identities: &mut PlayerIdentities,
    target_id: IdentityId,
    char_id: &str,
    pos: DVec3,
    now_tick: u64,
    niche_registry: &SpiritNicheRegistry,
) -> Result<SwitchOutcome, IdentityCmdError> {
    require_within_own_niche(char_id, pos, niche_registry)?;
    require_cooldown_passed(identities, now_tick)?;

    if identities.get(target_id).is_none() {
        return Err(IdentityCmdError::UnknownIdentityId(target_id));
    }
    let from = identities.active_identity_id;
    if from == target_id {
        return Err(IdentityCmdError::AlreadyActive(target_id));
    }

    if let Some(prev) = identities.active_mut() {
        prev.frozen = true;
    }
    if let Some(target) = identities.get_mut(target_id) {
        target.frozen = false;
    }
    identities.active_identity_id = target_id;
    identities.last_switch_tick = now_tick;

    Ok(SwitchOutcome {
        from,
        to: target_id,
    })
}

/// rename 子命令成功的输出。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenameOutcome {
    pub identity_id: IdentityId,
    pub display_name: String,
}

pub fn apply_rename(
    identities: &mut PlayerIdentities,
    new_name: &str,
    char_id: &str,
    pos: DVec3,
    niche_registry: &SpiritNicheRegistry,
) -> Result<RenameOutcome, IdentityCmdError> {
    require_within_own_niche(char_id, pos, niche_registry)?;
    // rename **不消耗冷却**
    let trimmed = new_name.trim();
    if trimmed.is_empty() {
        return Err(IdentityCmdError::DisplayNameEmpty);
    }
    if trimmed.chars().count() > MAX_DISPLAY_NAME_LEN {
        return Err(IdentityCmdError::DisplayNameTooLong);
    }

    let active_id = identities.active_identity_id;
    let Some(active) = identities.active_mut() else {
        return Err(IdentityCmdError::UnknownIdentityId(active_id));
    };
    active.rename(trimmed);
    let display_name = active.display_name.clone();
    Ok(RenameOutcome {
        identity_id: active_id,
        display_name,
    })
}

fn require_within_own_niche(
    char_id: &str,
    pos: DVec3,
    niche_registry: &SpiritNicheRegistry,
) -> Result<(), IdentityCmdError> {
    match check_within_own_niche(char_id, pos, niche_registry) {
        Ok(()) => Ok(()),
        Err(NichePreconditionError::NotInOwnNiche) => Err(IdentityCmdError::NotInOwnNiche),
        Err(NichePreconditionError::EmptyCharId) => Err(IdentityCmdError::EmptyCharId),
    }
}

fn require_cooldown_passed(
    identities: &PlayerIdentities,
    now_tick: u64,
) -> Result<(), IdentityCmdError> {
    if identities.cooldown_passed(now_tick) {
        Ok(())
    } else {
        Err(IdentityCmdError::CooldownNotPassed {
            remaining_ticks: identities.cooldown_remaining(now_tick),
        })
    }
}

#[allow(clippy::too_many_arguments)]
pub fn handle_identity_command(
    mut events: EventReader<CommandResultEvent<IdentityCmd>>,
    mut clients: Query<&mut Client>,
    mut players: Query<
        (&mut PlayerIdentities, &Lifecycle, &Position),
        With<valence::prelude::Client>,
    >,
    niche_registry: Option<Res<SpiritNicheRegistry>>,
    game_tick: Option<Res<GameTick>>,
    persistence: Option<Res<PersistenceSettings>>,
    mut created_writer: EventWriter<IdentityCreatedEvent>,
    mut switched_writer: EventWriter<IdentitySwitchedEvent>,
) {
    let now_tick = game_tick.map(|t| t.0 as u64).unwrap_or(0);
    let empty_registry = SpiritNicheRegistry::default();
    let niche_registry_ref = niche_registry.as_deref().unwrap_or(&empty_registry);

    for event in events.read() {
        let executor = event.executor;
        let Ok((mut identities, lifecycle, position)) = players.get_mut(executor) else {
            continue;
        };
        let char_id = lifecycle.character_id.as_str();
        let pos = position.get();

        match event.result.clone() {
            IdentityCmd::List => {
                let lines = apply_list(&identities);
                send_lines(&mut clients, executor, &lines);
            }
            IdentityCmd::New { display_name } => {
                match apply_new(
                    &mut identities,
                    &display_name,
                    char_id,
                    pos,
                    now_tick,
                    niche_registry_ref,
                ) {
                    Ok(outcome) => {
                        created_writer.send(IdentityCreatedEvent {
                            player: executor,
                            identity_id: outcome.created_id,
                            display_name: outcome.display_name.clone(),
                            previous_active: outcome.previous_active,
                            at_tick: now_tick,
                        });
                        send_lines(
                            &mut clients,
                            executor,
                            &[format!(
                                "已新建身份 id={} name=\"{}\"，当前 active",
                                outcome.created_id.0, outcome.display_name
                            )],
                        );
                        save_identities(persistence.as_deref(), char_id, &identities);
                    }
                    Err(err) => send_error(&mut clients, executor, &err),
                }
            }
            IdentityCmd::Switch { id } => match apply_switch(
                &mut identities,
                IdentityId(id),
                char_id,
                pos,
                now_tick,
                niche_registry_ref,
            ) {
                Ok(outcome) => {
                    switched_writer.send(IdentitySwitchedEvent {
                        player: executor,
                        from: outcome.from,
                        to: outcome.to,
                        at_tick: now_tick,
                    });
                    send_lines(
                        &mut clients,
                        executor,
                        &[format!(
                            "已切换身份 from id={} to id={}",
                            outcome.from.0, outcome.to.0
                        )],
                    );
                    save_identities(persistence.as_deref(), char_id, &identities);
                }
                Err(err) => send_error(&mut clients, executor, &err),
            },
            IdentityCmd::Rename { new_name } => {
                match apply_rename(&mut identities, &new_name, char_id, pos, niche_registry_ref) {
                    Ok(outcome) => {
                        send_lines(
                            &mut clients,
                            executor,
                            &[format!(
                                "已更名 id={} → \"{}\"",
                                outcome.identity_id.0, outcome.display_name
                            )],
                        );
                        save_identities(persistence.as_deref(), char_id, &identities);
                    }
                    Err(err) => send_error(&mut clients, executor, &err),
                }
            }
        }
    }
}

fn send_lines(clients: &mut Query<&mut Client>, executor: Entity, lines: &[String]) {
    let Ok(mut client) = clients.get_mut(executor) else {
        return;
    };
    for line in lines {
        client.send_chat_message(line.clone());
    }
}

fn send_error(clients: &mut Query<&mut Client>, executor: Entity, err: &IdentityCmdError) {
    let Ok(mut client) = clients.get_mut(executor) else {
        return;
    };
    client.send_chat_message(err.message());
}

fn save_identities(
    persistence: Option<&PersistenceSettings>,
    char_id: &str,
    identities: &PlayerIdentities,
) {
    let Some(settings) = persistence else { return };
    if let Err(error) = identity_db::save_player_identities(settings, char_id, identities) {
        tracing::warn!(?error, char_id, "[bong][identity] persistence save failed");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cultivation::components::Realm;
    use crate::identity::{RevealedTag, RevealedTagKind, IDENTITY_SWITCH_COOLDOWN_TICKS};
    use crate::social::components::SpiritNiche;

    fn install_niche(registry: &mut SpiritNicheRegistry, owner: &str, pos: [i32; 3]) {
        registry.upsert(SpiritNiche {
            owner: owner.to_string(),
            pos,
            placed_at_tick: 1,
            revealed: false,
            revealed_by: None,
            guardians: Vec::new(),
        });
    }

    fn niche_at_origin(owner: &str) -> SpiritNicheRegistry {
        let mut registry = SpiritNicheRegistry::default();
        install_niche(&mut registry, owner, [0, 64, 0]);
        registry
    }

    fn pos_at_niche() -> DVec3 {
        DVec3::new(0.5, 64.5, 0.5)
    }

    fn pos_far() -> DVec3 {
        DVec3::new(1000.0, 64.0, 1000.0)
    }

    #[test]
    fn apply_list_marks_active_identity() {
        let mut pid = PlayerIdentities::with_default("kiz", 0);
        pid.identities
            .push(IdentityProfile::new(IdentityId(1), "alt", 100));
        let lines = apply_list(&pid);
        assert!(lines[0].contains("当前 active: id=0"));
        assert!(
            lines
                .iter()
                .any(|l| l.contains("id=0") && l.contains("kiz") && l.contains(" *")),
            "active marker 应该在 id=0 行；实际 {lines:?}"
        );
        assert!(
            lines
                .iter()
                .any(|l| l.contains("id=1") && l.contains("alt") && !l.contains(" *")),
            "非 active 行不应有 marker；实际 {lines:?}"
        );
    }

    #[test]
    fn apply_list_shows_frozen_marker() {
        let mut pid = PlayerIdentities::with_default("kiz", 0);
        pid.identities
            .push(IdentityProfile::new(IdentityId(1), "alt", 100));
        pid.identities[1].frozen = true;
        let lines = apply_list(&pid);
        assert!(
            lines
                .iter()
                .any(|l| l.contains("id=1") && l.contains("[冷藏]")),
            "frozen identity 应有 [冷藏] 标记；实际 {lines:?}"
        );
    }

    #[test]
    fn apply_list_includes_reputation_score() {
        let mut pid = PlayerIdentities::with_default("kiz", 0);
        pid.identities[0].renown.fame = 60;
        pid.identities[0].renown.notoriety = 10;
        let lines = apply_list(&pid);
        // 60 - 10 = 50
        assert!(
            lines.iter().any(|l| l.contains("reputation=50")),
            "应显示 reputation=50；实际 {lines:?}"
        );
    }

    #[test]
    fn apply_new_creates_identity_and_sets_active() {
        let mut pid = PlayerIdentities::with_default("kiz", 0);
        let registry = niche_at_origin("offline:kiz");
        let outcome = apply_new(
            &mut pid,
            "alt",
            "offline:kiz",
            pos_at_niche(),
            100,
            &registry,
        )
        .unwrap();
        assert_eq!(outcome.created_id, IdentityId(1));
        assert_eq!(outcome.display_name, "alt");
        assert_eq!(outcome.previous_active, IdentityId(0));
        assert_eq!(pid.active_identity_id, IdentityId(1));
        assert!(pid.identities[0].frozen, "旧 active 应被冻结");
        assert!(!pid.identities[1].frozen, "新 identity 不应冻结");
        assert_eq!(pid.last_switch_tick, 100);
    }

    #[test]
    fn apply_new_outside_niche_rejected() {
        let mut pid = PlayerIdentities::with_default("kiz", 0);
        let registry = niche_at_origin("offline:kiz");
        let err = apply_new(&mut pid, "alt", "offline:kiz", pos_far(), 100, &registry).unwrap_err();
        assert_eq!(err, IdentityCmdError::NotInOwnNiche);
        // 失败时不应改 state
        assert_eq!(pid.active_identity_id, IdentityId(0));
        assert_eq!(pid.identities.len(), 1);
    }

    #[test]
    fn apply_new_empty_name_rejected() {
        let mut pid = PlayerIdentities::with_default("kiz", 0);
        let registry = niche_at_origin("offline:kiz");
        let err = apply_new(
            &mut pid,
            "   ",
            "offline:kiz",
            pos_at_niche(),
            100,
            &registry,
        )
        .unwrap_err();
        assert_eq!(err, IdentityCmdError::DisplayNameEmpty);
    }

    #[test]
    fn apply_new_too_long_name_rejected() {
        let mut pid = PlayerIdentities::with_default("kiz", 0);
        let registry = niche_at_origin("offline:kiz");
        let long = "a".repeat(33);
        let err = apply_new(
            &mut pid,
            &long,
            "offline:kiz",
            pos_at_niche(),
            100,
            &registry,
        )
        .unwrap_err();
        assert_eq!(err, IdentityCmdError::DisplayNameTooLong);
    }

    #[test]
    fn apply_new_within_cooldown_rejected() {
        let mut pid = PlayerIdentities::with_default("kiz", 0);
        pid.last_switch_tick = 100;
        let registry = niche_at_origin("offline:kiz");
        // now_tick = 200, last_switch_tick + 24000 = 24100 → 200 < 24100 → 冷却中
        let err = apply_new(
            &mut pid,
            "alt",
            "offline:kiz",
            pos_at_niche(),
            200,
            &registry,
        )
        .unwrap_err();
        assert!(matches!(
            err,
            IdentityCmdError::CooldownNotPassed { remaining_ticks } if remaining_ticks == 23_900
        ));
    }

    #[test]
    fn apply_switch_freezes_old_unfreezes_new() {
        let mut pid = PlayerIdentities::with_default("kiz", 0);
        pid.identities
            .push(IdentityProfile::new(IdentityId(1), "alt", 0));
        pid.identities[1].frozen = true;
        let registry = niche_at_origin("offline:kiz");

        let outcome = apply_switch(
            &mut pid,
            IdentityId(1),
            "offline:kiz",
            pos_at_niche(),
            100,
            &registry,
        )
        .unwrap();
        assert_eq!(outcome.from, IdentityId(0));
        assert_eq!(outcome.to, IdentityId(1));
        assert_eq!(pid.active_identity_id, IdentityId(1));
        assert!(pid.identities[0].frozen, "原 active 必须冻结");
        assert!(!pid.identities[1].frozen, "切到的 identity 必须 unfreeze");
        assert_eq!(pid.last_switch_tick, 100);
    }

    #[test]
    fn apply_switch_within_own_niche_succeeds() {
        let mut pid = PlayerIdentities::with_default("kiz", 0);
        pid.identities
            .push(IdentityProfile::new(IdentityId(1), "alt", 0));
        let registry = niche_at_origin("offline:kiz");
        assert!(apply_switch(
            &mut pid,
            IdentityId(1),
            "offline:kiz",
            pos_at_niche(),
            100,
            &registry
        )
        .is_ok());
    }

    #[test]
    fn apply_switch_outside_niche_rejected() {
        let mut pid = PlayerIdentities::with_default("kiz", 0);
        pid.identities
            .push(IdentityProfile::new(IdentityId(1), "alt", 0));
        let registry = niche_at_origin("offline:kiz");
        let err = apply_switch(
            &mut pid,
            IdentityId(1),
            "offline:kiz",
            pos_far(),
            100,
            &registry,
        )
        .unwrap_err();
        assert_eq!(err, IdentityCmdError::NotInOwnNiche);
        assert_eq!(pid.active_identity_id, IdentityId(0), "失败不应改 state");
    }

    #[test]
    fn apply_switch_within_cooldown_rejected() {
        let mut pid = PlayerIdentities::with_default("kiz", 0);
        pid.identities
            .push(IdentityProfile::new(IdentityId(1), "alt", 0));
        pid.last_switch_tick = 1000;
        let registry = niche_at_origin("offline:kiz");
        let err = apply_switch(
            &mut pid,
            IdentityId(1),
            "offline:kiz",
            pos_at_niche(),
            5000,
            &registry,
        )
        .unwrap_err();
        assert!(matches!(
            err,
            IdentityCmdError::CooldownNotPassed { remaining_ticks } if remaining_ticks == 20_000
        ));
    }

    #[test]
    fn apply_switch_unknown_id_rejected() {
        let mut pid = PlayerIdentities::with_default("kiz", 0);
        let registry = niche_at_origin("offline:kiz");
        let err = apply_switch(
            &mut pid,
            IdentityId(99),
            "offline:kiz",
            pos_at_niche(),
            100,
            &registry,
        )
        .unwrap_err();
        assert_eq!(err, IdentityCmdError::UnknownIdentityId(IdentityId(99)));
    }

    #[test]
    fn apply_switch_to_already_active_rejected() {
        let mut pid = PlayerIdentities::with_default("kiz", 0);
        let registry = niche_at_origin("offline:kiz");
        let err = apply_switch(
            &mut pid,
            IdentityId(0),
            "offline:kiz",
            pos_at_niche(),
            100,
            &registry,
        )
        .unwrap_err();
        assert_eq!(err, IdentityCmdError::AlreadyActive(IdentityId(0)));
    }

    #[test]
    fn apply_rename_does_not_consume_cooldown() {
        let mut pid = PlayerIdentities::with_default("kiz", 0);
        pid.last_switch_tick = 1000;
        let registry = niche_at_origin("offline:kiz");

        let outcome = apply_rename(&mut pid, "新名字", "offline:kiz", pos_at_niche(), &registry)
            .expect("rename 在冷却内仍允许");
        assert_eq!(outcome.identity_id, IdentityId(0));
        assert_eq!(outcome.display_name, "新名字");
        assert_eq!(pid.identities[0].display_name, "新名字");
        // last_switch_tick 不变
        assert_eq!(pid.last_switch_tick, 1000);
    }

    #[test]
    fn apply_rename_outside_niche_rejected() {
        let mut pid = PlayerIdentities::with_default("kiz", 0);
        let registry = niche_at_origin("offline:kiz");
        let err =
            apply_rename(&mut pid, "新名字", "offline:kiz", pos_far(), &registry).unwrap_err();
        assert_eq!(err, IdentityCmdError::NotInOwnNiche);
        assert_eq!(pid.identities[0].display_name, "kiz", "失败不应改 name");
    }

    #[test]
    fn apply_rename_empty_name_rejected() {
        let mut pid = PlayerIdentities::with_default("kiz", 0);
        let registry = niche_at_origin("offline:kiz");
        let err = apply_rename(&mut pid, "", "offline:kiz", pos_at_niche(), &registry).unwrap_err();
        assert_eq!(err, IdentityCmdError::DisplayNameEmpty);
    }

    #[test]
    fn unfreezing_old_identity_restores_renown_state() {
        // worldview "待复用" 语义：切到新 identity 后 frozen=true，但 renown 不动；
        // 切回去 unfreeze 后 renown 应保持完整（包括 fame/notoriety/RevealedTag）。
        let mut pid = PlayerIdentities::with_default("kiz", 0);
        pid.identities[0].renown.fame = 80;
        pid.identities[0].renown.notoriety = 20;
        pid.identities[0].revealed_tags.push(RevealedTag {
            kind: RevealedTagKind::DuguRevealed,
            witnessed_at_tick: 50,
            witness_realm: Realm::Spirit,
            permanent: true,
        });
        pid.identities
            .push(IdentityProfile::new(IdentityId(1), "alt", 100));
        let registry = niche_at_origin("offline:kiz");

        // 切到 alt → 旧 dugu identity 冻结
        apply_switch(
            &mut pid,
            IdentityId(1),
            "offline:kiz",
            pos_at_niche(),
            100,
            &registry,
        )
        .unwrap();
        assert!(pid.identities[0].frozen);
        // 冻结后 dugu identity 的 renown / tag 应该完整保留
        assert_eq!(pid.identities[0].renown.fame, 80);
        assert_eq!(pid.identities[0].renown.notoriety, 20);
        assert!(pid.identities[0].has_tag(RevealedTagKind::DuguRevealed));

        // 等冷却过 + 切回旧 identity
        apply_switch(
            &mut pid,
            IdentityId(0),
            "offline:kiz",
            pos_at_niche(),
            100 + IDENTITY_SWITCH_COOLDOWN_TICKS,
            &registry,
        )
        .unwrap();
        // unfreeze 后 renown 状态完整
        assert!(!pid.identities[0].frozen);
        assert_eq!(pid.identities[0].renown.fame, 80);
        assert_eq!(pid.identities[0].renown.notoriety, 20);
        assert!(pid.identities[0].has_tag(RevealedTagKind::DuguRevealed));
        assert_eq!(pid.identities[0].reputation_score(), 80 - 20 - 50);
    }

    #[test]
    fn switching_to_new_identity_resets_reputation_to_zero() {
        // 切到全新 identity，reputation_score 应为 0（无 fame / notoriety / tag）。
        let mut pid = PlayerIdentities::with_default("kiz", 0);
        pid.identities[0].renown.notoriety = 50;
        pid.identities[0].revealed_tags.push(RevealedTag {
            kind: RevealedTagKind::DuguRevealed,
            witnessed_at_tick: 0,
            witness_realm: Realm::Spirit,
            permanent: true,
        });
        let registry = niche_at_origin("offline:kiz");

        let outcome = apply_new(
            &mut pid,
            "fresh",
            "offline:kiz",
            pos_at_niche(),
            100,
            &registry,
        )
        .unwrap();
        let new_active = pid.get(outcome.created_id).unwrap();
        assert_eq!(new_active.reputation_score(), 0);
        assert!(new_active.revealed_tags.is_empty());
        assert_eq!(new_active.renown.fame, 0);
        assert_eq!(new_active.renown.notoriety, 0);
    }

    #[test]
    fn cooldown_rejection_uses_chinese_message() {
        let err = IdentityCmdError::CooldownNotPassed {
            remaining_ticks: 12_000,
        };
        assert_eq!(err.message(), COOLDOWN_REJECTION_MSG);
    }

    #[test]
    fn unknown_identity_id_message_contains_id() {
        let err = IdentityCmdError::UnknownIdentityId(IdentityId(42));
        assert!(err.message().contains("42"), "msg={}", err.message());
    }

    #[test]
    fn already_active_message_contains_id() {
        let err = IdentityCmdError::AlreadyActive(IdentityId(7));
        assert!(err.message().contains("7"), "msg={}", err.message());
    }

    #[test]
    fn i32_to_identity_id_raw_passthrough_for_non_negative() {
        assert_eq!(i32_to_identity_id_raw(0), 0);
        assert_eq!(i32_to_identity_id_raw(7), 7);
        assert_eq!(i32_to_identity_id_raw(i32::MAX), i32::MAX as u32);
    }

    #[test]
    fn i32_to_identity_id_raw_negative_maps_to_max() {
        // 负数通过 -> u32::MAX，apply_switch 会 UnknownIdentityId 拒绝
        assert_eq!(i32_to_identity_id_raw(-1), u32::MAX);
        assert_eq!(i32_to_identity_id_raw(-100), u32::MAX);
        assert_eq!(i32_to_identity_id_raw(i32::MIN), u32::MAX);
    }

    #[test]
    fn negative_switch_id_rejected_via_unknown_identity_path() {
        // 防回归：codex review 指出 .max(0) 把负数 clamp 成 0 会误切到默认 identity。
        // 修复后负数 → u32::MAX → apply_switch 找不到该 id → UnknownIdentityId
        let mut pid = PlayerIdentities::with_default("kiz", 0);
        let registry = niche_at_origin("offline:kiz");
        let err = apply_switch(
            &mut pid,
            IdentityId(i32_to_identity_id_raw(-1)),
            "offline:kiz",
            pos_at_niche(),
            100,
            &registry,
        )
        .unwrap_err();
        assert_eq!(
            err,
            IdentityCmdError::UnknownIdentityId(IdentityId(u32::MAX))
        );
        assert_eq!(
            pid.active_identity_id,
            IdentityId::DEFAULT,
            "失败不改 state"
        );
        assert_eq!(pid.last_switch_tick, 0, "拒绝不消耗冷却");
    }
}
