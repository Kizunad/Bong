"""P5b 多 Bot 组队渡劫：参与者登记与观礼距离邀请。

三个协议 Bot 共同进入 spawn：leader 发起渡虚劫，participant 在锁定阶段用真实
攻击包加入参与者，observer 先在近处收到观礼邀请、再移到远处观察邀请关闭。
状态快照仍按现有服务器协议对所有连接可见；本场景只锁定参与者名单和广播 payload
中的距离字段，不改变服务器分发行为。
"""

from __future__ import annotations

import time

from bot.bot import BotAssertionError


DESCRIPTION = "三 Bot 渡虚劫：队友加入 participants，近处/远处 observer 的观礼邀请按距离变化"
MODULES = ["cultivation", "network", "multibot"]

START_DU_XU = {"type": "start_du_xu", "v": 1}
DUXU_OMEN_WAIT_SECONDS = 75.0
DUXU_LOCK_WAIT_SECONDS = 40.0
PARTICIPANT_ATTACK_ATTEMPTS = 6
PARTICIPANT_ATTACK_INTERVAL_SECONDS = 0.6
OBSERVER_FAR_OFFSET_BLOCKS = 60.0


def _wait_ready(bot) -> None:
    bot.expect_event("game_join", timeout=15.0)
    bot.expect_event("pos_look", timeout=15.0)
    bot.send_client_settings(view_distance=10)


def _rendezvous_in_spawn(bot) -> None:
    anchor = bot.events[-1].t if bot.events else 0.0
    bot.cmd("tpzone spawn")
    bot.wait_for(
        lambda event: (
            event.kind == "chat"
            and event.t > anchor
            and event.data.get("text") == "Teleported to zone `spawn`."
        ),
        timeout=10.0,
        description="/tpzone spawn 权威命令回执",
    )
    bot.wait_for(
        lambda event: event.kind == "pos_look" and event.t > anchor,
        timeout=10.0,
        description="/tpzone spawn 后的权威位置",
    )


def _wait_peer_spawn(bot, username: str):
    return bot.wait_for(
        lambda event: (
            event.kind == "player_spawn"
            and (
                event.data.get("username") == username
                or bot.player_names.get(event.data.get("uuid")) == username
            )
        ),
        timeout=20.0,
        description=f"观察到 peer `{username}` 的 PlayerSpawn 身份与坐标",
    )


def _wait_tribulation_phase(bot, phase: str, after: float, timeout: float):
    return bot.wait_for(
        lambda event: (
            event.kind == "server_data"
            and event.t > after
            and event.data.get("payload_type") == "tribulation_state"
            and event.data.get("payload", {}).get("phase") == phase
        ),
        timeout=timeout,
        description=f"t>{after:.3f}s 后收到 tribulation_state phase={phase}",
    )


def _wait_broadcast(bot, stage: str, after: float, timeout: float):
    return bot.wait_for(
        lambda event: (
            event.kind == "server_data"
            and event.t > after
            and event.data.get("payload_type") == "tribulation_broadcast"
            and event.data.get("payload", {}).get("stage") == stage
        ),
        timeout=timeout,
        description=f"t>{after:.3f}s 后收到 tribulation_broadcast stage={stage}",
    )


def run(env) -> None:
    with env.new_bot("DuxL") as leader:
        _wait_ready(leader)
        _rendezvous_in_spawn(leader)

        with env.new_bot("DuxP") as participant:
            _wait_ready(participant)
            _rendezvous_in_spawn(participant)

            with env.new_bot("DuxO") as observer:
                _wait_ready(observer)
                _rendezvous_in_spawn(observer)

                # 先用 peer 的真实 PlayerSpawn 坐标集结，不读取 server 状态或自行造坐标。
                leader_for_participant = _wait_peer_spawn(participant, leader.username)
                leader_for_observer = _wait_peer_spawn(observer, leader.username)
                participant.move_to(
                    leader_for_participant.data["x"],
                    leader_for_participant.data["y"],
                    leader_for_participant.data["z"],
                )
                observer.move_to(
                    leader_for_observer.data["x"],
                    leader_for_observer.data["y"],
                    leader_for_observer.data["z"],
                )

                leader.cmd("realm set spirit")
                leader.expect_chat("[dev] realm set", timeout=10.0)
                leader.cmd("meridian open_all")
                leader.expect_chat("open_all does not auto-breakthrough", timeout=10.0)

                leader_character_id = env.lookup_character_id(leader.username)
                participant_character_id = env.lookup_character_id(participant.username)

                request_sent_at = leader.events[-1].t if leader.events else 0.0
                leader.intent(START_DU_XU)
                _wait_tribulation_phase(
                    leader, "omen", request_sent_at, DUXU_OMEN_WAIT_SECONDS
                )
                _wait_tribulation_phase(
                    participant, "omen", request_sent_at, DUXU_OMEN_WAIT_SECONDS
                )

                near_broadcast = _wait_broadcast(
                    observer, "warn", request_sent_at, timeout=15.0
                )
                near_payload = near_broadcast.data["payload"]
                if not near_payload.get("active") or not near_payload.get("spectate_invite"):
                    raise BotAssertionError(
                        "近处 observer 应收到 active=true 且 spectate_invite=true 的观礼广播，"
                        f"实际={near_payload}"
                    )
                if near_payload.get("spectate_distance", float("inf")) > 50.0:
                    raise BotAssertionError(
                        "近处 observer 的观礼距离必须落在既有 50 格邀请半径内，"
                        f"实际={near_payload.get('spectate_distance')}"
                    )

                # 走真实移动包离开观礼半径；锁定广播会再次携带该客户端的距离判断。
                observer_position = observer.position
                if observer_position is None:
                    raise BotAssertionError("observer 已收到集结广播但没有本地 PositionLook 坐标")
                far_x = observer_position[0] + OBSERVER_FAR_OFFSET_BLOCKS
                far_z = observer_position[2]
                observer.move_to(far_x, observer_position[1], far_z)

                lock = _wait_tribulation_phase(
                    leader,
                    "lock",
                    request_sent_at,
                    timeout=DUXU_OMEN_WAIT_SECONDS,
                )
                lock_broadcast = _wait_broadcast(
                    observer, "locked", request_sent_at, timeout=10.0
                )
                lock_payload = lock_broadcast.data["payload"]
                if lock_payload.get("spectate_invite"):
                    raise BotAssertionError(
                        "远处 observer 不应保留观礼邀请；锁定广播必须反映距离已超过邀请半径，"
                        f"实际={lock_payload}"
                    )
                if lock_payload.get("spectate_distance", 0.0) <= 50.0:
                    raise BotAssertionError(
                        "远处 observer 的锁定广播距离应超过 50 格邀请半径，"
                        f"实际={lock_payload.get('spectate_distance')}"
                    )

                leader_entity = int(leader_for_participant.data["entity_id"])
                for _attempt in range(PARTICIPANT_ATTACK_ATTEMPTS):
                    participant.attack_entity(leader_entity)
                    time.sleep(PARTICIPANT_ATTACK_INTERVAL_SECONDS)

                wave = _wait_tribulation_phase(
                    leader, "wave", lock.t, timeout=DUXU_LOCK_WAIT_SECONDS
                )
                participant_wave = _wait_tribulation_phase(
                    participant, "wave", lock.t, timeout=5.0
                )
                participants = wave.data["payload"].get("participants", [])
                if leader_character_id not in participants:
                    raise BotAssertionError(
                        "渡劫状态必须保留发起者 character_id，"
                        f"expected={leader_character_id} actual={participants}"
                    )
                if participant_character_id not in participants:
                    raise BotAssertionError(
                        "锁定阶段附近的队友攻击后必须进入 participants，"
                        f"expected={participant_character_id} actual={participants}"
                    )
                if participant_wave.data["payload"].get("participants") != participants:
                    raise BotAssertionError(
                        "leader 与 participant 对同一 wave 的 participants 快照必须一致，"
                        f"leader={participants} participant={participant_wave.data['payload'].get('participants')}"
                    )

                leader.assert_alive("组队渡劫参与者登记与 observer 距离广播完成后")
                participant.assert_alive("组队渡劫参与者登记完成后")
                observer.assert_alive("远处观礼邀请关闭后")
