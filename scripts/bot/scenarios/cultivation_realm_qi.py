"""修炼 dev 命令反馈 —— realm/qi/meridian 的协议 Bot 黑盒契约。

这些命令只负责搭建 dev 场景；断言玩家可见反馈和状态钳制，不把它们当作
自然突破或 qi ledger 守恒证据。
"""

from bot.bot import BotAssertionError
from bot.scenarios._combat_helpers import last_event_time

DESCRIPTION = "realm/qi/meridian dev 命令锁住成功、拒绝、钳制与重复状态反馈"
MODULES = ["cmd", "cultivation"]


def _chat_after(
    bot,
    watermark: float,
    expected: str,
    timeout: float = 10.0,
    *,
    exact: bool = False,
):
    return bot.wait_for(
        lambda event: (
            event.kind == "chat"
            and event.t > watermark
            and (
                event.data["text"] == expected
                if exact
                else expected in event.data["text"]
            )
        ),
        timeout=timeout,
        description=(
            f"t>{watermark:.3f}s 后严格等于「{expected}」的聊天消息"
            if exact
            else f"t>{watermark:.3f}s 后包含「{expected}」的聊天消息"
        ),
    )


def _command_and_chat(bot, command: str, expected: str, *, exact: bool = False):
    watermark = last_event_time(bot)
    bot.cmd(command)
    return _chat_after(bot, watermark, expected, exact=exact)


def _successful_command_and_chat(bot, command: str, substring: str):
    event = _command_and_chat(bot, command, substring)
    if " rejected:" in event.data["text"]:
        raise BotAssertionError(
            f"[{bot.username}] {command} 期望成功反馈，实际收到拒绝：{event.data['text']}"
        )
    return event


def run(env) -> None:
    with env.new_bot("Cult") as bot:
        bot.expect_event("game_join", timeout=15.0)
        bot.expect_event("pos_look", timeout=15.0)

        _successful_command_and_chat(
            bot,
            "realm set induce",
            "[dev] realm set Awaken -> Induce",
        )

        _command_and_chat(
            bot,
            "realm set bot_e2e_no_such_realm",
            "[dev] realm set rejected: unknown realm \"bot_e2e_no_such_realm\"; "
            "allowed: awaken|induce|condense|solidify|spirit|void",
            exact=True,
        )

        _successful_command_and_chat(
            bot,
            "qi max 12",
            "[dev] qi max 10.0 -> 12.0; current=0.0",
        )
        _successful_command_and_chat(
            bot,
            "qi set 11",
            "[dev] qi set 0.0 -> 11.0",
        )
        _successful_command_and_chat(
            bot,
            "qi max 4",
            "[dev] qi max 12.0 -> 4.0; current=4.0",
        )
        _command_and_chat(
            bot,
            "qi set -1",
            "[dev] qi set rejected: value must be finite >= 0",
            exact=True,
        )
        _command_and_chat(
            bot,
            "qi max -1",
            "[dev] qi max rejected: value must be finite >= 0",
            exact=True,
        )

        _successful_command_and_chat(bot, "meridian open lung", "[dev] opened meridian lung")
        _successful_command_and_chat(
            bot,
            "meridian open lung",
            "[dev] meridian lung already open",
        )
        listed = _successful_command_and_chat(bot, "meridian list", "[dev] opened meridians:")
        if "lung progress=1.00 cap=" not in listed.data["text"]:
            raise BotAssertionError(
                f"[{bot.username}] meridian list 必须报告已打开 lung 的满进度与容量，实际 "
                + listed.data["text"]
            )

        opened_all = _successful_command_and_chat(
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
