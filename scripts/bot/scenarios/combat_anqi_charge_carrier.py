"""暗器链：carrier 蓄力（charge_carrier）的装弹前置黑盒契约。

蓄力/抛射系统（server/src/combat/carrier.rs）从手槽持有的暗器载体读取
（`find_chargeable_hand`：equipped[main/off].held → CarrierKind::from_template_id），
因此必须先把手持暗器胚材装到 main_hand 才能蓄力。但暗器载体 `anqi_yibian_shougu`
在物品目录里是 `category="misc"`（server/assets/items/anqi.toml，无 weapon_spec），
而 `validate_equip_to` 的手槽档只放行 weapon / tool（off_hand 另收 treasure/shield）——
非以上档位一律返回 `InventoryMoveRejectReason::EquipCategoryMismatch` 并下发
`inventory_move_rejected` 回执。

本场景锁的就是这条**装弹护栏**：裸异变兽骨经标准装备 wire 试图挂载到 main_hand，
被服务器结构化拒绝且库存不变。该约束与姊妹场景
`inventory_equip_wearer_race_reject.py` 用同一条 `inventory_move_rejected`
S2C 管线（同一个 `validate_equip_to` 函数、同一 `emit_inventory_move_rejected`
下发路径）。

**已知缺口（记录不隐瞒）**：正因裸载体无法进手，charge_carrier 的成功路径
（预扣一半 qi → charged 模板原位转化 + carrier_charged 事件）无法由真实客户端
端到端触达；蓄力系统成功分支只在 server 单测里用直接写槽的
`inventory_with_main_hand()` helper 覆盖。真"装弹→蓄力"链路请在物品目录把载体档位
回填为 anqi/hidden_weapon 后再补该场景。
"""

from bot.scenarios._inventory_helpers import (
    equip_location,
    find_item,
    send_move,
    wait_inventory_contains,
    wait_join_and_inventory,
)

DESCRIPTION = "蓄力前置护栏：裸 anqi 载体装到 main_hand 触发 inventory_move_rejected（reason=equip_category_mismatch）"
MODULES = ["anqi", "combat", "inventory"]
DEFAULT_ENABLED = False
REQUIRED_ENV = "BOT_E2E_ANQI_REDIS"
RUN_IN_ALL_WHEN_ENV = REQUIRED_ENV

CARRIER_ID = "anqi_yibian_shougu"


def run(env) -> None:
    with env.new_bot("Charge") as bot:
        wait_join_and_inventory(bot)

        bot.cmd(f"give {CARRIER_ID} 1")
        bot.expect_chat(f"[dev] gave {CARRIER_ID} x1", timeout=10.0)
        snapshot = wait_inventory_contains(bot, CARRIER_ID, timeout=10.0)
        bone = find_item(snapshot, CARRIER_ID)

        send_move(
            bot,
            bone["item"]["instance_id"],
            bone["location"],
            equip_location("main_hand", "held"),
        )

        rejected = bot.expect_server_data("inventory_move_rejected", timeout=10.0)
        payload = rejected.data["payload"]
        assert payload["reason"] == "equip_category_mismatch", (
            "裸暗器载体应被 validate_equip_to 拒绝装到 main_hand（equip_category_mismatch），"
            f"实际 reason={payload['reason']!r}"
        )

        # 拒绝不应改动库存：与 move 前快照逐字段比对——revision 不变、同实例
        # 同格、stack 一致。只查「模板仍在且不在 equip」会放走「同模板换实例/
        # 移位/改堆叠」的坏实现（review finding [major]：拒收未验证不变库存
        # 契约）。
        bot.assert_alive("装弹护栏拒绝后")
        post = wait_inventory_contains(
            bot,
            CARRIER_ID,
            timeout=10.0,
            after_t=rejected.t,
        )
        assert int(post["revision"]) == int(snapshot["revision"]), (
            f"拒绝不应 bump inventory revision: {snapshot['revision']} -> {post['revision']}"
        )
        post_item = find_item(post, CARRIER_ID)
        assert post_item is not None, "拒绝后载体不应消失"
        assert post_item["item"]["instance_id"] == bone["item"]["instance_id"], (
            f"拒绝后应是同一实例仍在原格，实际 instance "
            f"{bone['item']['instance_id']} -> {post_item['item']['instance_id']}"
        )
        assert post_item["location"] == bone["location"], (
            f"拒绝后载体应仍在原格 {bone['location']!r}，实际 {post_item['location']!r}"
        )
        assert post_item["item"]["stack_count"] == bone["item"]["stack_count"], (
            f"拒绝后堆叠不应变化，实际 {bone['item']['stack_count']} -> "
            f"{post_item['item']['stack_count']}"
        )
        hand_held = (post.get("equipped", {}).get("main_hand_held") or {}).get("item_id")
        assert hand_held != CARRIER_ID, f"main_hand 不应持有裸载体，实际 {hand_held!r}"