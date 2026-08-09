"""agent_ui 场景共享 helper：注入 bong:agent_ui_cmd + 断言 S2C 专属通道 / Redis 回执。

覆盖 HEADLESSAUDIT 优先项 3（agent_ui_response）的 wire 契约：
- cmd 注入：Redis PUBLISH `bong:agent_ui_cmd`，payload = AgentUiRequestCommandV1 JSON
  （server 侧 serde deny_unknown_fields，六个字段全必填）。
- S2C 下发：`bong:agent_ui_request` 专属 JSON 通道（AgentUiRequestPayloadV1），
  **安全字段 realm_gate / allowed_button_ids 绝不下发**。
- S2C 关闭：`bong:agent_ui_close`（AgentUiClosePayloadV1，reason 可为 null=Replaced）。
- 回执：Redis `bong:agent_ui_response`（AgentUiResponsePayloadV1，
  action ∈ button_click/dismissed/timeout/replaced/error/parse_error）。

纯逻辑部分（cmd 构造、payload 形状断言、消息匹配）独立成函数，供 test_protocol.py
单测；wait 循环依赖 bot.wait_for / RedisPubSub，由场景集成覆盖。
"""

from __future__ import annotations

import json

from bot._redis_helpers import RedisPubSub
from bot.bot import BotAssertionError

DEFAULT_UI_XML = (
    "<owo-ui><components><flow-layout>"
    '<label>gap3 probe</label><button id="enter_realm">进入</button>'
    "</flow-layout></components></owo-ui>"
)

REQ_CHANNEL = "bong:agent_ui_request"
CLOSE_CHANNEL = "bong:agent_ui_close"
RESP_CHANNEL = "bong:agent_ui_response"

# C2S 请求的固定骨架；params 因 action 而异。
C2S_VERSION = 1


def build_cmd(
    request_id: str,
    target_player: str,
    timeout_ticks: int = 600,
    realm_gate: int = 0,
    allowed_button_ids: tuple[str, ...] = ("enter_realm", "cancel"),
    xml: str = DEFAULT_UI_XML,
) -> dict:
    """构造 AgentUiRequestCommandV1 JSON（六个字段全必填，与 Rust serde 镜像对齐）。"""
    return {
        "request_id": request_id,
        "target_player": target_player,
        "xml": xml,
        "timeout_ticks": timeout_ticks,
        "realm_gate": realm_gate,
        "allowed_button_ids": list(allowed_button_ids),
    }


def publish_cmd(redis: RedisPubSub, **cmd_fields) -> dict:
    """构造并注入一条 agent_ui_cmd；返回构造好的 cmd dict（供断言复用）。"""
    cmd = build_cmd(**cmd_fields)
    redis.publish("bong:agent_ui_cmd", json.dumps(cmd))
    return cmd


def assert_request_shape(payload: dict, request_id: str) -> None:
    """断言 S2C AgentUiRequestPayloadV1 形状 + 安全字段绝不下发。"""
    if payload.get("request_id") != request_id:
        raise BotAssertionError(
            f"bong:agent_ui_request request_id 应为 {request_id!r}，实际 {payload.get('request_id')!r}"
        )
    if (
        not isinstance(payload.get("target_player"), str)
        or not payload["target_player"]
    ):
        raise BotAssertionError(f"bong:agent_ui_request 缺 target_player：{payload!r}")
    if not isinstance(payload.get("xml"), str) or not payload["xml"]:
        raise BotAssertionError(f"bong:agent_ui_request 缺 xml：{payload!r}")
    if not isinstance(payload.get("timeout_ticks"), int):
        raise BotAssertionError(f"bong:agent_ui_request 缺 timeout_ticks：{payload!r}")
    if "realm_gate" in payload:
        raise BotAssertionError(
            "realm_gate 是 server 内部安全字段，绝不下发给 client（实际下发）"
        )
    if "allowed_button_ids" in payload:
        raise BotAssertionError(
            "allowed_button_ids 是 server 内部安全字段，绝不下发给 client（实际下发）"
        )


def response_matches(
    payload: dict, action: str | None = None, params_subset: dict | None = None
) -> bool:
    """bong:agent_ui_response 消息匹配：action 精确 + params 子集。"""
    if action is not None and payload.get("action") != action:
        return False
    if params_subset:
        got = payload.get("params") or {}
        if not isinstance(got, dict):
            return False
        if not all(got.get(key) == value for key, value in params_subset.items()):
            return False
    return True


def _payload_of(event) -> dict:
    raw = event.data["data"]
    try:
        return json.loads(raw.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise BotAssertionError(
            f"channel payload 不是合法 JSON：{error!r}（bytes={raw[:80]!r}）"
        ) from error


def expect_agent_ui_request(
    bot,
    request_id: str,
    after: float | None = None,
    timeout: float = 15.0,
    expect: bool = True,
) -> dict | None:
    """等 request_id 匹配的 bong:agent_ui_request payload 并断言形状。

    ``expect=False`` 时超时返回 None（负向断言：不应下发）。
    """

    def matches(event) -> bool:
        if event.kind != "payload" or event.data["channel"] != REQ_CHANNEL:
            return False
        if after is not None and event.t <= after:
            return False
        try:
            return (
                json.loads(event.data["data"].decode("utf-8")).get("request_id")
                == request_id
            )
        except (UnicodeDecodeError, json.JSONDecodeError):
            return False

    if not expect:
        try:
            bot.wait_for(
                matches, timeout, f"request_id={request_id} 的 {REQ_CHANNEL} payload"
            )
        except BotAssertionError:
            return None
        raise BotAssertionError(
            f"不应出现 request_id={request_id} 的 {REQ_CHANNEL} payload（拒绝路径漏下发）"
        )
    event = bot.wait_for(
        matches, timeout, f"request_id={request_id} 的 {REQ_CHANNEL} payload"
    )
    payload = _payload_of(event)
    assert_request_shape(payload, request_id)
    return payload


def expect_agent_ui_close(
    bot,
    request_id: str,
    reason: str | None,
    after: float | None = None,
    timeout: float = 15.0,
    expect: bool = True,
) -> dict | None:
    """等 request_id 匹配的 bong:agent_ui_close payload；断言 reason（None=Replaced 静默）。"""

    def matches(event) -> bool:
        if event.kind != "payload" or event.data["channel"] != CLOSE_CHANNEL:
            return False
        if after is not None and event.t <= after:
            return False
        try:
            return (
                json.loads(event.data["data"].decode("utf-8")).get("request_id")
                == request_id
            )
        except (UnicodeDecodeError, json.JSONDecodeError):
            return False

    if not expect:
        try:
            bot.wait_for(
                matches, timeout, f"request_id={request_id} 的 {CLOSE_CHANNEL} payload"
            )
        except BotAssertionError:
            return None
        raise BotAssertionError(
            f"不应出现 request_id={request_id} 的 {CLOSE_CHANNEL} payload"
        )
    event = bot.wait_for(
        matches, timeout, f"request_id={request_id} 的 {CLOSE_CHANNEL} payload"
    )
    payload = _payload_of(event)
    if payload.get("request_id") != request_id:
        raise BotAssertionError(
            f"{CLOSE_CHANNEL} request_id 应为 {request_id!r}，实际 {payload.get('request_id')!r}"
        )
    if payload.get("reason") != reason:
        raise BotAssertionError(
            f"{CLOSE_CHANNEL} reason 应为 {reason!r}（None=Replaced 静默），"
            f"实际 {payload.get('reason')!r}"
        )
    return payload


def expect_redis_response(
    redis: RedisPubSub,
    request_id: str,
    timeout: float = 15.0,
    expect: bool = True,
    **want,
) -> dict | None:
    """等 bong:agent_ui_response 上 request_id 匹配的消息；断言 action/params 子集。"""

    def matches(payload: dict) -> bool:
        return payload.get("request_id") == request_id and response_matches(
            payload, **want
        )

    got = redis.wait_message(RESP_CHANNEL, matches, timeout=timeout, expect=expect)
    if got is None and expect:
        raise AssertionError(f"request_id={request_id} 的 {RESP_CHANNEL} 回执未出现")
    return got


def expect_no_redis_response(
    redis: RedisPubSub, request_id: str, timeout: float = 2.0
) -> None:
    """负向断言：request_id 匹配的 bong:agent_ui_response 在窗口内不应出现。"""
    got = expect_redis_response(redis, request_id, timeout=timeout, expect=False)
    if got is not None:
        raise BotAssertionError(
            f"不应出现 request_id={request_id} 的 {RESP_CHANNEL} 回执，实际收到 {got!r}"
        )
