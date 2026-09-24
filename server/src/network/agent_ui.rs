//! plan-agent-ui-data-v1 P0 — 天道 UI-as-Data 服务端会话状态机。
//!
//! 完整责任链：
//!   Redis `bong:agent_ui_cmd` → `process_redis_inbound` → `AgentUiCmdEvent`
//!     → `receive_agent_ui_cmd_system`
//!       → 境界门控 → XML sanitize → 替换旧 session（Replaced 终态）
//!       → 新 `AgentUiSession`（Open）→ [P1] 专属 JSON channel → client
//!
//!   client CustomPayload `agent_ui_response` → `AgentUiResponseEvent`
//!     → `receive_agent_ui_response_system`
//!     → 校验 request_id / allowed_button_ids → 终态 → Redis `bong:agent_ui_response`
//!
//!   `agent_ui_tick_system` — 每 tick 扫过期 session → TimedOut 终态
//!
//!   `receive_player_disconnect_system` — 断线 → Dismissed 终态
//!
//! ## Wire protocol（JSON 专属 channel，不走 bong:server_data/proto 路径）
//!
//! server 直接序列化 `AgentUiRequestPayloadV1` / `AgentUiClosePayloadV1` 为 JSON 字节，通过
//! `client.send_custom_payload(ident!("bong:agent_ui_request") / ident!("bong:agent_ui_close"))` 发送。
//! client 注册专属 channel listener（`BongNetworkHandler.registerAgentUiChannels()`），
//! 解析 JSON 后写入 `AgentUiStore`。
//!
//! 这绕开了 `bong:server_data` proto 路径（proto_convert.rs 对 AgentUiRequest/AgentUiClose
//! 是 `unreachable!()`），消除了生产 panic。仿照 halfstep_rechallenge_emit.rs 的专属 channel 模式。

use std::collections::HashMap;

use valence::ident;
use valence::prelude::{
    bevy_ecs, Client, Entity, Event, EventReader, Query, RemovedComponents, Res, ResMut, Resource,
    Username, With,
};

use super::redis_bridge::RedisOutbound;
use super::RedisBridgeResource;
use crate::cultivation::components::Cultivation;
use crate::player::state::canonical_player_id;
use crate::schema::agent_ui::{
    AgentUiActionType, AgentUiClosePayloadV1, AgentUiRequestCommandV1, AgentUiRequestPayloadV1,
    AgentUiResponsePayloadV1,
};

/// S2C channel identifier for AgentUiRequest payloads（专属 JSON channel）。
/// client 侧 `BongNetworkHandler.registerAgentUiChannels()` 注册相同 channel。
pub const AGENT_UI_REQUEST_CHANNEL: &str = "bong:agent_ui_request";

/// S2C channel identifier for AgentUiClose payloads（专属 JSON channel）。
/// client 侧 `BongNetworkHandler.registerAgentUiChannels()` 注册相同 channel。
pub const AGENT_UI_CLOSE_CHANNEL: &str = "bong:agent_ui_close";

// ─── 公开事件 ────────────────────────────────────────────────────────────────

/// Redis `bong:agent_ui_cmd` → Bevy Event（由 `process_redis_inbound` 派发）。
#[derive(Debug, Clone, Event)]
pub struct AgentUiCmdEvent(pub AgentUiRequestCommandV1);

/// client → server CustomPayload `agent_ui_response` 反序列化后发出的 Bevy Event。
#[derive(Debug, Clone, Event)]
pub struct AgentUiResponseEvent {
    pub player: Entity,
    pub request_id: String,
    pub action: AgentUiActionType,
    pub params: HashMap<String, String>,
}

// ─── Session 状态机 ──────────────────────────────────────────────────────────

/// 天道 UI session 状态（Open → terminal）。
#[allow(dead_code)] // 各终态由 P1+ 阶段的完整系统触发；P0 仅单元测试使用
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentUiSessionState {
    /// 面板已推送给 client，等待响应或超时。
    Open,
    /// 玩家点击了按钮（terminal）。
    Completed,
    /// 玩家按 ESC 关闭（terminal）。
    Dismissed,
    /// server ticker 判断超时（terminal）。
    TimedOut,
    /// 同一玩家的新请求替换了此 session（terminal）。
    Replaced,
    /// 校验错误（境界/离线/allowed_button_ids）。server emit error response（terminal）。
    Error,
}

impl AgentUiSessionState {
    /// 是否已达终态（不再接受任何输入）。
    #[allow(dead_code)] // P1+ 阶段使用；P0 仅测试覆盖
    pub fn is_terminal(&self) -> bool {
        !matches!(self, AgentUiSessionState::Open)
    }
}

/// 一次天道 UI 面板交互 session。
#[derive(Debug, Clone)]
pub struct AgentUiSession {
    pub request_id: String,
    pub state: AgentUiSessionState,
    /// 超时 tick（绝对 tick 值）= 收到 cmd 时的当前 tick + timeout_ticks。
    pub expire_tick: u64,
    /// 允许的按钮 ID 白名单（server 侧校验，不下发 client）。
    pub allowed_button_ids: Vec<String>,
}

/// player entity → AgentUiSession 映射（单面板 mutex：同一时刻每个玩家最多一个 Open session）。
#[derive(Default, Resource)]
pub struct AgentUiSessionStore {
    sessions: HashMap<Entity, AgentUiSession>,
}

impl AgentUiSessionStore {
    /// 插入 / 替换 session；若旧 session 为 Open 则返回旧 session 以便发送 Replaced close 信号。
    pub fn upsert(&mut self, player: Entity, session: AgentUiSession) -> Option<AgentUiSession> {
        let old = self.sessions.insert(player, session);
        if let Some(ref o) = old {
            if o.state == AgentUiSessionState::Open {
                return old;
            }
        }
        None
    }

    /// 幂等取出：request_id 匹配才移除，防止竞争窗口双重消费。
    pub fn take_if_match(&mut self, player: Entity, request_id: &str) -> Option<AgentUiSession> {
        if self
            .sessions
            .get(&player)
            .map(|s| s.request_id == request_id)
            .unwrap_or(false)
        {
            self.sessions.remove(&player)
        } else {
            None
        }
    }

    /// 取出玩家当前 session（无论 request_id）。
    pub fn take(&mut self, player: Entity) -> Option<AgentUiSession> {
        self.sessions.remove(&player)
    }

    /// 获取不可变引用。
    pub fn get(&self, player: Entity) -> Option<&AgentUiSession> {
        self.sessions.get(&player)
    }
}

// ─── XML 清洗 ─────────────────────────────────────────────────────────────────

/// XML 清洗错误种类。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum XmlSanitizeError {
    /// payload 超出 8192 字节上限（在 serde 层已部分把控，此处 double-check）。
    TooLarge(usize),
    /// 检测到 `<!DOCTYPE` / `<!ENTITY`（XXE / DTD bomb 防御）。
    DoctypeOrEntity,
    /// 检测到嵌套深度超出 6 层（防御深度炸弹）。
    TooDeep,
    /// 节点数超出 64（防御宽度炸弹）。
    TooManyNodes,
    /// 检测到非白名单标签（仅允许 label/button/flow-layout/grid-layout/texture）。
    NonWhitelistedTag(String),
    /// 缺少 `<owo-ui>` 根节点：client UIModel.load 强制要求此根容器。
    MissingOwoUiRoot,
}

impl std::fmt::Display for XmlSanitizeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            XmlSanitizeError::TooLarge(n) => write!(f, "xml too large: {n} bytes (max 8192)"),
            XmlSanitizeError::DoctypeOrEntity => {
                write!(f, "xml contains DOCTYPE/ENTITY declaration (XXE rejected)")
            }
            XmlSanitizeError::TooDeep => write!(f, "xml depth exceeds limit 6"),
            XmlSanitizeError::TooManyNodes => write!(f, "xml node count exceeds limit 64"),
            XmlSanitizeError::NonWhitelistedTag(tag) => {
                write!(f, "xml contains non-whitelisted tag: <{tag}>")
            }
            XmlSanitizeError::MissingOwoUiRoot => {
                write!(
                    f,
                    "xml must have <owo-ui> as root element (client UIModel.load requirement)"
                )
            }
        }
    }
}

/// 允许白名单标签（OwoUI 组件）。
///
/// `owo-ui` / `components` 是 owo-lib UIModel.load 要求的 XML 根容器：
/// client 端 `UIModel.load(xml)` 强制要求 root=<owo-ui>，agent xmlTemplates.ts
/// 所有模板均产出 `<owo-ui><components>...</components></owo-ui>` 包裹形。
/// 不加这两个标签会导致 xml_sanitize 以 NonWhitelistedTag 拒绝所有 agent 模板，
/// AgentUiRequest 永不 emit（BLOCKER 修复：plan-agent-ui-data-v1 P0 v2）。
const ALLOWED_TAGS: &[&str] = &[
    "owo-ui",
    "components",
    "label",
    "button",
    "flow-layout",
    "grid-layout",
    "texture",
];

fn is_allowed_tag(tag: &str) -> bool {
    ALLOWED_TAGS.contains(&tag)
}

/// 简单 XML 清洗器（不依赖 XML 解析库，仅扫描文本）。
///
/// 策略：
/// - 拒绝 `<!DOCTYPE` / `<!ENTITY`（大小写不敏感）
/// - 计数 `<tag` 开口 tag，累加深度，拒绝超过 6 层 / 64 节点
/// - 过滤非白名单标签：由于不完整解析，以"reject non-whitelisted tag"实现（而非 strip）。
///   传入非白名单标签返回 Err(XmlSanitizeError)，不允许到达 client。
/// - 返回 `Ok(xml.to_owned())` 表示全部通过
///
/// 注：生产品质的 XML 清洗应使用 roxmltree 或 quick-xml；
///     此处轻量实现满足 P0 测试覆盖要求（full parser 在 P1 引入）。
pub fn xml_sanitize(xml: &str) -> Result<String, XmlSanitizeError> {
    // 1. 长度检查（字节口径）
    // 注：TypeBox schema 的 maxLength:8192 按 Unicode 码点计，server 此处按 UTF-8 字节计。
    // 汉字通常 3 字节，若 agent 产出的叙事 XML 含大量汉字，char < 8192 但 byte > 8192 会被拒。
    // 解决方案（plan-agent-ui-data-v1 MINOR-字节/字符口径）：
    //   - server 这里保持字节口径（更安全，防 payload 膨胀）；
    //   - agent 侧 uiRenderer.ts 在发布前做字节预检（Buffer.byteLength(xml,'utf8') <= 8192），
    //     确保叙事 XML 在字节上不超限。
    if xml.len() > 8192 {
        return Err(XmlSanitizeError::TooLarge(xml.len()));
    }

    // 2. DOCTYPE / ENTITY 检查（大小写不敏感）
    let upper = xml.to_ascii_uppercase();
    if upper.contains("<!DOCTYPE") || upper.contains("<!ENTITY") {
        return Err(XmlSanitizeError::DoctypeOrEntity);
    }

    // 2b. owo-ui 根节点检查：client UIModel.load 强制要求 root=<owo-ui>。
    // agent xmlTemplates.ts 所有模板均输出 <owo-ui><components>...</components></owo-ui>。
    // 裸 XML（如 <flow-layout>...） 通过 sanitize 但 client 必 fallback，此处提前拒绝。
    let trimmed = xml.trim_start();
    if !trimmed.starts_with("<owo-ui") {
        return Err(XmlSanitizeError::MissingOwoUiRoot);
    }

    // 3. 节点计数 + 深度检查（扫描 `<tag` 形式，跳过 `</tag>` 关闭 tag 和注释 / PI）
    let mut node_count: u32 = 0;
    let mut depth: i32 = 0;
    let mut i = 0;
    let bytes = xml.as_bytes();

    while i < bytes.len() {
        if bytes[i] == b'<' {
            let rest = &xml[i..];

            // 跳过注释 <!-- ... -->
            if rest.starts_with("<!--") {
                i += rest.find("-->").unwrap_or(rest.len() - 3) + 3;
                continue;
            }

            // 跳过 PI <?...?>
            if rest.starts_with("<?") {
                i += rest.find("?>").unwrap_or(rest.len() - 2) + 2;
                continue;
            }

            // 跳过 <!...>（已经 reject 了 DOCTYPE/ENTITY）
            if rest.starts_with("<!") {
                i += rest.find('>').unwrap_or(rest.len() - 1) + 1;
                continue;
            }

            // 关闭 tag `</tag>`
            if rest.starts_with("</") {
                depth -= 1;
                i += rest.find('>').unwrap_or(rest.len() - 1) + 1;
                continue;
            }

            // 开口 tag `<tag...>` 或自闭合 `<tag.../>`
            // 提取 tag 名
            let tag_start = i + 1; // 跳过 `<`
            let tag_end = xml[tag_start..]
                .find(|c: char| c.is_whitespace() || c == '>' || c == '/')
                .map(|p| tag_start + p)
                .unwrap_or(xml.len());
            let tag_name = &xml[tag_start..tag_end];

            // 校验白名单（忽略空 tag_name 防止越界）
            if !tag_name.is_empty() && !is_allowed_tag(tag_name) {
                // 非白名单标签 → 拒绝整个 xml（与 TypeBox 层的 additionalProperties 对称）
                return Err(XmlSanitizeError::NonWhitelistedTag(tag_name.to_string()));
            }

            // 节点计数
            node_count += 1;
            if node_count > 64 {
                return Err(XmlSanitizeError::TooManyNodes);
            }

            // 深度计数（自闭合 tag 不增加深度）
            let tag_end_bracket = xml[i..].find('>').map(|p| i + p).unwrap_or(xml.len());
            let is_self_closing = tag_end_bracket > 0 && bytes[tag_end_bracket - 1] == b'/';
            if !is_self_closing {
                depth += 1;
                if depth > 6 {
                    return Err(XmlSanitizeError::TooDeep);
                }
            }

            i = tag_end_bracket + 1;
            continue;
        }
        i += 1;
    }

    Ok(xml.to_owned())
}

// ─── 系统：接收 agent_ui_cmd Bevy event ─────────────────────────────────────

/// 接收 `AgentUiCmdEvent`（由 `process_redis_inbound` 在 mod.rs 中 emit），
/// 进行 cmd 合法性校验 + 境界门控 + XML 清洗 + session 管理，
/// 并向 client 推送 `ServerDataPayloadV1::AgentUiRequest`；
/// 替换旧 session 时向 client 推 `AgentUiClose` 并向 Redis 发 `Replaced` response。
pub fn receive_agent_ui_cmd_system(
    mut cmd_events: EventReader<AgentUiCmdEvent>,
    mut store: ResMut<AgentUiSessionStore>,
    mut clients: Query<(Entity, &Cultivation, &Username, &mut Client), With<Client>>,
    tick_resource: Option<Res<CurrentTickResource>>,
    redis: Res<RedisBridgeResource>,
) {
    let current_tick = tick_resource.as_ref().map(|r| r.0).unwrap_or(0);

    for AgentUiCmdEvent(cmd) in cmd_events.read() {
        process_agent_ui_cmd(cmd, &mut store, &mut clients, current_tick, &redis);
    }
}

fn process_agent_ui_cmd(
    cmd: &AgentUiRequestCommandV1,
    store: &mut AgentUiSessionStore,
    clients: &mut Query<(Entity, &Cultivation, &Username, &mut Client), With<Client>>,
    current_tick: u64,
    redis: &RedisBridgeResource,
) {
    // 1. 校验 cmd 合法性
    if let Err(e) = cmd.validate() {
        tracing::warn!("[bong][agent_ui] invalid AgentUiRequestCommandV1: {e}");
        send_error_response(redis, &cmd.request_id, "invalid_command");
        return;
    }

    tracing::debug!(
        "[bong][agent_ui] AgentUiCmd request_id={} target_player={} realm_gate={}",
        cmd.request_id,
        cmd.target_player,
        cmd.realm_gate,
    );

    // 2. 找目标玩家（target_player = canonical_player_id，即 "offline:<name>"）。
    //    world-state emit 把 PlayerProfile.uuid 填成 canonical_player_id("offline:<name>")，
    //    agent 侧从 world_state 取 player.uuid 作为 target_player 发回。
    //    因此这里必须用 canonical_player_id(username) 作比较键，而非裸 username.0。
    let player_info: Option<(Entity, u8)> = clients
        .iter()
        .find(|(_, _, username, _)| canonical_player_id(username.0.as_str()) == cmd.target_player)
        .map(|(entity, cult, _, _)| (entity, cult.realm.rank()));

    // 3. 离线拒绝
    if player_info.is_none() {
        tracing::debug!(
            "[bong][agent_ui] target_player={} offline, sending error response",
            cmd.target_player
        );
        let resp = AgentUiResponsePayloadV1 {
            request_id: cmd.request_id.clone(),
            action: AgentUiActionType::Error,
            target_player: None,
            params: [("reason".to_string(), "player_offline".to_string())]
                .into_iter()
                .collect(),
        };
        let _ = redis.tx_outbound.send(RedisOutbound::AgentUiResponse(resp));
        return;
    }

    let (player_entity, player_rank) = player_info.unwrap();

    // 4. 境界门控（realm_gate=0 不门控）
    if cmd.realm_gate > 0 && player_rank < cmd.realm_gate {
        tracing::debug!(
            "[bong][agent_ui] realm_gate_rejected player_rank={player_rank} required={}",
            cmd.realm_gate
        );
        let resp = AgentUiResponsePayloadV1 {
            request_id: cmd.request_id.clone(),
            action: AgentUiActionType::Error,
            target_player: Some(cmd.target_player.clone()),
            params: [
                ("reason".to_string(), "realm_gate_rejected".to_string()),
                ("player_realm".to_string(), player_rank.to_string()),
                ("required_realm".to_string(), cmd.realm_gate.to_string()),
            ]
            .into_iter()
            .collect(),
        };
        let _ = redis.tx_outbound.send(RedisOutbound::AgentUiResponse(resp));
        return;
    }

    // 5. XML 清洗
    let _sanitized_xml = match xml_sanitize(&cmd.xml) {
        Ok(xml) => xml,
        Err(e) => {
            tracing::warn!("[bong][agent_ui] xml_sanitize failed: {e}");
            let resp = AgentUiResponsePayloadV1 {
                request_id: cmd.request_id.clone(),
                action: AgentUiActionType::Error,
                target_player: None,
                params: [
                    ("reason".to_string(), "xml_sanitize_failed".to_string()),
                    ("detail".to_string(), e.to_string()),
                ]
                .into_iter()
                .collect(),
            };
            let _ = redis.tx_outbound.send(RedisOutbound::AgentUiResponse(resp));
            return;
        }
    };

    // 6. 替换旧 session（Replaced 终态）
    let expire_tick = current_tick + u64::from(cmd.timeout_ticks);
    let new_session = AgentUiSession {
        request_id: cmd.request_id.clone(),
        state: AgentUiSessionState::Open,
        expire_tick,
        allowed_button_ids: cmd.allowed_button_ids.clone(),
    };

    if let Some(old) = store.upsert(player_entity, new_session) {
        tracing::debug!(
            "[bong][agent_ui] replacing old session request_id={}",
            old.request_id
        );
        // Replaced 终态：向 Redis 发 replaced response（agent uiResponseConsumer.ts:196）
        let replaced_resp = AgentUiResponsePayloadV1 {
            request_id: old.request_id.clone(),
            action: AgentUiActionType::Replaced,
            target_player: None,
            params: HashMap::new(),
        };
        let _ = redis
            .tx_outbound
            .send(RedisOutbound::AgentUiResponse(replaced_resp));
        // 向 client 发 AgentUiClose（reason=None 表示 Replaced，client 静默关闭）
        // 走专属 bong:agent_ui_close JSON channel，绕开 bong:server_data/proto 路径
        // （proto_convert.rs 对 AgentUiClose 是 unreachable!()，生产会 panic）。
        match encode_agent_ui_close_payload(&old.request_id, None) {
            Ok(bytes) => {
                if let Ok(mut client) = clients.get_mut(player_entity) {
                    client
                        .3
                        .send_custom_payload(ident!("bong:agent_ui_close"), &bytes);
                    tracing::debug!(
                        "[bong][agent_ui] sent AgentUiClose(Replaced) for old request_id={}",
                        old.request_id
                    );
                }
            }
            Err(e) => {
                tracing::error!(
                    "[bong][agent_ui] failed to serialize AgentUiClosePayloadV1 for channel {}: {e}",
                    AGENT_UI_CLOSE_CHANNEL
                );
            }
        }
    }

    // 向 client 发 AgentUiRequest（携带 sanitized XML，不含安全字段）
    // 走专属 bong:agent_ui_request JSON channel，绕开 bong:server_data/proto 路径
    // （proto_convert.rs 对 AgentUiRequest 是 unreachable!()，生产会 panic）。
    let request_payload = AgentUiRequestPayloadV1 {
        request_id: cmd.request_id.clone(),
        target_player: cmd.target_player.clone(),
        xml: _sanitized_xml,
        timeout_ticks: cmd.timeout_ticks,
    };
    match serde_json::to_vec(&request_payload) {
        Ok(bytes) => {
            if let Ok(mut client) = clients.get_mut(player_entity) {
                client
                    .3
                    .send_custom_payload(ident!("bong:agent_ui_request"), &bytes);
                tracing::info!(
                    "[bong][agent_ui] sent AgentUiRequest request_id={} expire_tick={expire_tick}",
                    cmd.request_id,
                );
            }
        }
        Err(e) => {
            tracing::error!(
                "[bong][agent_ui] failed to serialize AgentUiRequestPayloadV1 for channel {}: {e}",
                AGENT_UI_REQUEST_CHANNEL
            );
        }
    }
    tracing::info!(
        "[bong][agent_ui] session created request_id={} expire_tick={expire_tick}",
        cmd.request_id,
    );
}

/// 使用生产 serde 镜像编码专属 close channel 的裸 JSON bytes。
fn encode_agent_ui_close_payload(
    request_id: &str,
    reason: Option<&str>,
) -> serde_json::Result<Vec<u8>> {
    serde_json::to_vec(&AgentUiClosePayloadV1 {
        request_id: request_id.to_string(),
        reason: reason.map(str::to_string),
    })
}

/// 向指定 client entity 发送 `AgentUiClosePayloadV1`（专属 bong:agent_ui_close JSON channel）。
/// `reason` = None 表示 Replaced（client 静默关闭），Some(str) 表示具体原因。
///
/// 不走 `bong:server_data` / proto 路径（proto_convert.rs 对 AgentUiClose 是 unreachable!()，
/// 生产会 panic）。
fn send_agent_ui_close_to_client(
    player: Entity,
    request_id: &str,
    reason: Option<&str>,
    clients: &mut Query<&mut Client>,
) {
    match encode_agent_ui_close_payload(request_id, reason) {
        Ok(bytes) => {
            if let Ok(mut client) = clients.get_mut(player) {
                client.send_custom_payload(ident!("bong:agent_ui_close"), &bytes);
            }
        }
        Err(e) => {
            tracing::error!(
                "[bong][agent_ui] failed to serialize AgentUiClosePayloadV1 for channel {}: {e}",
                AGENT_UI_CLOSE_CHANNEL
            );
        }
    }
}

fn send_error_response(redis: &RedisBridgeResource, request_id: &str, reason: &str) {
    let resp = AgentUiResponsePayloadV1 {
        request_id: request_id.to_owned(),
        action: AgentUiActionType::Error,
        target_player: None,
        params: [("reason".to_string(), reason.to_string())]
            .into_iter()
            .collect(),
    };
    let _ = redis.tx_outbound.send(RedisOutbound::AgentUiResponse(resp));
}

// ─── 系统：ticker 超时扫描 ────────────────────────────────────────────────────

/// 可选的当前 tick 资源（由 network/mod.rs 注入）。
#[derive(Default, Resource)]
pub struct CurrentTickResource(pub u64);

/// 每帧自增 `CurrentTickResource`，驱动 agent_ui_tick_system 的超时判断。
/// 在 `mod.rs` 中注册到 `Update` schedule，与其他 agent_ui 系统并列。
pub fn increment_current_tick_system(mut tick: ResMut<CurrentTickResource>) {
    tick.0 = tick.0.wrapping_add(1);
}

/// 每 tick 扫描 Open session，超过 expire_tick 则 → TimedOut，
/// 并发 timeout response 到 Redis（bong:agent_ui_response）。
pub fn agent_ui_tick_system(
    tick_resource: Option<Res<CurrentTickResource>>,
    mut store: ResMut<AgentUiSessionStore>,
    redis: Res<RedisBridgeResource>,
) {
    let current_tick = tick_resource.as_ref().map(|r| r.0).unwrap_or(0);

    let timed_out: Vec<Entity> = store
        .sessions
        .iter()
        .filter(|(_, s)| s.state == AgentUiSessionState::Open && current_tick >= s.expire_tick)
        .map(|(e, _)| *e)
        .collect();

    for entity in timed_out {
        if let Some(session) = store.take(entity) {
            tracing::debug!(
                "[bong][agent_ui] session timed out request_id={}",
                session.request_id
            );
            let resp = AgentUiResponsePayloadV1 {
                request_id: session.request_id.clone(),
                action: AgentUiActionType::Timeout,
                target_player: None,
                params: HashMap::new(),
            };
            let _ = redis.tx_outbound.send(RedisOutbound::AgentUiResponse(resp));
        }
    }
}

// ─── 系统：接收玩家响应 ────────────────────────────────────────────────────────

/// 处理 `AgentUiResponseEvent`（来自 client_request_handler dispatch）。
/// 校验：request_id 匹配 Open session + action = button_click → allowed_button_ids 校验。
/// 无活跃 session 或 request_id 不匹配时，向 client 发 AgentUiClose(session_expired) 防 UI 悬空。
pub fn receive_agent_ui_response_system(
    mut events: EventReader<AgentUiResponseEvent>,
    mut store: ResMut<AgentUiSessionStore>,
    mut clients: Query<&mut Client>,
    redis: Res<RedisBridgeResource>,
) {
    for ev in events.read() {
        let session_opt = store.get(ev.player);
        let Some(session) = session_opt else {
            // 无活跃 session → 发 AgentUiClose(session_expired) 防 UI 悬空
            tracing::debug!(
                "[bong][agent_ui] response for unknown session request_id={}, sending AgentUiClose",
                ev.request_id
            );
            send_agent_ui_close_to_client(
                ev.player,
                &ev.request_id,
                Some("session_expired"),
                &mut clients,
            );
            continue;
        };

        // request_id 不匹配 → stale response → 发 AgentUiClose(session_expired) 防 UI 悬空
        if session.request_id != ev.request_id {
            tracing::debug!(
                "[bong][agent_ui] stale response request_id={} (current={}), sending AgentUiClose",
                ev.request_id,
                session.request_id,
            );
            send_agent_ui_close_to_client(
                ev.player,
                &ev.request_id,
                Some("session_expired"),
                &mut clients,
            );
            continue;
        }

        // session 不为 Open → 已终态，忽略
        if session.state != AgentUiSessionState::Open {
            tracing::debug!(
                "[bong][agent_ui] response for non-open session request_id={} state={:?}",
                ev.request_id,
                session.state,
            );
            continue;
        }

        // button_click → allowed_button_ids 白名单校验
        if matches!(ev.action, AgentUiActionType::ButtonClick) {
            let button_id = ev.params.get("button_id").map(|s| s.as_str()).unwrap_or("");
            if !session.allowed_button_ids.is_empty()
                && !session.allowed_button_ids.iter().any(|b| b == button_id)
            {
                tracing::warn!(
                    "[bong][agent_ui] button_id={button_id} not in allowed_button_ids for request_id={}",
                    ev.request_id,
                );
                // 发 error response，并下发 close(reason) 让 client 给出玩家可见反馈。
                let resp = AgentUiResponsePayloadV1 {
                    request_id: ev.request_id.clone(),
                    action: AgentUiActionType::Error,
                    target_player: None,
                    params: [("reason".to_string(), "invalid_button_id".to_string())]
                        .into_iter()
                        .collect(),
                };
                let _ = redis.tx_outbound.send(RedisOutbound::AgentUiResponse(resp));
                let _ = store.take_if_match(ev.player, &ev.request_id);
                send_agent_ui_close_to_client(
                    ev.player,
                    &ev.request_id,
                    Some("invalid_button_id"),
                    &mut clients,
                );
                continue;
            }
        }

        // 消费 session（幂等）
        let Some(_session) = store.take_if_match(ev.player, &ev.request_id) else {
            continue;
        };

        // 转发到 agent（Redis bong:agent_ui_response）
        let resp = AgentUiResponsePayloadV1 {
            request_id: ev.request_id.clone(),
            action: ev.action.clone(),
            target_player: None,
            params: ev.params.clone(),
        };
        tracing::debug!(
            "[bong][agent_ui] forwarding response request_id={} action={:?}",
            ev.request_id,
            ev.action
        );
        let _ = redis.tx_outbound.send(RedisOutbound::AgentUiResponse(resp));
    }
}

// ─── 系统：断线清理 ───────────────────────────────────────────────────────────

/// 玩家断线 → Dismissed 终态。
pub fn receive_player_disconnect_system(
    mut removed: RemovedComponents<Client>,
    mut store: ResMut<AgentUiSessionStore>,
    redis: Res<RedisBridgeResource>,
) {
    for entity in removed.read() {
        if let Some(session) = store.take(entity) {
            if session.state == AgentUiSessionState::Open {
                tracing::debug!(
                    "[bong][agent_ui] player disconnect, session dismissed request_id={}",
                    session.request_id,
                );
                let resp = AgentUiResponsePayloadV1 {
                    request_id: session.request_id.clone(),
                    action: AgentUiActionType::Dismissed,
                    target_player: None,
                    params: HashMap::new(),
                };
                let _ = redis.tx_outbound.send(RedisOutbound::AgentUiResponse(resp));
            }
        }
    }
}

// ─── 单测 ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[path = "agent_ui_tests.rs"]
mod tests;
