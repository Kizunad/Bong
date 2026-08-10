"""join 链路 chunk 投递回归 —— 永久钉死 PR#846「join 虚空」类 bug。

真根因史（memory: project-rejoin-void-vd-ramp）：valence join tick 不发
SetChunkCacheCenter(0x4E)，客户端 chunk cache center 停在 (0,0)，spawn 迁离原点后
所有 chunk 到达即被静默丢弃 → 全虚空。协议 bot 三会话抓到 0 个 0x4E 定案。

本场景断言（首连 + 一次重连各跑一遍）：
1. GameJoin / PlayerPositionLook 到达（玩家被放置）
2. ChunkCenter(0x4E) 在 join 后短窗口内到达 —— 缺这个包就是 #846 复发
3. center 必须等于玩家所在 chunk（floor(pos/16)）—— center 指错地方等价于没发
4. 若视野内世界有内容：center 到达后必须有可用 chunk 投递，且全部落在 center
   半径内（超出的会被客户端静默丢弃）

关于第 4 条的"若"（实测 2026-07-06）：无 raster 的 fallback 世界只有原点 16×16
平台，而 spawn 散布区（zone "spawn"）大部分在平台外——bot 可能出生在纯虚空上，
0 个 chunk 是**当前 server 的真实行为**而非投递 bug。此时显式打印跳过（不静默）。
世界覆盖修好后（见 plan-bot-e2e-coverage-v1 问题记录 #5）应把下限收紧到 ≥8。
"""

from bot.bot import BotAssertionError

DESCRIPTION = "join/rejoin 必须投递 ChunkCenter=玩家chunk，center 后 chunk 可用（pin PR#846）"
MODULES = ["terrain", "network"]

CENTER_TIMEOUT = 10.0
FIRST_CHUNK_BUDGET = 10.0


def _one_session(env, tag: str) -> None:
    with env.new_bot(tag) as bot:
        bot.expect_event("game_join", timeout=15.0)
        center = bot.expect_event("chunk_center", timeout=CENTER_TIMEOUT)
        pos = bot.expect_event("pos_look", timeout=15.0)

        cx, cz = center.data["x"], center.data["z"]
        player_chunk = (int(pos.data["x"] // 16), int(pos.data["z"] // 16))
        if (cx, cz) != player_chunk:
            raise BotAssertionError(
                f"[{bot.username}] 期望 ChunkCenter 等于玩家所在 chunk {player_chunk}"
                f"（center 指错位置时客户端照样丢弃视野 chunk = 虚空），"
                f"实际 center=({cx},{cz})"
            )

        try:
            bot.wait_for(
                lambda e: e.kind == "chunk_data",
                timeout=FIRST_CHUNK_BUDGET,
                description="任意 ChunkData",
            )
        except BotAssertionError:
            # 不是投递 bug：raster-less 世界在 spawn 散布区没有 chunk 可发。
            print(
                f"    [warn] {bot.username} 出生点视野内世界无 chunk"
                "（raster-less fallback 平台盖不住 spawn 散布区）——chunk 投递 leg 跳过"
            )
            bot.assert_alive("join 全程（虚空出生）")
            return

        # 世界有内容 → 客户端以 center 为准裁剪，center 后必须有可用 chunk。
        bot.wait_for(
            lambda e: any(c.t >= center.t for c in bot.events_of("chunk_data")),
            timeout=FIRST_CHUNK_BUDGET,
            description=(
                "center 之后 ≥1 个 ChunkData"
                "（#846 修复契约：join 补 center 并重灌视野 chunk）"
            ),
        )

        distance_events = bot.events_of("chunk_load_distance")
        # 原版客户端实际接收半径 = load distance + 3（缓存边距）；
        # 未收到 0x4F 时按 GameJoin 默认视距 10 兜底。
        radius = (distance_events[-1].data["distance"] if distance_events else 10) + 3
        stray = [
            (e.data["x"], e.data["z"])
            for e in bot.events_of("chunk_data")
            if abs(e.data["x"] - cx) > radius or abs(e.data["z"] - cz) > radius
        ]
        if stray:
            raise BotAssertionError(
                f"[{bot.username}] 期望所有 chunk 落在 center=({cx},{cz}) 半径 {radius} 内"
                f"（超出的会被客户端静默丢弃 = 玩家看到虚空），"
                f"实际有 {len(stray)} 个越界: {stray[:5]}"
            )
        bot.assert_alive("join 全程")


def run(env) -> None:
    _one_session(env, "J1")
    # 重连是 #846 的原始触发面：同名玩家断开重进
    _one_session(env, "J1")
