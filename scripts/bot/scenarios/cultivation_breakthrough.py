"""修炼突破 intent 链路 —— dev 铺垫后发 `breakthrough_request`。

黑盒断言面：
- C2S：`bot.intent({"type":"breakthrough_request","v":1})`，形状来自
  `server/src/schema/client_request.rs::ClientRequestV1::BreakthroughRequest`
- dispatch：`server/src/network/client_request_handler.rs` 应 emit `BreakthroughRequest`
- 结果：`server/src/cultivation/breakthrough.rs` / `network/vfx_event_emit.rs`
  应让玩家收到突破相关 `bong:vfx_event`，或至少收到含“突破”的 chat/narration 反馈。
"""

DESCRIPTION = "breakthrough_request intent 经 dev 铺垫后产生突破相关 payload 或 chat"
MODULES = ["cultivation", "network"]

BREAKTHROUGH_REQUEST = {"type": "breakthrough_request", "v": 1}


def _is_breakthrough_observation(event, sent_at: float) -> bool:
    if event.t <= sent_at:
        return False
    if event.kind == "payload":
        channel = event.data["channel"]
        data = event.data["data"]
        return (
            channel.startswith("bong:breakthrough")
            or (
                channel == "bong:vfx_event"
                and (
                    b"bong:breakthrough_pillar" in data
                    or b"bong:breakthrough_fail" in data
                    or b"breakthrough" in data
                )
            )
            or (channel == "bong:server_data" and b"breakthrough" in data)
        )
    return event.kind == "chat" and "突破" in event.data["text"]


def run(env) -> None:
    with env.new_bot("Break") as bot:
        bot.expect_event("game_join", timeout=15.0)
        bot.expect_event("pos_look", timeout=15.0)

        # 醒灵 -> 引气：成本低、无需灵眼；open_all 铺满经脉，zone_qi 避免场地前置失败。
        bot.cmd("realm set awaken")
        bot.expect_chat("[dev] realm set", timeout=10.0)
        bot.expect_chat("Awaken", timeout=10.0)

        bot.cmd("meridian open_all")
        bot.expect_chat("open_all does not auto-breakthrough", timeout=10.0)

        bot.cmd("qi set 20")
        bot.expect_chat("[dev] qi set", timeout=10.0)

        bot.cmd("zone_qi set spawn 1.00")
        bot.expect_chat("[dev] zone_qi `spawn`", timeout=10.0)

        sent_at = bot.events[-1].t if bot.events else 0.0
        bot.intent(BREAKTHROUGH_REQUEST)

        bot.wait_for(
            lambda e: _is_breakthrough_observation(e, sent_at),
            timeout=15.0,
            description=(
                "breakthrough_request 后的突破相关 payload/chat；若超时，按顺序检查 "
                "client_request_handler.rs BreakthroughRequest dispatch、"
                "cultivation/breakthrough.rs outcome、network/vfx_event_emit.rs bong:vfx_event 接线"
            ),
        )
        bot.assert_alive("breakthrough_request intent 链路执行后")
