"""新手 POI 生产注册黑盒回归。

`scripts/bot-e2e.sh` 先生成含六类 novice POI 的真实 v2 raster fixture，再通过
dev-only `/tppoi novice` 读取生产 `PoiNoviceRegistry`。任一 manifest/loader/register
链路断开、registry 为空、类别缺失或 selection tag 错误，本场景都必须撞红。
"""

import re

from bot.bot import BotAssertionError

DESCRIPTION = "新手 POI registry 经生产启动注册，并可由协议 Bot 只读观察"
MODULES = ["terrain", "poi_novice"]

EXPECTED = {
    "forge_station": "strict_radius_1500",
    "alchemy_furnace": "strict_radius_1500",
    "rogue_village": "strict_radius_1500",
    "mutant_nest": "relaxed_radius_2000",
    "scroll_hidden": "strict_radius_1500",
    "spirit_herb_valley": "relaxed_radius_2000_qi_margin_0_1",
}


def _selection_strategy(detail_text: str) -> str | None:
    match = re.search(r"(?:^|\s)selection=([^\s]+)(?:\s|$)", detail_text)
    return match.group(1) if match is not None else None


def run(env) -> None:
    with env.new_bot("Poi") as bot:
        bot.expect_event("game_join", timeout=15.0)
        bot.expect_event("pos_look", timeout=15.0)

        bot.cmd("tppoi novice")
        event = bot.expect_chat("[dev] novice_poi registry count=", timeout=10.0)
        text = event.data["text"]
        match = re.search(r"count=(\d+) kinds=(.+)$", text)
        if match is None:
            raise BotAssertionError(
                f"[{bot.username}] 期望 novice registry 摘要同时含 count 与 kinds，"
                f"实际 chat={text!r}"
            )
        count = int(match.group(1))
        kinds = match.group(2)
        try:
            kind_counts = {
                kind: int(kind_count)
                for entry in kinds.split(",")
                for kind, kind_count in [entry.split(":", maxsplit=1)]
            }
        except (TypeError, ValueError) as error:
            raise BotAssertionError(
                f"[{bot.username}] novice registry kinds 摘要格式非法：{kinds!r}"
            ) from error

        target_counts = {kind: kind_counts.get(kind) for kind in EXPECTED}
        expected_counts = {kind: 1 for kind in EXPECTED}
        if count != sum(kind_counts.values()) or target_counts != expected_counts:
            raise BotAssertionError(
                f"[{bot.username}] 期望真实 v2 manifest 六类目标 novice POI 各 1，"
                f"实际 count={count}, kinds={kind_counts!r}"
            )

        for kind, strategy in EXPECTED.items():
            detail = bot.expect_chat(f"[dev] novice_poi {kind} ", timeout=10.0)
            detail_text = detail.data["text"]
            actual_strategy = _selection_strategy(detail_text)
            if actual_strategy != strategy:
                raise BotAssertionError(
                    f"[{bot.username}] 期望 {kind} selection={strategy}，"
                    f"实际 selection={actual_strategy!r}, chat={detail_text!r}"
                )
        bot.assert_alive("读取 novice POI registry 摘要后")
