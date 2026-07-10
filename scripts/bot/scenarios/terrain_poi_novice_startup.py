"""新手 POI 生产注册黑盒回归。

通过 dev-only `/tppoi novice` 读取生产 `PoiNoviceRegistry` 的只读摘要。CI 的
fallback flat world 没有 raster manifest，因此 count 可以是 0；关键契约是资源
必须经真实 world 注册链存在，命令必须返回 `registry count=...` 而不是
`registry missing`。真实 v2 manifest 的六类坐标/selection/PoiSpawned 完整载荷由
Rust Startup 集成测试锁定。
"""

from bot.bot import BotAssertionError

DESCRIPTION = "新手 POI registry 经生产启动注册，并可由协议 Bot 只读观察"
MODULES = ["terrain", "poi_novice"]


def run(env) -> None:
    with env.new_bot("Poi") as bot:
        bot.expect_event("game_join", timeout=15.0)
        bot.expect_event("pos_look", timeout=15.0)

        bot.cmd("tppoi novice")
        event = bot.expect_chat("[dev] novice_poi registry count=", timeout=10.0)
        text = event.data["text"]
        if "kinds=" not in text:
            raise BotAssertionError(
                f"[{bot.username}] 期望 novice registry 摘要同时含 count 与 kinds，"
                f"实际 chat={text!r}"
            )
        bot.assert_alive("读取 novice POI registry 摘要后")
