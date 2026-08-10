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
单测；wait 循环依赖 bot.wait_for / RedisClient，由场景集成覆盖。
"""

from __future__ import annotations

import json

from bot._redis_client_helpers import RedisClient
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


def publish_cmd(redis: RedisClient, **cmd_fields) -> dict:
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
    timeout_ticks = payload.get("timeout_ticks")
    if not isinstance(timeout_ticks, int) or isinstance(timeout_ticks, bool):
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
    payload: dict,
    action: str | None = None,
    params_subset: dict | None = None,
    params_exact: dict | None = None,
) -> bool:
    """bong:agent_ui_response 消息匹配：action 精确 + params 子集/精确。

    子集匹配要求 key 存在且值相等——缺失的 key 不等于显式 null，不能放行。
    精确匹配要求 params 与声明逐键相等（原样转发契约用）：额外字段即违约。
    ``params_exact`` 与 ``params_subset`` 互斥，同时给出是调用方错误。
    """
    if action is not None and payload.get("action") != action:
        return False
    if params_exact is not None and params_subset is not None:
        raise ValueError("params_exact 与 params_subset 互斥，不能同时给出")
    if params_exact is not None:
        got = payload.get("params")
        if not isinstance(got, dict):
            return False
        return got == params_exact
    if params_subset is not None:
        got = payload.get("params")
        if not isinstance(got, dict):
            return False
        if params_subset == {}:
            return got == {}
        if not all(
            key in got and got[key] == value for key, value in params_subset.items()
        ):
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


def _decode_payload_request_id(event) -> tuple[object | None, str | None]:
    """解码 payload 的 request_id；返回 ``(request_id, 描述)``。

    合法 JSON 对象返回 ``(request_id 或 None, None)``；非对象 JSON（数组/字符串/
    数字/null）与非法 JSON 返回 ``(None, 描述)``。匹配器据此不崩（不得对非 dict
    调 ``.get`` 抛 AttributeError），负向断言据此把「形状错误但确实送达的 payload」
    判为违约并报告收到的形状。
    """
    raw = event.data["data"]
    try:
        data = json.loads(raw.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        return None, f"payload 不是合法 JSON：{error!r}（bytes={raw[:80]!r}）"
    if not isinstance(data, dict):
        return None, f"payload 是 {type(data).__name__} 而非 JSON 对象：{data!r}"
    return data.get("request_id"), None


def expect_agent_ui_request(
    bot,
    request_id: str,
    after: float | None = None,
    timeout: float = 15.0,
    expect: bool = True,
) -> dict | None:
    """等 request_id 匹配的 bong:agent_ui_request payload 并断言形状。

    ``expect=False`` 时超时返回 None（负向断言：不应下发）。负向断言观测的是
    **任何** bong:agent_ui_request payload 是否在窗口内到达目标 bot——拒绝路径的
    契约是「面板不下发」，不是「不下发指定 request_id 的面板」：实现若漏做门禁却
    仍发出形状错误（request_id 缺失/损坏/非对象 JSON）的 payload，必须失败并报告
    收到的形状，而不是靠 request_id 过滤把错误下发当干净拒绝。bong:agent_ui_request
    是 per-client custom payload（server 端 ``send_custom_payload``），本 bot 只会
    收到发给自己的 payload，无跨 bot 串扰。
    """

    def _on_channel_after(event) -> bool:
        return (
            event.kind == "payload"
            and event.data["channel"] == REQ_CHANNEL
            and (after is None or event.t > after)
        )

    def matches(event) -> bool:
        if not _on_channel_after(event):
            return False
        got_id, _ = _decode_payload_request_id(event)
        return got_id == request_id

    if not expect:
        try:
            event = bot.wait_for(
                _on_channel_after, timeout, f"任何 {REQ_CHANNEL} payload（负向窗口）"
            )
        except BotAssertionError:
            return None
        got_id, note = _decode_payload_request_id(event)
        detail = note if note is not None else f"request_id={got_id!r}"
        raise BotAssertionError(
            f"不应出现任何 {REQ_CHANNEL} payload（拒绝路径漏下发），实际收到：{detail}"
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
        got_id, _ = _decode_payload_request_id(event)
        return got_id == request_id

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
    if "reason" not in payload:
        raise BotAssertionError(f"{CLOSE_CHANNEL} 缺必填 reason 字段：{payload!r}")
    if payload.get("reason") != reason:
        raise BotAssertionError(
            f"{CLOSE_CHANNEL} reason 应为 {reason!r}（None=Replaced 静默），"
            f"实际 {payload.get('reason')!r}"
        )
    return payload


def expect_redis_response(
    redis: RedisClient,
    request_id: str,
    timeout: float = 15.0,
    expect: bool = True,
    **want,
) -> dict | None:
    """等 bong:agent_ui_response 上 request_id 匹配的消息；断言 action/params（子集或精确）。"""

    def matches(payload: dict) -> bool:
        return payload.get("request_id") == request_id and response_matches(
            payload, **want
        )

    got = redis.wait_message(RESP_CHANNEL, matches, timeout=timeout, expect=expect)
    if got is None and expect:
        raise AssertionError(f"request_id={request_id} 的 {RESP_CHANNEL} 回执未出现")
    return got


def expect_no_redis_response(
    redis: RedisClient, request_id: str, timeout: float = 2.0
) -> None:
    """负向断言：request_id 匹配的 bong:agent_ui_response 在窗口内不应出现。"""
    got = expect_redis_response(redis, request_id, timeout=timeout, expect=False)
    if got is not None:
        raise BotAssertionError(
            f"不应出现 request_id={request_id} 的 {RESP_CHANNEL} 回执，实际收到 {got!r}"
        )
