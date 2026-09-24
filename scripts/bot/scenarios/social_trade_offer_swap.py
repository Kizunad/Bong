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

import time

from bot.scenarios._inventory_helpers import (
    find_item,
    inventory_instance_map,
    latest_inventory_snapshot,
    require_item,
    wait_inventory_revision_after_matching,
    wait_join_and_inventory,
)
from bot.scenarios._social_helpers import wait_player_protocol_id

DESCRIPTION = "双 bot 面对面交易：A 以带元数据矿物换 B 的另一份矿物，双方背包原子互换"
MODULES = ["social", "trade", "network", "cmd", "multibot"]

# Bare mineral grants carry non-default `mineral_id` and `freshness` metadata;
# this makes the preservation oracle mutation-sensitive instead of comparing
# two default-valued fixtures.
OFFER_COMMAND_ID = "za_gang"
OFFER_ITEM_ID = "mineral_za_gang"
REQUEST_COMMAND_ID = "ling_shi_zhong"
REQUEST_ITEM_ID = "mineral_ling_shi_zhong"


def run(env) -> None:
    with env.new_bot("TA") as alice:
        wait_join_and_inventory(alice)
        with env.new_bot("TB") as bob:
            wait_join_and_inventory(bob)

            # 清包归一化：server data dir 跨 run 持久化，复跑时上一轮的 give 物品
            # 会污染断言（背包已满给不进 / find_item 误判），先清 pack 再发。
            alice_join_revision = int(latest_inventory_snapshot(alice)["revision"])
            bob_join_revision = int(latest_inventory_snapshot(bob)["revision"])
            alice.cmd("clearinv naked")
            alice.expect_chat("[dev] clearinv All revision=", timeout=10.0)
            bob.cmd("clearinv naked")
            bob.expect_chat("[dev] clearinv All revision=", timeout=10.0)

            alice_cleared = wait_inventory_revision_after_matching(
                alice,
                alice_join_revision,
                lambda snap: not inventory_instance_map(snap),
                description="clearinv 后 authoritative inventory 为空",
                timeout=10.0,
            )
            bob_cleared = wait_inventory_revision_after_matching(
                bob,
                bob_join_revision,
                lambda snap: not inventory_instance_map(snap),
                description="clearinv 后 authoritative inventory 为空",
                timeout=10.0,
            )

            alice.cmd(f"give {OFFER_COMMAND_ID} 1")
            bob.cmd(f"give {REQUEST_COMMAND_ID} 1")

            alice_snapshot = wait_inventory_revision_after_matching(
                alice,
                int(alice_cleared["revision"]),
                lambda snap: find_item(snap, OFFER_ITEM_ID) is not None,
                description=f"清包后新发的 {OFFER_ITEM_ID} 已进入 authoritative inventory",
                timeout=10.0,
            )
            talisman = require_item(alice_snapshot, OFFER_ITEM_ID)
            bob_snapshot = wait_inventory_revision_after_matching(
                bob,
                int(bob_cleared["revision"]),
                lambda snap: find_item(snap, REQUEST_ITEM_ID) is not None,
                description=f"清包后新发的 {REQUEST_ITEM_ID} 已进入 authoritative inventory",
                timeout=10.0,
            )
            pill = require_item(bob_snapshot, REQUEST_ITEM_ID)
            before_by_owner = {
                "alice": inventory_instance_map(alice_snapshot),
                "bob": inventory_instance_map(bob_snapshot),
            }
            before_all = {**before_by_owner["alice"], **before_by_owner["bob"]}
            assert len(before_all) == sum(len(items) for items in before_by_owner.values()), (
                "交易前双方 authoritative snapshot 出现重复 instance_id，"
                f"无法证明唯一所有权：alice={sorted(before_by_owner['alice'])} "
                f"bob={sorted(before_by_owner['bob'])}"
            )
            for instance_id, item in before_all.items():
                assert item.get("mineral_id"), (
                    f"交易 fixture instance_id={instance_id} 必须携带非默认 mineral_id，"
                    f"实际 item={item!r}"
                )
            assert any(item.get("freshness") is not None for item in before_all.values()), (
                "交易 fixture 至少一件物品必须携带非默认 freshness 元数据，"
                f"实际 items={before_all!r}"
            )

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
            response_anchor = {
                "alice": max((event.t for event in alice.events), default=0.0),
                "bob": max((event.t for event in bob.events), default=0.0),
            }
            bob.intent(
                {
                    "type": "trade_offer_response",
                    "v": 1,
                    "offer_id": offer["offer_id"],
                    "accepted": True,
                    "requested_instance_id": int(pill["item"]["instance_id"]),
                }
            )

            alice_final = wait_inventory_revision_after_matching(
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
            bob_final = wait_inventory_revision_after_matching(
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

            # 覆盖成功响应后的完整有界窗口，抓住显式 send + Changed emitter 的延迟双发。
            time.sleep(3.0)
            after_events = {
                "alice": [
                    event
                    for event in alice.events_of("server_data")
                    if event.t > response_anchor["alice"]
                    and event.data.get("payload_type") == "inventory_snapshot"
                ],
                "bob": [
                    event
                    for event in bob.events_of("server_data")
                    if event.t > response_anchor["bob"]
                    and event.data.get("payload_type") == "inventory_snapshot"
                ],
            }
            assert len(after_events["alice"]) == 1, (
                "成功交易后的完整 3 秒窗口内 alice 必须恰好收到一条最终 inventory_snapshot，"
                f"实际 {len(after_events['alice'])} 条"
            )
            assert len(after_events["bob"]) == 1, (
                "成功交易后的完整 3 秒窗口内 bob 必须恰好收到一条最终 inventory_snapshot，"
                f"实际 {len(after_events['bob'])} 条"
            )
            assert after_events["alice"][0].data["payload"] == alice_final
            assert after_events["bob"][0].data["payload"] == bob_final

            after_by_owner = {
                "alice": inventory_instance_map(alice_final),
                "bob": inventory_instance_map(bob_final),
            }
            after_all = {**after_by_owner["alice"], **after_by_owner["bob"]}
            assert len(after_all) == sum(len(items) for items in after_by_owner.values()), (
                "交易后原始 instance_id 必须在双方中唯一归属，"
                f"实际 alice={sorted(after_by_owner['alice'])} bob={sorted(after_by_owner['bob'])}"
            )
            assert set(after_all) == set(before_all), (
                "成功交易必须守恒双方 authoritative snapshot 的完整 instance multiset；"
                f"before={sorted(before_all)} after={sorted(after_all)}"
            )

            offered_id = int(talisman["item"]["instance_id"])
            requested_id = int(pill["item"]["instance_id"])
            for instance_id in before_all:
                expected_owner = (
                    "bob"
                    if instance_id == offered_id
                    else "alice"
                    if instance_id == requested_id
                    else "alice"
                    if instance_id in before_by_owner["alice"]
                    else "bob"
                )
                assert instance_id in after_by_owner[expected_owner], (
                    f"instance_id={instance_id} 交易后 owner 应为 {expected_owner}，"
                    f"实际 alice={sorted(after_by_owner['alice'])} bob={sorted(after_by_owner['bob'])}"
                )
                for field in (
                    "item_id",
                    "display_name",
                    "grid_width",
                    "grid_height",
                    "weight",
                    "rarity",
                    "description",
                    "stack_count",
                    "spirit_quality",
                    "durability",
                    "mineral_id",
                    "scroll_kind",
                    "scroll_skill_id",
                    "scroll_xp_grant",
                    "charges",
                    "forge_quality",
                    "forge_color",
                    "forge_side_effects",
                    "forge_achieved_tier",
                    "alchemy",
                    "freshness",
                ):
                    assert after_all[instance_id].get(field) == before_all[instance_id].get(field), (
                        f"instance_id={instance_id} 的 {field} 必须跨交易保真；"
                        f"before={before_all[instance_id].get(field)!r} "
                        f"after={after_all[instance_id].get(field)!r}"
                    )

            alice.assert_alive("交易换包后")
            bob.assert_alive("交易换包后")
