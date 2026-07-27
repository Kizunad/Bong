"""修炼 dev 命令反馈 —— realm/qi/meridian 的协议 Bot 黑盒契约。

这些命令只负责搭建 dev 场景；断言玩家可见反馈和状态钳制，不把它们当作
自然突破或 qi ledger 守恒证据。
"""

from bot.bot import BotAssertionError

DESCRIPTION = "realm/qi/meridian dev 命令锁住成功、拒绝、钳制与重复状态反馈"
MODULES = ["cmd", "cultivation"]


def _chat_after(bot, watermark: float, substring: str, timeout: float = 10.0):
    return bot.wait_for(
        lambda event: (
            event.kind == "chat"
            and event.t > watermark
            and substring in event.data["text"]
        ),
        timeout=timeout,
        description=f"t>{watermark:.3f}s 后包含「{substring}」的聊天消息",
    )


def _command_and_chat(bot, command: str, substring: str):
    watermark = bot.events[-1].t if bot.events else 0.0
    bot.cmd(command)
    return _chat_after(bot, watermark, substring)


def run(env) -> None:
    with env.new_bot("Cult") as bot:
        bot.expect_event("game_join", timeout=15.0)
        bot.expect_event("pos_look", timeout=15.0)

        realm = _command_and_chat(bot, "realm set induce", "[dev] realm set")
        if "Induce" not in realm.data["text"]:
            raise BotAssertionError(
                f"[{bot.username}] realm set 成功反馈必须带目标 Induce，实际 "
                + realm.data["text"]
            )

        _command_and_chat(
            bot,
            "realm set bot_e2e_no_such_realm",
            "[dev] realm set rejected: unknown realm \"bot_e2e_no_such_realm\"; "
            "allowed: awaken|induce|condense|solidify|spirit|void",
        )

        _command_and_chat(bot, "qi max 12", "[dev] qi max")
        _command_and_chat(bot, "qi set 11", "[dev] qi set")
        clamped = _command_and_chat(bot, "qi max 4", "[dev] qi max")
        if "current=4.0" not in clamped.data["text"]:
            raise BotAssertionError(
                f"[{bot.username}] 降低 qi max 必须同步钳制 qi_current 到 4.0，实际 "
                + clamped.data["text"]
            )
        _command_and_chat(
            bot,
            "qi set -1",
            "[dev] qi set rejected: value must be finite >= 0",
        )
        _command_and_chat(
            bot,
            "qi max -1",
            "[dev] qi max rejected: value must be finite >= 0",
        )

        _command_and_chat(bot, "meridian open lung", "[dev] opened meridian lung")
        _command_and_chat(
            bot,
            "meridian open lung",
            "[dev] meridian lung already open",
        )
        listed = _command_and_chat(bot, "meridian list", "[dev] opened meridians:")
        if "lung progress=1.00 cap=" not in listed.data["text"]:
            raise BotAssertionError(
                f"[{bot.username}] meridian list 必须报告已打开 lung 的满进度与容量，实际 "
                + listed.data["text"]
            )

        opened_all = _command_and_chat(
            bot,
            "meridian open_all",
            "open_all does not auto-breakthrough",
        )
        if "total opened=20" not in opened_all.data["text"]:
            raise BotAssertionError(
                f"[{bot.username}] humanoid open_all 必须报告 20 条经脉全部打开，实际 "
                + opened_all.data["text"]
            )
        if "realm remains Induce" not in opened_all.data["text"]:
            raise BotAssertionError(
                f"[{bot.username}] open_all 不得暗中触发突破，实际 "
                + opened_all.data["text"]
            )

        bot.assert_alive("realm/qi/meridian dev 命令反馈后")
