"""连接生命周期：失联客户端被 server 侧 keepalive 超时清理。

server（valence keepalive.rs：period 8s）对每个 Client 发 KeepAlive；Bot 停掉
心跳应答（silent()）后，server 在下一个周期检测到无应答 → 移除 Client →
关连接 = 服务端对失联/挂死连接的清理。

本场景锁住：
- 正常心跳时连接保持（最大化宽容的既有面，先锁住基线）
- silent 后 server **主动**在有限时间内断开该连接（connection_lost），而不是
  把失联连接挂到天荒地老
- 清理发生前 server 仍在发 KeepAlive（证明触发源是超时，不是我们主动断开）
- 被清理的同身份重连仍能干净 join（清理不污染持久化状态）

等待上限取 30s：周期 8s + 无应答判死 8s = 最坏 ~16s，30s 留足余量。
"""

from bot.scenarios._combat_helpers import last_event_time
from bot.scenarios._inventory_helpers import wait_join_and_inventory

DESCRIPTION = "失联客户端被 keepalive 超时清理（server 主动断连），同身份可干净重连"
MODULES = ["network"]

SILENCE_TIMEOUT = 30.0


def run(env) -> None:
    with env.new_bot("Idle") as bot:
        wait_join_and_inventory(bot)
        # 基线：正常心跳时连接保持
        bot.expect_event("keepalive", timeout=30.0)
        bot.assert_alive("收到首个 keepalive 后连接保持")

        silence_anchor = last_event_time(bot)
        bot.silent()  # 停掉 KeepAlive 应答，保持读取

        # 清理前 server 仍会再发 KeepAlive（bot 不应答）；证明触发源是超时
        bot.wait_for(
            lambda e: e.kind == "keepalive" and e.t > silence_anchor,
            timeout=SILENCE_TIMEOUT,
            description="silent 后 server 仍发 KeepAlive（然后因无应答判死）",
        )

        # server 侧清理：连接被主动断开
        lost = bot.wait_for(
            lambda e: e.kind == "connection_lost",
            timeout=SILENCE_TIMEOUT,
            description=(
                "silent 后 server 因 keepalive 超时主动断开连接"
                "（服务端失联清理，而非挂死连接）"
            ),
        )
        assert lost.t - silence_anchor >= 5.0, (
            "连接不应在 silent 后立刻断开（应等 server 走完至少一个 keepalive"
            "周期判死），实际 silent 后 {:.1f}s 就断了".format(lost.t - silence_anchor)
        )

    # 被清理的同身份重连：干净 join，连接保持
    with env.new_bot("Idle") as bot:
        wait_join_and_inventory(bot)
        bot.assert_alive("keepalive 超时清理后同身份重连，干净 join")
