"""玩家聊天 zone 回显回归 —— 锁 fix-playtest-batch 的 chat echo 修复。

历史 bug（2026-07-06 bot playtest）：普通聊天只进 Redis `bong:player_chat`
喂天道，发送者和同 zone 玩家全都看不见——发言如喊进虚空。修复后
`collect_player_chat` 的 PlayerChat 分支向同 zone client（含发送者）广播
`<username> message`，广播格局与 SpiritTreasureDialogue 一致。

断言：发送者必须在短窗口内收到自己的 `<name> text` 回显。
"""

import time

DESCRIPTION = "普通聊天必须回显给发送者（同 zone 广播含自己），不再是喂天道的单向管道"
MODULES = ["network", "social"]

ECHO_TIMEOUT = 6.0


def run(env) -> None:
    with env.new_bot("CE") as bot:
        bot.expect_event("game_join", timeout=15.0)
        bot.expect_event("pos_look", timeout=15.0)
        time.sleep(1.0)

        marker = f"echo-{env.run_tag}"
        bot.chat(marker)
        event = bot.expect_chat(marker, timeout=ECHO_TIMEOUT)
        text = event.data["text"]
        assert bot.username in text, (
            f"期望回显带发送者名（<{bot.username}> {marker}，同 zone 广播格式），"
            f"实际收到 {text!r}——名字缺失说明广播格式回归"
        )
        bot.assert_alive("chat echo 全程")
