"""切磋邀请回执：拒绝/逾时/未知 accept + 真实邀请 producer→response 端到端。

协议契约面（server/src/social/mod.rs dispatch_sparring_invites /
handle_sparring_invite_responses）：
- 邀请由 `SparringInviteRequest` 事件驱动（dev 命令 `/sparring invite <username>`
  补上 bot 可达的生产通道）→ `dispatch_sparring_invites` 向 target 下发
  SparringInvite payload（ServerData oneof field 64；bot 侧 `_sparring_invite`
  解码出 invite_id/initiator/target/realm_band/breath_hint/terms/expires_at_ms）。
- C2S `sparring_invite_response{v,invite_id,accepted,timed_out}`：
  - accepted=false → 回执方收聊天「切磋已拒绝」（即使 invite_id 未知也反馈）
  - timed_out=true → 回执方收聊天「切磋邀请已逾时」
  - accepted=true 但 invite_id 无对应 pending → server 拒绝（warn log），
    无聊天反馈、连接保持
  - accepted=true 且 pending 真实存在 → 双端建立 SparringState，双方都收
    「切磋开始：不掉装、不扣寿、不记死仇」
本场景消费**服务器真实下发的邀请**（而非伪造 invite_id），把 producer→response
整条链路打通；回执三分支在无 pending 的 fixture 上也各锁一次。
"""

from __future__ import annotations

import time

from bot.scenarios._combat_helpers import last_event_time, wait_for_ready

DESCRIPTION = "切磋邀请回执：拒绝/逾时/未知 accept 各锁 + 真实邀请端到端消费回执"
MODULES = ["social", "network", "cmd", "multibot"]

UNKNOWN_INVITE_ID = "sparring:bot_e2e_nonexistent_invite"


def _sparring_chat_count(bot) -> int:
    return sum(
        1
        for event in bot.events_of("chat")
        if "切磋" in event.data["text"]
    )


def _expect_chat_after(bot, substring: str, anchor_t: float, timeout: float = 10.0):
    bot.wait_for(
        lambda e: (
            e.kind == "chat"
            and substring in e.data["text"]
            and e.t > anchor_t
        ),
        timeout=timeout,
        description=f"t>{anchor_t:.2f} 后收到含「{substring}」的聊天",
    )


def _wait_invite(bot, anchor_t: float, timeout: float = 10.0) -> dict:
    event = bot.wait_for(
        lambda e: (
            e.kind == "server_data"
            and e.data.get("payload_type") == "sparring_invite"
            and e.t > anchor_t
        ),
        timeout=timeout,
        description=f"t>{anchor_t:.2f} 后收到 sparring_invite payload（field 64 解码）",
    )
    return event.data["payload"]


def run(env) -> None:
    with env.new_bot("SA") as bot:
        wait_for_ready(bot)

        # ── 分支①：拒绝回执 → 聊天「切磋已拒绝」（registry 无此邀请也按拒绝反馈）──
        bot.intent(
            {
                "type": "sparring_invite_response",
                "v": 1,
                "invite_id": UNKNOWN_INVITE_ID,
                "accepted": False,
                "timed_out": False,
            }
        )
        bot.expect_chat("切磋已拒绝", timeout=10.0)

        # ── 分支②：逾时回执 → 聊天「切磋邀请已逾时」──
        bot.intent(
            {
                "type": "sparring_invite_response",
                "v": 1,
                "invite_id": UNKNOWN_INVITE_ID,
                "accepted": False,
                "timed_out": True,
            }
        )
        bot.expect_chat("切磋邀请已逾时", timeout=10.0)

        # ── 分支③：接受回执但无对应 pending → server 丢弃，无聊天反馈、连接保持 ──
        chat_before = _sparring_chat_count(bot)
        bot.intent(
            {
                "type": "sparring_invite_response",
                "v": 1,
                "invite_id": UNKNOWN_INVITE_ID,
                "accepted": True,
                "timed_out": False,
            }
        )
        time.sleep(2.0)
        assert _sparring_chat_count(bot) == chat_before, (
            f"未知 invite_id 的 accept 回执不应产生任何聊天反馈（pending 不存在即拒绝），"
            "实际收到了新「切磋」聊天"
        )

        # ── 分支④：真实邀请 producer→consume→decline ──
        # SA 经 dev 命令生产邀请，SB 必须解码服务器真实下发的 SparringInvite
        # （field 64）拿到 invite_id 等字段再回执——不是伪造的 unknown id。
        with env.new_bot("SB") as target:
            wait_for_ready(target)

            anchor = last_event_time(target)
            bot.cmd(f"sparring invite {target.username}")
            _expect_chat_after(bot, "[dev] sparring invite", last_event_time(bot))
            invite = _wait_invite(target, anchor)
            assert invite["invite_id"].startswith("sparring:"), (
                f"服务器下发的 sparring_invite.invite_id 应以 sparring: 开头，实际 {invite['invite_id']!r}"
            )
            assert invite["initiator"], (
                f"sparring_invite.initiator 不应为空，实际 {invite['initiator']!r}"
            )
            assert invite["target"], (
                f"sparring_invite.target 不应为空，实际 {invite['target']!r}"
            )
            assert invite["realm_band"], (
                f"sparring_invite.realm_band 不应为空，实际 {invite['realm_band']!r}"
            )
            assert invite["breath_hint"] == "气息相试", (
                f"sparring_invite.breath_hint 应为「气息相试」，实际 {invite['breath_hint']!r}"
            )
            assert invite["terms"] == "点到为止", (
                f"sparring_invite.terms 应为「点到为止」，实际 {invite['terms']!r}"
            )
            assert invite["expires_at_ms"] > 0, (
                f"sparring_invite.expires_at_ms 应 > 0，实际 {invite['expires_at_ms']!r}"
            )

            target_anchor = last_event_time(target)
            target.intent(
                {
                    "type": "sparring_invite_response",
                    "v": 1,
                    "invite_id": invite["invite_id"],
                    "accepted": False,
                    "timed_out": False,
                }
            )
            _expect_chat_after(target, "切磋已拒绝", target_anchor)

            # ── 分支⑤：真实邀请 accept → 双端 SparringState ──
            anchor2 = last_event_time(target)
            bot.cmd(f"sparring invite {target.username}")
            _expect_chat_after(bot, "[dev] sparring invite", last_event_time(bot))
            invite2 = _wait_invite(target, anchor2)
            assert invite2["invite_id"] != invite["invite_id"], (
                "第二次邀请应下发新的 invite_id（pending 互不串扰），实际复用了上一次"
            )

            target_anchor2 = last_event_time(target)
            # 双端锚点必须在 accept 回执之前定格：服务端在同一 handler 内把「切磋开始」
            # 同时发给双端，若在 target 那侧等回后再取 bot 锚点，bot 侧的聊天早已落袋，
            # 锚点会等于聊天自身时间戳，`e.t > anchor` 恒 false 而误报超时。
            bot_anchor2 = last_event_time(bot)
            target.intent(
                {
                    "type": "sparring_invite_response",
                    "v": 1,
                    "invite_id": invite2["invite_id"],
                    "accepted": True,
                    "timed_out": False,
                }
            )
            _expect_chat_after(target, "切磋开始", target_anchor2)
            _expect_chat_after(bot, "切磋开始", bot_anchor2)

            bot.assert_alive("真实邀请 accept 建立 SparringState 后")
            target.assert_alive("真实邀请 accept 建立 SparringState 后")
