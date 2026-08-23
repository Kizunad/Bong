"""P2/P5 多 Bot：协议身份互见、共享 NPC 身份、同一靶各自 typed outgoing hit。"""

from __future__ import annotations

import math
import time

from bot.bot import BotAssertionError
from bot.scenarios._combat_helpers import (
    is_outgoing_positive_hit,
    last_event_time,
    move_to_melee_range,
    queue_fight_target,
    queue_npc_scenario,
    wait_for_ready,
)

# CI 上这些等的是 server 对具体请求的响应，而不是本地计算：实测挂掉那次进程里已经
# 收了 3000~15000 条事件，10s 窗口在满载 runner 上不够用（同一 commit 重跑就换了个
# 场景挂）。放宽只在**失败**路径上多花时间——成功时 wait_for 收到事件立刻返回，
# 绿色用例的墙钟不变。
SERVER_RESPONSE_TIMEOUT = 25.0

# 近战重试预算单列，**不要**跟上面那个混用：实测把它从 10s/8 次放宽到 25s/20 次
# 仍然打不中（PR #2066 的 CI），瓶颈不在预算，调大只会让失败更慢。真因待查，
# 失败消息已带 _melee_failure_diagnosis 的现场判据。
MELEE_RETRY_TIMEOUT = 15.0
MELEE_DISTANCE = 1.2

DESCRIPTION = "两个 Bot 互见 PlayerSpawn，并对共同观察到的同一 passive NPC 各自产生 outgoing typed hit"
MODULES = ["network", "multibot", "combat", "npc"]


def _wait_for_peer(bot, peer_username: str):
    return bot.wait_for(
        lambda event: event.kind == "player_spawn"
        and (
            event.data.get("username") == peer_username
            or bot.player_names.get(event.data.get("uuid")) == peer_username
        ),
        timeout=SERVER_RESPONSE_TIMEOUT,
        description=f"PlayerList+PlayerSpawn 权威关联到在线 peer `{peer_username}`",
    )


def _rendezvous_in_spawn(bot) -> None:
    anchor = last_event_time(bot)
    bot.cmd("tpzone spawn")
    bot.wait_for(
        lambda event: event.kind == "chat"
        and event.t > anchor
        and event.data.get("text") == "Teleported to zone `spawn`.",
        timeout=SERVER_RESPONSE_TIMEOUT,
        description="/tpzone spawn 权威命令回执",
    )
    bot.wait_for(
        lambda event: event.kind == "pos_look" and event.t > anchor,
        timeout=SERVER_RESPONSE_TIMEOUT,
        description="/tpzone spawn 后 server 权威 PositionLook",
    )


def _melee_failure_diagnosis(bot, target_id: int) -> str:
    """把"打不中"拆成能判读的几种原因。

    实测（PR #2066 的 CI）把重试预算从 10s/8 次放宽到 25s/20 次**仍然**打不中，
    说明瓶颈根本不是预算——但旧的失败消息只说"重试 N 次未命中"，看不出到底是
    目标没了、够不着、还是打出去没反馈。这里把三者分开报，下次撞红直接可判。
    """
    target = bot.entity_pos(target_id)
    if target is None:
        return (
            f"目标 entity_id={target_id} 已不在 {bot.username} 的实体表里"
            "（destroy/despawn，或从未同步过）——目标先没了，再多重试也没用"
        )
    if bot.position is None:
        return f"目标在 {target}，但 {bot.username} 自己的 position 未知"
    bx, by, bz = bot.position
    tx, ty, tz = target
    flat = math.hypot(bx - tx, bz - tz)
    combat = [
        e for e in bot.events
        if e.kind == "server_data" and e.data.get("payload_type") == "combat_event"
    ]
    outgoing = sum(
        1
        for e in combat
        for entry in (e.data.get("payload") or {}).get("events", [])
        if isinstance(entry, dict) and entry.get("outgoing") is True
    )
    return (
        f"目标仍在 {target}，{bot.username} 在 {bot.position}，水平距离 {flat:.2f} 格"
        f"（贴身目标 {MELEE_DISTANCE} 格，垂直差 {abs(by - ty):.2f}）；"
        f"整场共收到 {len(combat)} 条 combat_event、其中 outgoing 条目 {outgoing} 个"
        "——距离超标说明贴身失败，距离达标但 outgoing=0 说明攻击包没被判定为命中"
    )


def _attack_until_positive_hit(
    bot, spawn, target_id: int, timeout: float = MELEE_RETRY_TIMEOUT
) -> None:
    """追踪同一协议实体重试近战，覆盖击退同步和服务端 10 tick GCD。"""
    # /tpzone 后玩家可能仍在从传送高度落向地面；这次必要的真实定位不应
    # 抢占攻击重试预算，否则首轮移动就会耗尽整个窗口。
    move_to_melee_range(bot, spawn, MELEE_DISTANCE)
    deadline = time.monotonic() + timeout
    attempts = 0
    while time.monotonic() < deadline:
        anchor = last_event_time(bot)
        bot.attack_entity(target_id)
        attempts += 1
        remaining = deadline - time.monotonic()
        try:
            bot.wait_for(
                lambda event: event.t > anchor and is_outgoing_positive_hit(event),
                timeout=min(0.55, max(0.01, remaining)),
                description="同一目标的 outgoing=true positive hit",
            )
            return
        except BotAssertionError:
            # 失败攻击可能是目标击退位置尚未同步，或仍在 10 tick GCD；下一轮
            # 重新读取 entity_pos，禁止用旧 spawn 坐标反复猜测。
            if time.monotonic() >= deadline:
                break
            time.sleep(min(0.55, deadline - time.monotonic()))
            if time.monotonic() < deadline:
                move_to_melee_range(bot, spawn, MELEE_DISTANCE)

    raise BotAssertionError(
        f"[{bot.username}] 在 {timeout:.1f}s 内对同一 target_id={target_id} "
        f"重试 {attempts} 次仍未收到 outgoing=true positive hit；"
        f"{_melee_failure_diagnosis(bot, target_id)}"
    )


def run(env) -> None:
    with env.new_bot("MCA") as alice:
        wait_for_ready(alice)

        with env.new_bot("MCB") as bob:
            wait_for_ready(bob)

            # 单 Bot 宽容场景仍故意省略 ClientSettings；多实体观察场景则显式声明正常
            # 客户端视距，避免把 Valence 的最小默认 tracking 半径误当成 gameplay 身份缺失。
            alice.send_client_settings(view_distance=10)
            bob.send_client_settings(view_distance=10)

            # spawn_distribution 会把不同用户名分散到多个相距数百格的出生簇；PlayerList
            # 是全服身份表，但 PlayerSpawn 只在实体视野内发送。先用 dev-only tpzone 把两端
            # 权威重定位到同一个 rendezvous，并要求各自收到新的 PositionLook；后续身份、
            # 共享靶与伤害证据仍全部来自真实协议包。
            _rendezvous_in_spawn(alice)
            _rendezvous_in_spawn(bob)

            alice_peer = _wait_for_peer(alice, bob.username)
            bob_peer = _wait_for_peer(bob, alice.username)
            if alice_peer.data["entity_id"] == alice.entity_id:
                raise BotAssertionError(
                    "Alice 观察到的 Bob PlayerSpawn 不得复用 Alice 自身 entity_id；"
                    f"actual={alice_peer.data['entity_id']}"
                )
            if bob_peer.data["entity_id"] == bob.entity_id:
                raise BotAssertionError(
                    "Bob 观察到的 Alice PlayerSpawn 不得复用 Bob 自身 entity_id；"
                    f"actual={bob_peer.data['entity_id']}"
                )

            # scenario command will clear earlier scenario fixtures and create one protocol-visible,
            # stationary real-combat NPC. Both clients must observe the same protocol entity ID.
            queue_npc_scenario(alice, "clear")
            alice_spawn = queue_fight_target(alice)
            target_id = int(alice_spawn.data["entity_id"])
            bob_spawn = bob.wait_for(
                lambda event: event.kind == "entity_spawn"
                and event.data.get("entity_id") == target_id,
                timeout=SERVER_RESPONSE_TIMEOUT,
                description=f"Bob 观察到 Alice 所见的同一 passive NPC entity_id={target_id}",
            )
            if alice_spawn.data["uuid"] != bob_spawn.data["uuid"]:
                raise BotAssertionError(
                    "共享 NPC 必须同时对拍协议 entity_id 与 UUID，不能仅凭附近另一个 zombie；"
                    f"alice_uuid={alice_spawn.data['uuid']} bob_uuid={bob_spawn.data['uuid']}"
                )

            move_to_melee_range(alice, alice_spawn, MELEE_DISTANCE)
            alice_anchor = last_event_time(alice)
            alice.attack_entity(target_id)
            alice.wait_for(
                lambda event: event.t > alice_anchor and is_outgoing_positive_hit(event),
                timeout=SERVER_RESPONSE_TIMEOUT,
                description="Alice 的专属 outgoing=true positive hit",
            )
            alice.wait_for(
                lambda event: event.kind == "entity_move"
                and event.t > alice_anchor
                and event.data.get("entity_id") == target_id,
                timeout=SERVER_RESPONSE_TIMEOUT,
                description=f"Alice 命中后精确共享 NPC entity_id={target_id} 产生 knockback",
            )

            # Bob 已通过同一 server 权威 rendezvous 与 Alice/目标共处；按 Bob 观察到的
            # 真实目标坐标走 C2S 移动包贴身，不使用协议外坐标或本地瞬移猜测。
            _attack_until_positive_hit(bob, bob_spawn, target_id)

            alice.assert_alive("互见身份并命中共享 NPC 后")
            bob.assert_alive("互见身份并命中共享 NPC 后")
