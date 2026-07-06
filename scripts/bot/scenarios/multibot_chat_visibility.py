"""P5 多 bot 并发：两个协议 Bot 同服互见实体与聊天广播。

不联跑 agent，不断言天道 narration；chat → narration 需要 agent/Redis 编排，
应由后续 coverage/bug plan 单独承接。本场景只锁住同 server 多连接下的基础
实体可见性和广播回流。
"""

from bot.bot import BotAssertionError

DESCRIPTION = "两个 Bot 同 server：互见 entity_spawn，A 发 chat，B 收到同一条广播文本"
MODULES = ["network", "multibot", "chat", "entity"]


MAX_PLAYER_SPAWN_DISTANCE_SQ = 8.0 * 8.0


def _event_mark(bot) -> float:
    return bot.events[-1].t if bot.events else 0.0


def _expect_entity_spawn_near(observer, target, after: float) -> None:
    if target.position is None:
        raise BotAssertionError(f"[{target.username}] 缺少 pos_look，无法做互见实体坐标断言")
    tx, ty, tz = target.position

    def near_target(event) -> bool:
        if event.kind != "entity_spawn" or event.t <= after:
            return False
        if observer.entity_id is not None and event.data["entity_id"] == observer.entity_id:
            return False
        dx = event.data["x"] - tx
        dy = event.data["y"] - ty
        dz = event.data["z"] - tz
        return dx * dx + dy * dy + dz * dz <= MAX_PLAYER_SPAWN_DISTANCE_SQ

    observer.wait_for(
        near_target,
        timeout=12.0,
        description=(
            f"{observer.username} 收到位于 {target.username} 坐标附近的 entity_spawn"
            "（多 bot 同服互见实体；只观察真实 S2C 包，不读 server 内部状态）"
        ),
    )


def run(env) -> None:
    with env.new_bot("MCA") as alice:
        alice.expect_event("game_join", timeout=15.0)
        alice.expect_event("pos_look", timeout=15.0)
        alice_spawn_mark = _event_mark(alice)

        with env.new_bot("MCB") as bob:
            bob.expect_event("game_join", timeout=15.0)
            bob.expect_event("pos_look", timeout=15.0)

            _expect_entity_spawn_near(alice, bob, after=alice_spawn_mark)
            _expect_entity_spawn_near(bob, alice, after=0.0)

            marker = f"bot-e2e-chat-{env.run_tag}"
            alice.chat(marker)
            bob.expect_chat(marker, timeout=10.0)

            alice.assert_alive("多 bot entity/chat 可见性检查后")
            bob.assert_alive("多 bot entity/chat 可见性检查后")
