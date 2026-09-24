"""无效目标夺舍拒绝路径：格式不对 / target 不存在的请求必须是静默 no-op。

黑盒断言面：
- C2S：`bot.intent({"type":"duo_she_request","v":1,"target_id":...})`
- 结果：`server/src/cultivation/possession.rs::process_duo_she_requests`——
  `resolve_target_snapshot` 查不到目标即 `continue`（无事件、无标记、无 Despawned），
  host 与同服第三人连接均保持；cooldown 也不被 mark（30min 冷却只记成功）。

两种无效输入都锁：
1. `offline:<bystander>` 二段式 canonical id——真实玩家但格式与运行时完整
   character_id（`offline:<user>:<uuid>`）不匹配，必须静默拒绝（实测锁定，
   见 cultivation_duo_she_player_target.py 的 target_id 契约说明）。
2. `offline:<不存在>`——查无此 character，同样静默 no-op。

断言：每次请求后等待窗口内两 bot 均存活、无任何连接终止；`bong:duo_she_event`
不应发布（由 harness 侧 redis SUBSCRIBE 证据兜底，见 DONE-W6-BOTSCEN-GAP2.md）。
"""

from __future__ import annotations

import time

DESCRIPTION = "无效 target_id 夺舍静默拒绝：两 bot 连接保持、无人被终结"
MODULES = ["cultivation", "multibot", "network"]

REQUEST = {"type": "duo_she_request", "v": 1}
NO_SUCH_TARGET = "offline:no_such_player_botscen"
SILENT_WINDOW = 6.0


def _assert_silent_rejection(host, bystander, target_id: str, label: str) -> None:
    sent_at = host.events[-1].t if host.events else 0.0
    host.intent({**REQUEST, "target_id": target_id})

    # 静默窗口：无效目标不应终结/踢任何人。
    time.sleep(SILENT_WINDOW)
    host.assert_alive(f"无效夺舍请求后（{label}）")
    bystander.assert_alive(f"无效夺舍请求后（{label}，bystander 不应被误杀）")

    # 双保险：窗口内确实没有发生过任何连接终止事件。
    ended = [
        e
        for e in host.events_of("connection_lost") + host.events_of("disconnect")
        if e.t > sent_at
    ]
    if ended:
        raise AssertionError(
            f"[{host.username}] 无效 target_id {target_id!r}（{label}）不应触发连接终止，"
            f"实际窗口内出现 {ended!r}——resolve_target_snapshot 拒绝路径疑似失效"
        )


def run(env) -> None:
    with env.new_bot("GD2I") as host:
        host.expect_event("game_join", timeout=15.0)
        host.expect_event("pos_look", timeout=15.0)

        with env.new_bot("GD2J") as bystander:
            bystander.expect_event("game_join", timeout=15.0)
            bystander.expect_event("pos_look", timeout=15.0)

            _assert_silent_rejection(
                host,
                bystander,
                f"offline:{bystander.username}",
                "二段式 canonical id（真实玩家、格式不匹配）",
            )
            _assert_silent_rejection(
                host,
                bystander,
                NO_SUCH_TARGET,
                "不存在的 character_id",
            )
