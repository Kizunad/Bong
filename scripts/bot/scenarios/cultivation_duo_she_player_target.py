"""双人夺舍完整链路：host 对同服玩家发 `duo_she_request`，夺舍目标身体。

target_id 契约（实测锁定）：必须是**完整 character_id** `offline:<victim>:<uuid>`
（`player_character_id` 复合形式）。二段式 `offline:<victim>` 与运行时
lifecycle.character_id 不等，会被 `resolve_target_snapshot` 静默跳过（首次运行实测
无事件、无 despawn、无 redis 发布；该拒绝路径由 cultivation_duo_she_invalid_target
场景锁定）。uuid 由 server 在 join 时生成、从不经 S2C 下发，场景经
`env.lookup_character_id`（读 server 侧 player_core，仅构造输入）取得。

黑盒断言面：
- C2S：`bot.intent({"type":"duo_she_request","v":1,"target_id":...})`，
  形状来自 `server/src/schema/client_request.rs::ClientRequestV1::DuoSheRequest`
- dispatch：`server/src/network/client_request_handler.rs` 应 emit `DuoSheRequestEvent`
- 结果：`server/src/cultivation/possession.rs::process_duo_she_requests`——
  target 实体插 `(PossessedVictim, Despawned)`（victim 连接随实体 despawn 终止），
  host 经 `inherit_host_runtime_body` 继承 target 的坐标（S2C pos_look 可观察），
  host 本体连接保持；`bong:duo_she_event` 由 `cultivation_bridge` 发布（harness
  侧 redis 证据，见 DONE-W6-BOTSCEN-GAP2.md）。

victim 先走离出生点，host 夺舍后应瞬移至 victim 坐标——锁"身体继承"而非仅"目标死"。
"""

from __future__ import annotations

DESCRIPTION = "双人夺舍：host 夺舍同服玩家，victim 连接终止、host 继承坐标且连接保持"
MODULES = ["cultivation", "multibot", "network"]

REQUEST = {"type": "duo_she_request", "v": 1}

# victim 走离出生点的目标偏移（东向），保证 host 夺舍后的瞬移可观测。
VICTIM_OFFSET = 16.0
POSITION_TOLERANCE = 8.0


def _connection_ended(event) -> bool:
    return event.kind in ("connection_lost", "disconnect")


def run(env) -> None:
    with env.new_bot("GD2H") as host:
        host.expect_event("game_join", timeout=15.0)
        host.expect_event("pos_look", timeout=15.0)
        host_origin = host.position

        with env.new_bot("GD2V") as victim:
            victim.expect_event("game_join", timeout=15.0)
            victim.expect_event("pos_look", timeout=15.0)
            host.assert_alive("双 bot 就绪后")

            target_x = victim.position[0] + VICTIM_OFFSET
            target_z = victim.position[2]
            victim.move_to(target_x, victim.position[1], target_z)
            victim_last_pos = victim.position
            host.assert_alive("victim 移动后")

            host.intent({**REQUEST, "target_id": env.lookup_character_id(victim.username)})

            # target 实体被 Despawned：victim 连接必须终止（PossessedVictim 终结路径）。
            victim.wait_for(
                _connection_ended,
                timeout=25.0,
                description=(
                    "duo_she_request 后 victim 连接终止（实体 Despawned 关闭客户端）；"
                    "若超时检查 possession.rs 对 target 的 (PossessedVictim, Despawned) 插入"
                ),
            )

            # host 继承 target 坐标：应收到指向 victim 位置的 pos_look。
            host.wait_for(
                lambda e: (
                    e.kind == "pos_look"
                    and (
                        (e.data["x"] - victim_last_pos[0]) ** 2
                        + (e.data["z"] - victim_last_pos[2]) ** 2
                    )
                    ** 0.5
                    <= POSITION_TOLERANCE
                ),
                timeout=20.0,
                description=(
                    "duo_she_request 后 host 收到 victim 坐标的 pos_look（身体继承）；"
                    f"若超时检查 inherit_host_runtime_body 的 Position 继承，期望≈{victim_last_pos}"
                ),
            )
            host.assert_alive("host 夺舍后")

            # 双保险：host 确实离开过出生点（否则上面的距离断言无意义）。
            dx = host.position[0] - host_origin[0]
            dz = host.position[2] - host_origin[2]
            if (dx * dx + dz * dz) ** 0.5 < 3.0:
                raise AssertionError(
                    f"[{host.username}] 期望夺舍后 host 已离开出生点，实际位移 "
                    f"({dx:.1f}, {dz:.1f})——move_to 或身体继承未生效"
                )
