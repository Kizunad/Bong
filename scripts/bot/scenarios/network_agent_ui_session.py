"""天道 UI 会话生命周期（HEADLESSAUDIT 优先项 3：agent_ui_response）。

协议链：Redis bong:agent_ui_cmd（agent 侧注入，本场景用 bot 侧 Redis 直发模拟）
→ server 校验/清洗 → S2C bong:agent_ui_request 专属 JSON 通道 → C2S
bong:client_request `agent_ui_response` → server 校验 request_id/allowed_button_ids
→ Redis bong:agent_ui_response 转发给 agent。

锁定的行为：
- button_click（白名单内）→ 转发 button_click + params.button_id 原样回执
- dismissed → 转发 dismissed
- parse_error（client 上报）→ 原样转发整个 params（非 button_click 不查白名单）
- 无响应 → server ticker 权威超时 → Timeout 终态回执
- 新 cmd 替换旧 session → 旧 session Replaced 回执 + S2C 静默 close(reason=null)
- 断线 → Dismissed 终态回执
- 终态回执 exactly-once：timeout / replaced / 断线 dismissed 每个 request_id 只发一次
- S2C 下发的 payload 绝不含安全字段 realm_gate / allowed_button_ids

每个探针用唯一 request_id（含 run_tag），Redis 订阅只按 request_id 过滤，防串扰。
"""

from __future__ import annotations

from bot._agent_ui_helpers import (
    RESP_CHANNEL,
    RedisClient,
    expect_agent_ui_close,
    expect_agent_ui_request,
    expect_no_redis_response,
    expect_redis_response,
    publish_cmd,
)

DESCRIPTION = "天道 UI 会话：button_click/dismissed/parse_error 转发、timeout、替换、断线 Dismissed"
MODULES = ["network", "agent_ui"]


def _mark(bot) -> float:
    return bot.events[-1].t if bot.events else 0.0


def _new_session(bot, redis, target_player, run_tag, phase: str, **overrides):
    """注入 cmd + 等 S2C 下发；返回 (request_id, cmd)。"""
    request_id = f"gap3_{run_tag}_{phase}"
    mark = _mark(bot)
    cmd = publish_cmd(
        redis,
        request_id=request_id,
        target_player=target_player,
        **overrides,
    )
    payload = expect_agent_ui_request(bot, request_id, after=mark, timeout=15.0)
    if payload["target_player"] != target_player:
        raise AssertionError(
            f"request_id={request_id} 的 target_player 应为 {target_player!r}，"
            f"实际 {payload['target_player']!r}"
        )
    return request_id, cmd


def _click(bot, request_id: str, button_id: str) -> None:
    bot.intent(
        {
            "type": "agent_ui_response",
            "v": 1,
            "request_id": request_id,
            "action": "button_click",
            "params": {"button_id": button_id},
        }
    )


def _happy_button_click(bot, redis, target_player, run_tag) -> None:
    request_id, cmd = _new_session(bot, redis, target_player, run_tag, "click")
    _click(bot, request_id, cmd["allowed_button_ids"][0])
    got = expect_redis_response(
        redis,
        request_id,
        action="button_click",
        params_subset={"button_id": cmd["allowed_button_ids"][0]},
    )
    if got["request_id"] != request_id:
        raise AssertionError(f"button_click 回执 request_id 漂移：{got!r}")


def _dismissed(bot, redis, target_player, run_tag) -> None:
    request_id, _ = _new_session(bot, redis, target_player, run_tag, "dismiss")
    bot.intent(
        {
            "type": "agent_ui_response",
            "v": 1,
            "request_id": request_id,
            "action": "dismissed",
        }
    )
    expect_redis_response(redis, request_id, action="dismissed", params_subset={})


def _parse_error(bot, redis, target_player, run_tag) -> None:
    request_id, _ = _new_session(bot, redis, target_player, run_tag, "parseerr")
    bot.intent(
        {
            "type": "agent_ui_response",
            "v": 1,
            "request_id": request_id,
            "action": "parse_error",
            "params": {"reason": "owo_parse_failed"},
        }
    )
    # 非 button_click 动作原样转发（含 params），不查 allowed_button_ids——
    # 原样契约必须精确匹配整个 params，额外字段（如注入 button_id）即违约。
    expect_redis_response(
        redis,
        request_id,
        action="parse_error",
        params_exact={"reason": "owo_parse_failed"},
    )


def _timeout(bot, redis, target_player, run_tag) -> None:
    request_id, _ = _new_session(
        bot, redis, target_player, run_tag, "timeout", timeout_ticks=20
    )
    # 不回 C2S：等 server ticker 权威超时（20 ticks ≈ 1s @20tps，留 15s 余量）
    got = expect_redis_response(redis, request_id, timeout=15.0, action="timeout")
    if got["params"] not in ({}, None):
        raise AssertionError(f"timeout 回执应无 params，实际 {got!r}")
    # 终态契约 exactly-once：同一 request_id 不得再出现第二条回执
    expect_no_redis_response(redis, request_id, timeout=2.0)


def _replaced(bot, redis, target_player, run_tag) -> None:
    old_id, _ = _new_session(bot, redis, target_player, run_tag, "repl_old")
    mark = _mark(bot)
    new_id, new_cmd = _new_session(bot, redis, target_player, run_tag, "repl_new")
    # 旧 session 被替换：Redis Replaced 回执 + S2C 静默 close（reason=null）
    expect_redis_response(
        redis, old_id, timeout=15.0, action="replaced", params_subset={}
    )
    # 终态契约 exactly-once：同一 request_id 不得再出现第二条回执
    expect_no_redis_response(redis, old_id, timeout=2.0)
    expect_agent_ui_close(bot, old_id, reason=None, after=mark, timeout=15.0)
    # 新 session 仍可正常点击（替换不影响新 session 生命周期）
    _click(bot, new_id, new_cmd["allowed_button_ids"][0])
    expect_redis_response(
        redis,
        new_id,
        action="button_click",
        params_subset={"button_id": new_cmd["allowed_button_ids"][0]},
    )


def _disconnect_dismissed(bot, redis, target_player, run_tag) -> None:
    request_id, _ = _new_session(bot, redis, target_player, run_tag, "disconn")
    bot.close()
    expect_redis_response(redis, request_id, timeout=15.0, action="dismissed")
    # 终态契约 exactly-once：同一 request_id 不得再出现第二条回执
    expect_no_redis_response(redis, request_id, timeout=2.0)


def run(env) -> None:
    with env.new_bot("AgS") as bot:
        bot.expect_event("game_join", timeout=15.0)
        bot.expect_event("pos_look", timeout=15.0)
        target_player = f"offline:{bot.username}"

        redis = RedisClient()
        try:
            redis.subscribe(RESP_CHANNEL)
            _happy_button_click(bot, redis, target_player, env.run_tag)
            _dismissed(bot, redis, target_player, env.run_tag)
            _parse_error(bot, redis, target_player, env.run_tag)
            _timeout(bot, redis, target_player, env.run_tag)
            _replaced(bot, redis, target_player, env.run_tag)
            _disconnect_dismissed(bot, redis, target_player, env.run_tag)
        finally:
            redis.close()
