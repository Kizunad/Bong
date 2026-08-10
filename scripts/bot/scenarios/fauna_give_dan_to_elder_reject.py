"""give_dan_to_elder 拒收链（plan-dying-elder-v1 P1 垂死大能馈赠）。

handle_give_dan_to_elder（client_request_handler.rs:19791）前置检查顺序：
1. pill_instance_id 不在背包 → 聊天「§c[垂死大能] 背包中未找到该回元丹。」
2. 物品非 huiyuan_pill → 聊天「§c[垂死大能] 只接受回元丹。」
3. elder_entity_id 无对应实体 → 聊天「§c[垂死大能] 找不到目标大能。」

happy path（大能收丹 → Plea→Recovering）不可黑盒构造：垂死大能 spawn 要求
tsy zone spirit_qi < -0.4 且间隔 30 游戏日（dying_elder.rs:62 DYING_ELDER_
SPAWN_INTERVAL_TICKS = 30 * 24000 ≈ 10h 实墙钟），fixture 运行不可达；dev
命令层也无 elder/大能 spawn 命令。故本场景按拒收链逐条断言聊天反馈，并验证
请求失败不中断连接（大能未收丹、玩家无损失）。

顺序断言同时锁定检查顺序：先背包后模板、模板先于目标实体。
"""

from ._inventory_helpers import (
    require_item,
    wait_inventory_contains,
    wait_inventory_revision_after,
    wait_join_and_inventory,
)

DESCRIPTION = "give_dan_to_elder 拒收链：背包缺失→非回元丹→目标大能不存在，逐条聊天拒绝"
MODULES = ["fauna", "network"]

DAN_REQUEST = {"type": "give_dan_to_elder", "v": 1}
MEAT_ITEM = "food.mundane.cooked_meat"
ELDER_ID = 1
NO_SUCH_ELDER_ID = 987654321


def run(env) -> None:
    with env.new_bot("DhH") as bot:
        snapshot = wait_join_and_inventory(bot)
        revision = snapshot["revision"]

        # 1. instance_id 不在背包 → 背包中未找到该回元丹。
        bot.intent({**DAN_REQUEST, "pill_instance_id": 999999999999, "elder_entity_id": ELDER_ID})
        bot.expect_chat("背包中未找到该回元丹。", timeout=10.0)
        bot.assert_alive("背包缺失拒收后")

        # 2. 背包内有非回元丹物品 → 只接受回元丹。
        bot.cmd(f"give {MEAT_ITEM} 1")
        bot.expect_chat(f"[dev] gave {MEAT_ITEM} x1", timeout=10.0)
        snapshot = wait_inventory_contains(bot, MEAT_ITEM, timeout=10.0)
        meat = require_item(snapshot, MEAT_ITEM)
        bot.intent(
            {**DAN_REQUEST, "pill_instance_id": meat["item"]["instance_id"], "elder_entity_id": ELDER_ID}
        )
        bot.expect_chat("只接受回元丹。", timeout=10.0)
        bot.assert_alive("非回元丹拒收后")

        # 3. 回元丹在背包、大能实体不存在 → 找不到目标大能。
        revision = snapshot["revision"]
        bot.cmd("give huiyuan_pill 1")
        bot.expect_chat("[dev] gave huiyuan_pill x1", timeout=10.0)
        snapshot = wait_inventory_revision_after(bot, revision, timeout=10.0)
        pill = require_item(snapshot, "huiyuan_pill")
        bot.intent(
            {**DAN_REQUEST, "pill_instance_id": pill["item"]["instance_id"], "elder_entity_id": NO_SUCH_ELDER_ID}
        )
        bot.expect_chat("找不到目标大能。", timeout=10.0)
        bot.assert_alive("give_dan_to_elder 拒收链全程")
