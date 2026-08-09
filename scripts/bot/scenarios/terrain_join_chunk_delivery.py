"""join 链路 chunk 投递回归 —— 永久钉死 PR#846「join 虚空」类 bug。

本场景同时作为 raster-less fallback 世界的协议级验收：只有 harness 明确证明本轮
self-start server 没有 raster/Anvil 输入且命中 BOT_FALLBACK_FLAT_READY，场景才运行。
它以三个稳定 tag 覆盖三个 spawn_distribution 簇，并保留同名重连 leg。
"""

import math
import os

from bot.bot import BotAssertionError

DESCRIPTION = "owned fallback 世界三个出生簇 join/rejoin 均投递正确 center 与至少 8 个 chunk"
MODULES = ["terrain", "network"]
DEFAULT_ENABLED = False
REQUIRED_ENV = "BOT_E2E_FALLBACK_OWNED"
RUN_IN_ALL_WHEN_ENV = REQUIRED_ENV

CENTER_TIMEOUT = 10.0
CHUNK_BUDGET = 10.0
MIN_CHUNKS_AFTER_CENTER = 8

# BOT_E2E_RUN_TAG=ci is part of the CI witness: these full usernames are selected by the same
# production FNV seed path as PlayerSpawnSelector and cover all three configured clusters.
EXPECTED_CI_CLUSTERS = {
    "J1": ((180.0, 140.0), 112.0, "east"),
    "J2": ((-240.0, -160.0), 96.0, "west"),
    "FC": ((24.0, -24.0), 80.0, "central"),
}

# 同一生产 FNV 种子的精确产出（x, z），由 Rust 测试
# ci_bot_tags_cover_all_three_clusters_in_distinct_chunks 用真实
# spawn_selector::select 逐位复核；test_protocol.py 的
# _mirror_select 必须复现这些值。若 Rust 选择数学漂移，Rust 测试撞红；
# 若 Python 镜像漂移，test_protocol.py 撞红——镜像不再自证。
EXPECTED_CI_SPAWN_POSITIONS = {
    "J1": (95.61722776093924, 209.19172692675917),
    "J2": (-312.3280904906235, -100.69280549134928),
    "FC": (-36.23367324735134, 25.390079995465705),
}


def _assert_expected_cluster(
    run_tag: str,
    tag: str,
    position: tuple[float, float],
) -> str:
    if run_tag != "ci":
        raise BotAssertionError(
            "fallback multi-cluster witness requires BOT_E2E_RUN_TAG=ci so the exact production "
            f"usernames BciJ1/BciJ2/BciFC are pinned; actual run_tag={run_tag!r}"
        )
    try:
        (anchor_x, anchor_z), radius, cluster = EXPECTED_CI_CLUSTERS[tag]
    except KeyError as error:
        raise BotAssertionError(f"unknown fallback cluster tag {tag!r}") from error

    x, z = position
    distance = math.hypot(x - anchor_x, z - anchor_z)
    if distance > radius + 1e-9:
        raise BotAssertionError(
            f"[B{run_tag}{tag}] expected {cluster} spawn cluster centered at "
            f"({anchor_x},{anchor_z}) radius={radius}, actual position=({x},{z}) "
            f"distance={distance:.3f}"
        )
    return cluster


def _one_session(env, tag: str) -> tuple[tuple[int, int], str]:
    with env.new_bot(tag) as bot:
        bot.expect_event("game_join", timeout=15.0)
        center = bot.expect_event("chunk_center", timeout=CENTER_TIMEOUT)
        pos = bot.expect_event("pos_look", timeout=15.0)

        cx, cz = center.data["x"], center.data["z"]
        player_chunk = (math.floor(pos.data["x"] / 16), math.floor(pos.data["z"] / 16))
        cluster = _assert_expected_cluster(
            env.run_tag,
            tag,
            (float(pos.data["x"]), float(pos.data["z"])),
        )
        if (cx, cz) != player_chunk:
            raise BotAssertionError(
                f"[{bot.username}] 期望 ChunkCenter 等于玩家所在 chunk {player_chunk}"
                f"（center 指错位置时客户端照样丢弃视野 chunk = 虚空），"
                f"实际 center=({cx},{cz})"
            )

        bot.wait_for(
            lambda _event: len(
                [chunk for chunk in bot.events_of("chunk_data") if chunk.t >= center.t]
            )
            >= MIN_CHUNKS_AFTER_CENTER,
            timeout=CHUNK_BUDGET,
            description=(
                f"center 之后 ≥{MIN_CHUNKS_AFTER_CENTER} 个 ChunkData"
                "（fallback 平台必须覆盖真实 spawn_distribution + 醒灵视域）"
            ),
        )

        distance_events = bot.events_of("chunk_load_distance")
        # 原版客户端实际接收半径 = load distance + 3（缓存边距）；
        # 未收到 0x4F 时按 GameJoin 默认视距 10 兜底。
        radius = (distance_events[-1].data["distance"] if distance_events else 10) + 3
        stray = [
            (event.data["x"], event.data["z"])
            for event in bot.events_of("chunk_data")
            if event.t >= center.t
            and (
                abs(event.data["x"] - cx) > radius
                or abs(event.data["z"] - cz) > radius
            )
        ]
        if stray:
            raise BotAssertionError(
                f"[{bot.username}] 期望所有 center 后 chunk 落在 center=({cx},{cz})"
                f" 半径 {radius} 内，实际有 {len(stray)} 个越界: {stray[:5]}"
            )
        bot.assert_alive("owned fallback join 全程")
        return player_chunk, cluster


def run(env) -> None:
    if os.environ.get(REQUIRED_ENV) != "1":
        raise BotAssertionError(
            f"本场景只能由 self-start fallback harness 执行：需 {REQUIRED_ENV}=1"
        )

    # BOT_E2E_RUN_TAG=ci 时，J1/J2/FC 分别稳定落在 east/west/central 三个配置簇。
    first_sessions = {
        tag: _one_session(env, tag)
        for tag in ("J1", "J2", "FC")
    }
    first_chunks = {session[0] for session in first_sessions.values()}
    clusters = {session[1] for session in first_sessions.values()}
    if clusters != {"east", "west", "central"}:
        raise BotAssertionError(
            "三个稳定 Bot tag 必须精确命中 east/west/central 出生簇，"
            f"实际={first_sessions}"
        )
    if len(first_chunks) != 3:
        raise BotAssertionError(
            "三个稳定 Bot tag 必须命中三个不同出生 chunk，"
            f"否则无法证明 fallback 覆盖多簇，实际={sorted(first_chunks)}"
        )

    # 同名玩家断开重进是 #846 原始触发面；production seed 还必须保持同一簇和 chunk。
    rejoin_chunk, rejoin_cluster = _one_session(env, "J1")
    expected_chunk, expected_cluster = first_sessions["J1"]
    if (rejoin_chunk, rejoin_cluster) != (expected_chunk, expected_cluster):
        raise BotAssertionError(
            "J1 重连必须稳定落回同一 production 出生点证据，"
            f"首轮={(expected_chunk, expected_cluster)} 重连={(rejoin_chunk, rejoin_cluster)}"
        )
