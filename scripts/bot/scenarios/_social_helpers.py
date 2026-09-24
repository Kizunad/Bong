"""Shared assertions for social (sparring / trade) bot scenarios."""

from __future__ import annotations

from bot.bot import BotAssertionError
from bot.mc_protocol import offline_uuid


def wait_player_protocol_id(observer, username, timeout: float = 15.0) -> int:
    """发现另一名玩家的 MC protocol entity id（按其 offline UUID 精确匹配）。

    game_join 的 entity_id 恒为 0（valence 保留给客户端自身），无法作 target；
    目标玩家的真实 protocol id 只在 PlayerSpawnS2c 里下发，且只有进入观察者
    视野时才会推送。NPC 也有 spawn 包，不能靠坐标猜——server 身份由用户名
    确定性导出（valence offline_uuid = sha256(username) 前 16 字节），
    用 PlayerSpawnS2c 携带的 UUID 精确辨认。
    """
    expected = offline_uuid(username)
    events = [
        e
        for e in observer.events_of("player_spawn")
        if e.data["uuid"] == expected
    ]
    if not events:
        events = [
            observer.wait_for(
                lambda e: e.kind == "player_spawn" and e.data["uuid"] == expected,
                timeout=timeout,
                description=f"玩家 {username} spawn（offline uuid {expected}）",
            )
        ]
    # 实体可能被销毁后重新入视野（chunk 重载 / 层实体复用），旧 spawn 的 id 会失效
    # 甚至被 NPC 占用；取最新一条 spawn 的 id 才是当前存活实体的 protocol id。
    return events[-1].data["entity_id"]
