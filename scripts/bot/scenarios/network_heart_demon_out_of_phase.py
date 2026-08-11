"""`heart_demon_decision` 非心魔相下发：server 必须优雅忽略，不崩溃不误伤。

黑盒断言面：
- C2S `heart_demon_decision{v, choice_idx}` 在**无渡劫**状态下发出（choice_idx=2 / 缺省 /
  未知字段），`client_request_handler.rs` 照常派发 `HeartDemonChoiceSubmitted`，
  `tribulation.rs::heart_demon_choice_system` 因 phase 非 HeartDemon 或玩家无
  TribulationState 而跳过——不得产生任何 tribulation_state / heart_demon_offer payload，
  连接保持、玩家存活。
- 未知字段形状（deny_unknown_fields）被 server 拒收并留 warn log，同样不得断连。
"""

from __future__ import annotations

import time

from bot.bot import Bot
from bot.scenarios._combat_helpers import wait_for_ready

DESCRIPTION = "非心魔相下发 heart_demon_decision（含未知字段）：server 优雅忽略，连接保持"
MODULES = ["cultivation", "tribulation", "network"]

HEART_DEMON_DECISION = {"type": "heart_demon_decision", "v": 1}


def _no_tribulation_payloads_arrive(bot: Bot, within: float = 3.0) -> None:
    """断言在 within 秒静默窗内不出现 tribulation_state / heart_demon_offer payload。

    负向异步断言必须做窗末原子观测：循环先**完成一次扫描**再判 deadline，deadline 到达的
    那一轮也要扫过（否则恰好落在窗尾的 payload 会被漏掉）；扫描在 _lock 下做快照，
    reader 线程异步追加，不加锁迭代可能读到半更新列表。
    """
    deadline = time.monotonic() + within
    while True:
        with bot._lock:
            snapshot = list(bot.events)
        for event in snapshot:
            if event.kind != "server_data":
                continue
            payload_type = event.data.get("payload_type")
            if payload_type in ("tribulation_state", "heart_demon_offer"):
                raise AssertionError(
                    f"[{bot.username}] 非心魔相下发 decision 后不应出现 {payload_type} payload："
                    f"{event.data.get('payload')!r}"
                )
        if time.monotonic() >= deadline:
            return
        time.sleep(0.2)


def run(env) -> None:
    with env.new_bot("J1") as bot:
        wait_for_ready(bot)

        # 无渡劫状态下逐个发：合法 idx、缺省 idx、未知字段（server 拒收留 log）。
        for request in (
            {**HEART_DEMON_DECISION, "choice_idx": 2},
            HEART_DEMON_DECISION,
            {**HEART_DEMON_DECISION, "choice_idx": 1, "unknown_field": 7},
        ):
            bot.intent(request)
            _no_tribulation_payloads_arrive(bot)
            bot.assert_alive(f"下发 {request!r} 后连接应保持")

        # 拒绝路径同样不得产生副作用 payload。
        _no_tribulation_payloads_arrive(bot, within=5.0)
