"""修炼/渡劫/顿悟 gap 场景共享断言工具 —— `cultivation_*_du_xu|insight_*` 场景共用。

与 `_rejection_helpers.py`（errpath 分支）同风格：「干净处理」的可观察契约：

1. 请求被处理/拒绝后连接**不被踢**、**不被单方面遗忘**（无 disconnect / connection_lost）；
2. server 在处理之后**继续心跳**（新的 keepalive 到达）；
3. 处理之后一个**合法**请求仍产生预期响应（`set_meridian_target` → 「已收到经脉目标」聊天），
   证明连接不是「没崩但已坏」。

下划线前缀：runner 跳过下划线开头的文件，本模块只做共享工具不被当作场景。
"""

from __future__ import annotations


def wait_keepalive_after(bot, after: float, timeout: float = 25.0):
    """等 t > after 的新 keepalive（server 在拒绝/忽略坏请求后仍主动维持连接）。"""
    return bot.wait_for(
        lambda e: e.kind == "keepalive" and e.t > after,
        timeout=timeout,
        description="server 处理请求后仍继续心跳（连接没被踢、也没被单方面遗忘）",
    )


def assert_valid_request_still_works(bot, *, meridian: str = "lung") -> None:
    """合法请求必须仍被正常处理 —— 连接在之前请求之后处于完好可用状态。

    用 `set_meridian_target` 当探针：预期响应是 server 广播「已收到经脉目标：」聊天确认。
    """
    bot.intent({"v": 1, "type": "set_meridian_target", "meridian": meridian})
    bot.expect_chat("已收到经脉目标", timeout=10.0)


def wait_payload_containing(
    bot, channel: str, needle: bytes, after: float, timeout: float, description: str
):
    """等 t > after 且原始字节含 needle 的 channel payload（浅扫描，同 combat_skill_cast 风格）。"""
    return bot.wait_for(
        lambda e: e.kind == "payload"
        and e.data["channel"] == channel
        and e.t > after
        and needle in e.data["data"],
        timeout=timeout,
        description=description,
    )


def du_xu_setup(bot) -> None:
    """dev 铺垫到渡虚劫可发起状态：通灵 + 全经脉。

    `start_du_xu` 前置（`server/src/cultivation/tribulation.rs::du_xu_prereqs_met`）：
    realm == Spirit 且全部经脉已开且 opened_count >= Void.required_meridians()。
    """
    bot.cmd("realm set spirit")
    bot.expect_chat("[dev] realm set", timeout=10.0)
    bot.cmd("meridian open_all")
    bot.expect_chat("open_all does not auto-breakthrough", timeout=10.0)


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


def breakthrough_setup(bot) -> float:
    """dev 铺垫后真实 `breakthrough_request` → 首次突破到 Induce（触发顿悟邀约）。

    返回 sent_at（breakthrough_request 发出时刻的 monotonic 时间戳），供后续
    after 断言用。配方取自 cultivation_breakthrough.py：醒灵 + 全经脉 + qi 20 +
    spawn 灵眼充足 → 成功率 clamp 到 1.0，确定性成功。
    """
    bot.cmd("realm set awaken")
    bot.expect_chat("[dev] realm set", timeout=10.0)
    bot.cmd("meridian open_all")
    bot.expect_chat("open_all does not auto-breakthrough", timeout=10.0)
    bot.cmd("qi set 20")
    bot.expect_chat("[dev] qi set", timeout=10.0)
    bot.cmd("zone_qi set spawn 1.00")
    bot.expect_chat("[dev] zone_qi", timeout=10.0)

    sent_at = bot.events[-1].t if bot.events else 0.0
    bot.intent({"type": "breakthrough_request", "v": 1})
    bot.wait_for(
        lambda e: _is_breakthrough_observation(e, sent_at),
        timeout=15.0,
        description=(
            "breakthrough_request 后的突破相关 payload/chat（首次突破 → 顿悟邀约触发点）"
        ),
    )
    bot.assert_alive("breakthrough_request 链路执行后")
    return sent_at
