"""`heart_demon_decision` 非心魔相下发：server 必须优雅忽略，不崩溃不误伤。

黑盒断言面：
- C2S `heart_demon_decision{v, choice_idx}` 在**无渡劫**状态下发出（choice_idx=2 / 缺省 /
  未知字段），`client_request_handler.rs` 照常派发 `HeartDemonChoiceSubmitted`，
  `tribulation.rs::heart_demon_choice_system` 因 phase 非 HeartDemon 或玩家无
  TribulationState 而跳过——不得产生任何**属于本角色**的 tribulation_state /
  heart_demon_offer payload，连接保持、玩家存活。
- 未知字段形状（deny_unknown_fields）被 server 拒收并留 warn log，同样不得断连。

review finding major-5（out-of-phase 锚点/归属/保活）修复：
- **pre-send watermark**：每条请求下发前取 `last_event_time` 作锚点，静默窗只扫
  `t>锚点` 的事件——排除任何 pre-request 的历史广播污染。
- **角色归属**：tribulation_state / offer 按 `actor_name==本 bot 用户名` 或
  `char_id==offline:<user>:` 前缀归属；刚加入的 observer 合法收到**他人**活跃天劫
  快照不会被误判为"本请求被错误响应"。
- **request/response 保活控制**：末段发送一个确定性回显的 dev 命令并等其响应，证明
  post-request 派发/响应链路仍活着（仅 reader 线程存活不足以证明 server 仍在服务）。
"""

from __future__ import annotations

import time

from bot.bot import Bot
from bot.scenarios._combat_helpers import wait_for_ready

DESCRIPTION = "非心魔相下发 heart_demon_decision（含未知字段）：server 优雅忽略，连接保持"
MODULES = ["cultivation", "tribulation", "network"]

HEART_DEMON_DECISION = {"type": "heart_demon_decision", "v": 1}
TRIBULATION_PAYLOAD_TYPES = ("tribulation_state", "heart_demon_offer")


def _last_event_time(bot: Bot) -> float:
    with bot._lock:
        return bot.events[-1].t if bot.events else 0.0


def _attributable_to(bot: Bot, payload) -> bool:
    """只认**本角色**的 payload：排除其他玩家活跃天劫的正常广播。

    - `actor_name` 是 Username 组件（普发广播带本角色名）；
    - `char_id` 是 `offline:<username>:<uuid>` 复合形式（player_state 快照归属）。
    """
    if not isinstance(payload, dict):
        return False
    if payload.get("actor_name") == bot.username:
        return True
    char_id = payload.get("char_id") or ""
    return char_id.startswith(f"offline:{bot.username}:")


def _post_watermark_tribulation_events(bot: Bot, after: float):
    """t 严格大于 `after` 且归属本角色的 tribulation_state / heart_demon_offer 事件。"""
    with bot._lock:
        return [
            event
            for event in bot.events
            if event.t > after
            and event.kind == "server_data"
            and event.data.get("payload_type") in TRIBULATION_PAYLOAD_TYPES
            and _attributable_to(bot, event.data.get("payload"))
        ]


def _no_tribulation_payloads_arrive(bot: Bot, after: float, within: float = 3.0) -> None:
    """断言 after 锚点之后、within 秒静默窗内不出现**归属本角色**的天劫 payload。

    负向异步断言必须做窗末原子观测：循环先**完成一次扫描**再判 deadline，deadline 到达的
    那一轮也要扫过（否则恰好落在窗尾的 payload 会被漏掉）；扫描在 _lock 下做快照，
    reader 线程异步追加，不加锁迭代可能读到半更新列表。
    """
    deadline = time.monotonic() + within
    while True:
        events = _post_watermark_tribulation_events(bot, after)
        if events:
            event = events[0]
            raise AssertionError(
                f"[{bot.username}] 非心魔相下发 decision 后不应出现归属本角色的 "
                f"{event.data.get('payload_type')} payload：{event.data.get('payload')!r}"
            )
        if time.monotonic() >= deadline:
            return
        time.sleep(0.2)


def _keepalive_request_response(bot: Bot) -> None:
    """有效的 request/response 保活控制：下发确定性回显的 dev 命令并等回显。

    修 review finding major-5 的 assert_alive 盲区：assert_alive 只证明 reader 线程/底层
    socket 活着，不能证明 server 仍在处理本客户端请求。这里用一次真实请求→回显往返证明
    post-request 派发响应链路通畅。
    """
    after = _last_event_time(bot)
    bot.cmd("health set 100")
    bot.wait_for(
        lambda e: (
            e.kind == "chat"
            and "Queued /health set" in e.data.get("text", "")
            and e.t > after
        ),
        timeout=10.0,
        description="health set 的确定性回显（request/response 保活控制）",
    )


def run(env) -> None:
    with env.new_bot("J1") as bot:
        wait_for_ready(bot)

        # 无渡劫状态下逐个发：合法 idx、缺省 idx、未知字段（server 拒收留 log）。
        # 每条请求前取 watermark 锚点，静默窗只扫 t>锚点 且归属本角色的 payload。
        for request in (
            {**HEART_DEMON_DECISION, "choice_idx": 2},
            HEART_DEMON_DECISION,
            {**HEART_DEMON_DECISION, "choice_idx": 1, "unknown_field": 7},
        ):
            after = _last_event_time(bot)
            bot.intent(request)
            _no_tribulation_payloads_arrive(bot, after)
            bot.assert_alive(f"下发 {request!r} 后连接应保持")

        # 拒绝路径同样不得产生副作用 payload。
        after = _last_event_time(bot)
        _no_tribulation_payloads_arrive(bot, after, within=5.0)
        bot.assert_alive("静默窗结束后连接应保持")

        # 保活控制：真实请求→回显，证明 post-request 派发响应链路仍活着。
        _keepalive_request_response(bot)
