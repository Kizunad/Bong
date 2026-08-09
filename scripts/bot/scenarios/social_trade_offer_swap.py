"""面对面交易 happy path：双 bot 一物换一物（trade_offer_request + trade_offer_response 接受）。

协议契约面（server/src/social/mod.rs dispatch_trade_offers / handle_trade_offer_responses）：
- C2S `trade_offer_request{v,target:"entity:<eid>",offered_instance_id}` 由发起者发往
  50 格内目标玩家；校验物品在包、目标背包非空后，只向**目标**推
  `bong:server_data` oneof=65 TradeOffer{offer_id, offered_item, requested_items,...}，
  发起者收不到任何回执 payload。
- C2S `trade_offer_response{v,offer_id,accepted:true,requested_instance_id}` 由目标回执：
  双方背包原子互换 offered↔requested，双端各收一次 inventory_snapshot。
- 断言走真实 wire 观察（payload 解码 + 背包快照），不读 server 内部状态。
"""

from __future__ import annotations

from bot.scenarios._inventory_helpers import (
    find_item,
    latest_inventory_snapshot,
    require_item,
    wait_inventory_contains,
    wait_inventory_revision_after_matching,
    wait_join_and_inventory,
)
from bot.scenarios._social_helpers import wait_player_protocol_id

DESCRIPTION = "双 bot 面对面交易：A 以 starter_talisman 换 B 的 huiyuan_pill，双方背包原子互换"
MODULES = ["social", "trade", "network", "cmd", "multibot"]

OFFER_ITEM_ID = "starter_talisman"
REQUEST_ITEM_ID = "huiyuan_pill"


def run(env) -> None:
    with env.new_bot("TA") as alice:
        wait_join_and_inventory(alice)
        with env.new_bot("TB") as bob:
            wait_join_and_inventory(bob)

            # 清包归一化：server data dir 跨 run 持久化，复跑时上一轮的 give 物品
            # 会污染断言（背包已满给不进 / find_item 误判），先清 pack 再发。
            alice.cmd("clearinv naked")
            alice.expect_chat("[dev] clearinv All revision=", timeout=10.0)
            bob.cmd("clearinv naked")
            bob.expect_chat("[dev] clearinv All revision=", timeout=10.0)

            alice.cmd(f"give {OFFER_ITEM_ID} 1")
            alice.expect_chat(f"[dev] gave {OFFER_ITEM_ID} x1", timeout=10.0)
            bob.cmd(f"give {REQUEST_ITEM_ID} 1")
            bob.expect_chat(f"[dev] gave {REQUEST_ITEM_ID} x1", timeout=10.0)

            alice_snapshot = wait_inventory_contains(alice, OFFER_ITEM_ID, timeout=10.0)
            talisman = require_item(alice_snapshot, OFFER_ITEM_ID)
            bob_snapshot = wait_inventory_contains(bob, REQUEST_ITEM_ID, timeout=10.0)
            pill = require_item(bob_snapshot, REQUEST_ITEM_ID)

            # game_join 的 entity_id 恒为 0（valence 保留给客户端自身），不能当
            # target；真实 protocol id 由 PlayerSpawnS2c 下发，bob 加入后由 alice 视野捕获。
            bob_protocol_id = wait_player_protocol_id(
                alice, username=bob.username, timeout=15.0
            )
            alice.intent(
                {
                    "type": "trade_offer_request",
                    "v": 1,
                    "target": f"entity:{bob_protocol_id}",
                    "offered_instance_id": int(talisman["item"]["instance_id"]),
                }
            )

            offer_event = bob.expect_server_data("trade_offer", timeout=10.0)
            offer = offer_event.data["payload"]
            assert str(offer.get("offer_id", "")).startswith("trade:"), (
                "trade_offer payload 应携带 trade: 前缀 offer_id（目标回执的寻址键），"
                f"实际 {offer.get('offer_id')!r}"
            )
            assert offer.get("offered_item", {}).get("item_id") == OFFER_ITEM_ID, (
                f"trade_offer 的 offered_item 应为 {OFFER_ITEM_ID}（发起者拿出的物品），"
                f"实际 {offer.get('offered_item')!r}"
            )
            assert int(offer["offered_item"]["instance_id"]) == int(
                talisman["item"]["instance_id"]
            ), (
                "trade_offer 的 offered_item.instance_id 应等于发起者背包中该物品的 instance_id，"
                f"实际 {offer['offered_item'].get('instance_id')!r}"
            )
            requested_ids = [
                item.get("item_id") for item in offer.get("requested_items", [])
            ]
            assert REQUEST_ITEM_ID in requested_ids, (
                f"trade_offer 的 requested_items 应包含 {REQUEST_ITEM_ID}（目标可回礼清单），"
                f"实际 {requested_ids}"
            )
            assert int(offer.get("expires_at_ms", 0)) > 0, (
                "trade_offer 应带 expires_at_ms 截止时间戳，实际 0"
            )
            assert all(
                e.data.get("payload_type") != "trade_offer"
                for e in alice.events_of("server_data")
            ), (
                "trade_offer payload 只应发给目标玩家（bob），发起者 alice 不应收到——"
                "发错了说明 dispatch 寻址把邀请回给了发起者"
            )

            alice_prev_rev = int(latest_inventory_snapshot(alice)["revision"])
            bob_prev_rev = int(latest_inventory_snapshot(bob)["revision"])
            bob.intent(
                {
                    "type": "trade_offer_response",
                    "v": 1,
                    "offer_id": offer["offer_id"],
                    "accepted": True,
                    "requested_instance_id": int(pill["item"]["instance_id"]),
                }
            )

            wait_inventory_revision_after_matching(
                alice,
                alice_prev_rev,
                lambda snap: find_item(snap, REQUEST_ITEM_ID) is not None
                and find_item(snap, OFFER_ITEM_ID) is None,
                description=(
                    f"alice 背包出现 {REQUEST_ITEM_ID} 且不再持有 {OFFER_ITEM_ID}"
                    "（换包生效）"
                ),
                timeout=10.0,
            )
            wait_inventory_revision_after_matching(
                bob,
                bob_prev_rev,
                lambda snap: find_item(snap, OFFER_ITEM_ID) is not None
                and find_item(snap, REQUEST_ITEM_ID) is None,
                description=(
                    f"bob 背包出现 {OFFER_ITEM_ID} 且不再持有 {REQUEST_ITEM_ID}"
                    "（换包生效）"
                ),
                timeout=10.0,
            )

            alice.assert_alive("交易换包后")
            bob.assert_alive("交易换包后")
