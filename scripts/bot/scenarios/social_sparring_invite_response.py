"""切磋邀请回执三分支：拒绝/逾时聊天反馈、未知 invite 接受无副作用。

协议契约面（server/src/social/mod.rs handle_sparring_invite_responses）：
- 邀请本身由 S2S（agent_cmd / NPC 侧）生成，C2S 侧只有回执
  `sparring_invite_response{v,invite_id,accepted,timed_out}`；本场景在无 pending
  邀请的 fixture 世界锁住回执三分支的 wire 行为：
  - accepted=false → 回执方收聊天「切磋已拒绝」（即使 invite_id 未知也反馈）
  - timed_out=true → 回执方收聊天「切磋邀请已逾时」
  - accepted=true 但 invite_id 无对应 pending → server 拒绝（warn log），
    无聊天反馈、连接保持
- accept 成功路径（SparringState 双端建立 + 「切磋开始」聊天）依赖 S2S 邀请侧
  先注入 pending，非单 bot 场景可复现，留给邀请侧就绪后的场景。
"""

from __future__ import annotations

import time

from bot.scenarios._combat_helpers import wait_for_ready

DESCRIPTION = "切磋邀请回执：拒绝/逾时各有聊天反馈，未知 invite 的接受静默且连接保持"
MODULES = ["social", "network"]

UNKNOWN_INVITE_ID = "sparring:bot_e2e_nonexistent_invite"


def _sparring_chat_count(bot) -> int:
    return sum(
        1
        for event in bot.events_of("chat")
        if "切磋" in event.data["text"]
    )


def run(env) -> None:
    with env.new_bot("SA") as bot:
        wait_for_ready(bot)

        # 分支①：拒绝回执 → 聊天「切磋已拒绝」（registry 无此邀请也按拒绝反馈）
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

        # 分支②：逾时回执 → 聊天「切磋邀请已逾时」
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

        # 分支③：接受回执但无对应 pending → server 丢弃，无聊天反馈、连接保持
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
        bot.assert_alive("未知邀请 accept 回执后")
