"""P2/P5 多 Bot：协议身份互见、共享 NPC 身份、同一靶各自 typed outgoing hit。"""

from __future__ import annotations

from bot.bot import BotAssertionError
from bot.scenarios._combat_helpers import (
    is_outgoing_positive_hit,
    last_event_time,
    move_to_melee_range,
    queue_fight_target,
    queue_npc_scenario,
    wait_for_ready,
)

DESCRIPTION = "两个 Bot 互见 PlayerSpawn，并对共同观察到的同一 passive NPC 各自产生 outgoing typed hit"
MODULES = ["network", "multibot", "combat", "npc"]


def _wait_for_peer(bot, peer_username: str):
    return bot.wait_for(
        lambda event: event.kind == "player_spawn"
        and (
            event.data.get("username") == peer_username
            or bot.player_names.get(event.data.get("uuid")) == peer_username
        ),
        timeout=15.0,
        description=f"PlayerList+PlayerSpawn 权威关联到在线 peer `{peer_username}`",
    )


def _rendezvous_in_spawn(bot) -> None:
    anchor = last_event_time(bot)
    bot.cmd("tpzone spawn")
    bot.wait_for(
        lambda event: event.kind == "chat"
        and event.t > anchor
        and event.data.get("text") == "Teleported to zone `spawn`.",
        timeout=10.0,
        description="/tpzone spawn 权威命令回执",
    )
    bot.wait_for(
        lambda event: event.kind == "pos_look" and event.t > anchor,
        timeout=10.0,
        description="/tpzone spawn 后 server 权威 PositionLook",
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
                timeout=15.0,
                description=f"Bob 观察到 Alice 所见的同一 passive NPC entity_id={target_id}",
            )
            if alice_spawn.data["uuid"] != bob_spawn.data["uuid"]:
                raise BotAssertionError(
                    "共享 NPC 必须同时对拍协议 entity_id 与 UUID，不能仅凭附近另一个 zombie；"
                    f"alice_uuid={alice_spawn.data['uuid']} bob_uuid={bob_spawn.data['uuid']}"
                )

            move_to_melee_range(alice, alice_spawn)
            alice_anchor = last_event_time(alice)
            alice.attack_entity(target_id)
            alice.wait_for(
                lambda event: event.t > alice_anchor and is_outgoing_positive_hit(event),
                timeout=10.0,
                description="Alice 的专属 outgoing=true positive hit",
            )
            alice.wait_for(
                lambda event: event.kind == "entity_move"
                and event.t > alice_anchor
                and event.data.get("entity_id") == target_id,
                timeout=10.0,
                description=f"Alice 命中后精确共享 NPC entity_id={target_id} 产生 knockback",
            )

            # Bob 已通过同一 server 权威 rendezvous 与 Alice/目标共处；按 Bob 观察到的
            # 真实目标坐标走 C2S 移动包贴身，不使用协议外坐标或本地瞬移猜测。
            move_to_melee_range(bob, bob_spawn)
            bob_anchor = last_event_time(bob)
            bob.attack_entity(target_id)
            bob.wait_for(
                lambda event: event.t > bob_anchor and is_outgoing_positive_hit(event),
                timeout=10.0,
                description="Bob 的专属 outgoing=true positive hit",
            )

            alice.assert_alive("互见身份并命中共享 NPC 后")
            bob.assert_alive("互见身份并命中共享 NPC 后")
