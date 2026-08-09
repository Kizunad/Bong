"""天道 UI 拒绝路径（HEADLESSAUDIT 优先项 3：agent_ui_response 的拒绝面）。

协议链与 network_agent_ui_session.py 相同，本场景锁「拒绝/丢弃」路径：
- button_id 不在白名单 → Redis error 回执 {reason: invalid_button_id}
  + S2C close(invalid_button_id) 玩家可见反馈
- stale request_id → S2C close(session_expired) 防 UI 悬空；**不消费**旧 session
  （随后对旧 session 合法点击仍成功）；Redis 无 stale 回执
- realm_gate 门控（化虚+，bot 醒灵）→ Redis error 回执 {reason: realm_gate_rejected,
  player_realm, required_realm}；**不向 client 下发** request payload
- 目标玩家离线 → Redis error 回执 {reason: player_offline}
- cmd 校验失败（timeout_ticks 越界）→ Redis error 回执 {reason: invalid_command}
- cmd serde 拒绝（缺必填字段）→ redis_bridge 直接丢弃，**无任何回执**

正负断言成对：拒绝路径各自断言「该有的回执/close 有」「不该有的下发/回执没有」。
"""

from __future__ import annotations

from bot._agent_ui_helpers import (
    RESP_CHANNEL,
    RedisPubSub,
    expect_agent_ui_close,
    expect_agent_ui_request,
    expect_no_redis_response,
    expect_redis_response,
    publish_cmd,
)
from bot.bot import BotAssertionError

DESCRIPTION = "天道 UI 拒绝路径：invalid_button_id/stale/realm_gate/offline/invalid_command/serde-drop"
MODULES = ["network", "agent_ui"]

# 化虚+（worldview 六境界最高阶）：醒灵 bot 必被门控拒绝。
REALM_GATE_MAX = 6


def _mark(bot) -> float:
    return bot.events[-1].t if bot.events else 0.0


def _invalid_button_id(bot, redis, target_player, run_tag) -> None:
    request_id = f"gap3_{run_tag}_badbtn"
    mark = _mark(bot)
    publish_cmd(
        redis,
        request_id=request_id,
        target_player=target_player,
        allowed_button_ids=("ok",),
    )
    expect_agent_ui_request(bot, request_id, after=mark, timeout=15.0)
    bot.intent(
        {
            "type": "agent_ui_response",
            "v": 1,
            "request_id": request_id,
            "action": "button_click",
            "params": {"button_id": "not_allowed"},
        }
    )
    expect_redis_response(
        redis,
        request_id,
        action="error",
        params_subset={"reason": "invalid_button_id"},
    )
    expect_agent_ui_close(bot, request_id, reason="invalid_button_id", timeout=15.0)


def _stale_request_id(bot, redis, target_player, run_tag) -> None:
    request_id = f"gap3_{run_tag}_stale"
    mark = _mark(bot)
    publish_cmd(redis, request_id=request_id, target_player=target_player)
    expect_agent_ui_request(bot, request_id, after=mark, timeout=15.0)

    stale_id = f"gap3_{run_tag}_stale_no_such_session"
    bot.intent(
        {
            "type": "agent_ui_response",
            "v": 1,
            "request_id": stale_id,
            "action": "dismissed",
        }
    )
    # stale 响应：S2C close(session_expired) 防 UI 悬空 + Redis 无该 request_id 回执
    expect_agent_ui_close(bot, stale_id, reason="session_expired", timeout=15.0)
    expect_no_redis_response(redis, stale_id, timeout=2.0)

    # stale 响应不消费旧 session：合法点击旧 session 仍应成功回执
    bot.intent(
        {
            "type": "agent_ui_response",
            "v": 1,
            "request_id": request_id,
            "action": "button_click",
            "params": {"button_id": "enter_realm"},
        }
    )
    expect_redis_response(
        redis,
        request_id,
        action="button_click",
        params_subset={"button_id": "enter_realm"},
    )


def _realm_gate_rejected(bot, redis, target_player, run_tag) -> None:
    request_id = f"gap3_{run_tag}_gate"
    mark = _mark(bot)
    publish_cmd(
        redis,
        request_id=request_id,
        target_player=target_player,
        realm_gate=REALM_GATE_MAX,
    )
    got = expect_redis_response(
        redis,
        request_id,
        action="error",
        params_subset={
            "reason": "realm_gate_rejected",
            "player_realm": "1",
            "required_realm": str(REALM_GATE_MAX),
        },
    )
    if got["params"].get("player_realm") != "1":
        raise BotAssertionError(
            f"realm_gate 回执 player_realm 应为醒灵(1)，实际 {got!r}"
        )
    # 门控拒绝时 panel 绝不下发到 client
    expect_agent_ui_request(bot, request_id, after=mark, timeout=2.0, expect=False)


def _player_offline(bot, redis, run_tag) -> None:
    request_id = f"gap3_{run_tag}_offline"
    ghost = f"offline:B{run_tag}GHOST"
    publish_cmd(redis, request_id=request_id, target_player=ghost)
    expect_redis_response(
        redis,
        request_id,
        action="error",
        params_subset={"reason": "player_offline"},
    )


def _invalid_command(bot, redis, target_player, run_tag) -> None:
    request_id = f"gap3_{run_tag}_badcmd"
    publish_cmd(
        redis,
        request_id=request_id,
        target_player=target_player,
        timeout_ticks=5,  # 低于 20..=2400 下限 → cmd.validate() 失败
    )
    expect_redis_response(
        redis,
        request_id,
        action="error",
        params_subset={"reason": "invalid_command"},
    )


def _serde_dropped(bot, redis, target_player, run_tag) -> None:
    request_id = f"gap3_{run_tag}_serde"
    # 缺必填字段 allowed_button_ids → redis_bridge serde 拒绝，整包丢弃（无回执）
    redis.publish(
        "bong:agent_ui_cmd",
        '{"request_id":"%s","target_player":"%s","xml":"<owo-ui/>",'
        '"timeout_ticks":600,"realm_gate":0}' % (request_id, target_player),
    )
    expect_no_redis_response(redis, request_id, timeout=2.0)


def run(env) -> None:
    with env.new_bot("AgR") as bot:
        bot.expect_event("game_join", timeout=15.0)
        bot.expect_event("pos_look", timeout=15.0)
        target_player = f"offline:{bot.username}"

        redis = RedisPubSub()
        try:
            redis.subscribe(RESP_CHANNEL)
            _invalid_button_id(bot, redis, target_player, env.run_tag)
            _stale_request_id(bot, redis, target_player, env.run_tag)
            _realm_gate_rejected(bot, redis, target_player, env.run_tag)
            _player_offline(bot, redis, env.run_tag)
            _invalid_command(bot, redis, target_player, env.run_tag)
            _serde_dropped(bot, redis, target_player, env.run_tag)
            bot.assert_alive("全部 agent_ui 拒绝探针后")
        finally:
            redis.close()
