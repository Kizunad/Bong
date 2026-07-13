"""装备门拒绝回执 —— `InventoryMoveRejectReason::RaceMismatch` 的协议级回归覆盖。

plan-race-system-v1 P3b（决议 §8.1 #5）新增 `ItemTemplate.wearer_race` 装备门：拒绝时
server 下发结构化 `inventory_move_rejected` payload（`reason` 字段是
`InventoryMoveRejectReason::to_wire_tag()` 的 snake_case tag，`race_mismatch` 是本
plan 新增的一个变体）。

**已知缺口（记录不隐瞒）**：本场景无法真正触发 `race_mismatch` 本身——
1. 当前物品目录（`server/assets/items/*.toml`）尚无任何 `wearer_race` 非 `Any` 的
   真实条目（P3b 只交付了字段+校验逻辑，未回填目录数据）；
2. 玩家运行时变更种族的入口（`/race set <id>` dev 命令）明确排到 P5
   （`docs/plan-race-system-v1.md` §P5，非本 plan P3 范围），当前无法把一个连接中的
   bot 玩家的 `Cultivation.race` 改成非 human 以触发 Species/Humanoid 档拒绝。

故本场景改为验证同一条 `inventory_move_rejected` 协议管线的**姊妹拒绝原因**
`equip_category_mismatch`（同一 `validate_equip_to` 函数、同一 wire 结构、同一
`emit_inventory_move_rejected` 下发路径——`race_mismatch` 是该函数里"槽位分支判定
后、Ok(()) 前"的最后一道闸，前面任何一道闸的拒绝走的是完全相同的 S2C 回执管线）：
非护甲/非容器/非伪皮物品（`spirit_grass`，Herb 类）尝试穿到 chest/worn 必被拒绝，
证明该管线端到端可达且字段解得对。真正的 `race_mismatch` 端到端回归请在 P5 落地
`/race set` 后补一条姊妹场景（给非人形/异种族物品 + `/race set whale` + equip → 断言
`reason == "race_mismatch"`）。
"""

import time

from ._inventory_helpers import (
    container_location,
    equip_location,
    find_item,
    require_item,
    send_move,
    wait_join_and_inventory,
)

DESCRIPTION = "非护甲物品穿 chest/worn 触发 inventory_move_rejected（reason=equip_category_mismatch，与 race_mismatch 同管线）"
MODULES = ["inventory"]

NON_ARMOR_ITEM = "spirit_grass"


def run(env) -> None:
    with env.new_bot("Herbal") as bot:
        snapshot = wait_join_and_inventory(bot)

        bot.cmd(f"give {NON_ARMOR_ITEM} 1")
        bot.expect_chat(f"[dev] gave {NON_ARMOR_ITEM} x1", timeout=10.0)
        time.sleep(0.3)

        # 再拉一次快照定位刚给的物品（give 走独立 inventory_snapshot 推送）。
        snapshot = _latest_snapshot_containing(bot, snapshot, NON_ARMOR_ITEM)
        herb = require_item(snapshot, NON_ARMOR_ITEM)

        emit_t = time.time()
        send_move(
            bot,
            herb["item"]["instance_id"],
            herb["location"],
            equip_location("chest", "worn"),
        )

        rejected = bot.expect_server_data("inventory_move_rejected", timeout=10.0)
        payload = rejected.data["payload"]
        assert payload["reason"] == "equip_category_mismatch", (
            "非护甲/非容器/非伪皮物品穿 chest/worn 应被 validate_equip_to 的类型校验拒绝"
            f"（equip_category_mismatch），实际 reason={payload['reason']!r}——"
            "若此处变成别的 reason，多半是本场景物品/目标槽选择被后续改动使无效，"
            "而不是 race gate 真的生效了（本场景物品无 wearer_race 门）"
        )
        assert payload.get("slot") is None, (
            "equip_category_mismatch 不携带 slot（与 armor_slot_mismatch 的区别，"
            "见 InventoryMoveRejectReason::slot() 的 match）"
        )

        # 拒绝不应改变库存状态：物品仍在原位，未被移动到 chest。
        bot.assert_alive("装备门拒绝回执后")
        post_snapshot = _latest_snapshot_containing(bot, snapshot, NON_ARMOR_ITEM, after=emit_t)
        still_there = find_item(post_snapshot, NON_ARMOR_ITEM)
        assert still_there is not None, "拒绝后物品不应从库存中消失"
        assert still_there["location"]["kind"] != "equip" or still_there["location"].get(
            "slot"
        ) != "chest", "拒绝后物品不应出现在 chest 槽"


def _latest_snapshot_containing(bot, fallback: dict, item_id: str, after: float = 0.0) -> dict:
    from ._inventory_helpers import wait_inventory_contains

    if find_item(fallback, item_id) is not None:
        return fallback
    return wait_inventory_contains(bot, item_id, timeout=10.0)
