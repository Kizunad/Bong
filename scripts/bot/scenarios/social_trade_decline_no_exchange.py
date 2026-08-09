"""面对面交易拒绝与无效目标分支：accepted=false 不换包，坏 target 不发 offer。

协议契约面（server/src/social/mod.rs handle_trade_offer_responses / dispatch_trade_offers）：
- C2S `trade_offer_response{v,offer_id,accepted:false}`（真实 offer_id）：
  pending 移除且无任何背包变更、无新 snapshot 推送。
- C2S `trade_offer_request` 的 target 解析失败（`entity:` 不存在）/ self-target
  （initiator == target）：server 静默丢弃，目标玩家收不到任何 trade_offer payload。
- 负面断言用「窗口内无新 trade_offer / 无新 inventory_snapshot + 背包不变 + 连接存活」
  三合一锁住，避免只靠 sleep 假装验证。
"""

from __future__ import annotations

import time

from bot.scenarios._inventory_helpers import (
    find_item,
    latest_inventory_snapshot,
    require_item,
    wait_inventory_contains,
    wait_join_and_inventory,
)
from bot.scenarios._social_helpers import wait_player_protocol_id

DESCRIPTION = "面对面交易拒绝/坏 target 分支：decline 不换包，entity 不存在与 self-target 不发 offer"
MODULES = ["social", "trade", "network", "cmd", "multibot"]

OFFER_ITEM_ID = "starter_talisman"
REQUEST_ITEM_ID = "huiyuan_pill"
MISSING_TARGET = "entity:2147483647"  # i32 max 附近，fixture 世界不存在该协议实体


def _server_data_count(bot, payload_type: str) -> int:
    return sum(
        1
        for event in bot.events_of("server_data")
        if event.data.get("payload_type") == payload_type
    )


def _inventory_snapshot_count(bot) -> int:
    return _server_data_count(bot, "inventory_snapshot")


def run(env) -> None:
    with env.new_bot("DA") as alice:
        wait_join_and_inventory(alice)
        with env.new_bot("DB") as bob:
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
            bob_pill = require_item(
                wait_inventory_contains(bob, REQUEST_ITEM_ID, timeout=10.0),
                REQUEST_ITEM_ID,
            )

            # ── 分支①：真实 offer 被目标拒绝 → pending 移除，背包分毫不动 ──
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
            offer = bob.expect_server_data("trade_offer", timeout=10.0).data["payload"]

            alice_snapshots_before = _inventory_snapshot_count(alice)
            bob_snapshots_before = _inventory_snapshot_count(bob)
            bob.intent(
                {
                    "type": "trade_offer_response",
                    "v": 1,
                    "offer_id": offer["offer_id"],
                    "accepted": False,
                }
            )
            time.sleep(2.0)
            assert _inventory_snapshot_count(alice) == alice_snapshots_before, (
                "拒绝交易后 alice 不应收到新的 inventory_snapshot（换包才推快照），"
                "实际收到了新快照"
            )
            assert _inventory_snapshot_count(bob) == bob_snapshots_before, (
                "拒绝交易后 bob 不应收到新的 inventory_snapshot（换包才推快照），"
                "实际收到了新快照"
            )
            assert find_item(latest_inventory_snapshot(alice), OFFER_ITEM_ID) is not None, (
                f"拒绝后 alice 仍应持有 {OFFER_ITEM_ID}（accepted=false 不换包），"
                "实际背包里没了"
            )
            assert find_item(latest_inventory_snapshot(bob), OFFER_ITEM_ID) is None, (
                f"拒绝后 bob 不应持有 {OFFER_ITEM_ID}（accepted=false 不换包），"
                "实际背包里出现了"
            )

            # 拒绝的 pending 移除证明：同一 offer_id 重放 accepted=true 必须被拒。
            # 服务端若在拒绝时没把 pending 从 registry 移除（review 点名的违规实现），
            # 这次带完整 requested_instance_id 的 accept 就会触发换包——下面的
            # 「无新快照 + 双端背包不变」立即抓住。
            alice_accept_before = _inventory_snapshot_count(alice)
            bob_accept_before = _inventory_snapshot_count(bob)
            bob.intent(
                {
                    "type": "trade_offer_response",
                    "v": 1,
                    "offer_id": offer["offer_id"],
                    "accepted": True,
                    "requested_instance_id": int(bob_pill["item"]["instance_id"]),
                }
            )
            time.sleep(2.0)
            assert _inventory_snapshot_count(alice) == alice_accept_before, (
                "已拒绝 offer 重放 accept 后 alice 不应收到新 inventory_snapshot"
                "（pending 应已随拒绝移除，重放必须被静默丢弃）"
            )
            assert _inventory_snapshot_count(bob) == bob_accept_before, (
                "已拒绝 offer 重放 accept 后 bob 不应收到新 inventory_snapshot"
                "（pending 应已随拒绝移除，重放必须被静默丢弃）"
            )
            assert find_item(latest_inventory_snapshot(alice), OFFER_ITEM_ID) is not None, (
                f"已拒绝 offer 重放 accept 后 alice 仍应持有 {OFFER_ITEM_ID}（不得换包）"
            )
            assert find_item(latest_inventory_snapshot(bob), REQUEST_ITEM_ID) is not None, (
                f"已拒绝 offer 重放 accept 后 bob 仍应持有 {REQUEST_ITEM_ID}（不得换包）"
            )
            assert find_item(latest_inventory_snapshot(bob), OFFER_ITEM_ID) is None, (
                f"已拒绝 offer 重放 accept 后 bob 不应得到 {OFFER_ITEM_ID}（不得换包）"
            )

            # ── 分支②：target 解析失败（entity: 不存在）→ 静默丢弃 ──
            bob_offers_before = _server_data_count(bob, "trade_offer")
            alice.intent(
                {
                    "type": "trade_offer_request",
                    "v": 1,
                    "target": MISSING_TARGET,
                    "offered_instance_id": int(talisman["item"]["instance_id"]),
                }
            )
            time.sleep(2.0)
            assert _server_data_count(bob, "trade_offer") == bob_offers_before, (
                f"target={MISSING_TARGET} 解析失败时 bob 不应收到 trade_offer payload，"
                "实际收到了（resolve_trade_offer_target 漏了失效校验）"
            )

            # ── 分支③：self-target（initiator == target）→ 静默丢弃 ──
            # 用 alice 的**真实** protocol id（经 bob 视野发现）自指，命中
            # dispatch_trade_offers 的 initiator == target 门禁，而非解析失败。
            # 断言盯 alice（发起者 == 目标）：若实现去掉该门禁，错误投递会落到
            # alice 自己头上——盯无关的 bob 完全测不到这条契约（review 点名）。
            alice_protocol_id = wait_player_protocol_id(
                bob, username=alice.username, timeout=15.0
            )
            alice_offers_before = _server_data_count(alice, "trade_offer")
            alice.intent(
                {
                    "type": "trade_offer_request",
                    "v": 1,
                    "target": f"entity:{alice_protocol_id}",
                    "offered_instance_id": int(talisman["item"]["instance_id"]),
                }
            )
            time.sleep(2.0)
            assert _server_data_count(alice, "trade_offer") == alice_offers_before, (
                "self-target 交易请求应被静默丢弃（initiator == target 门禁），"
                "alice（发起者==目标）不应收到自己的 trade_offer payload"
            )
            assert _server_data_count(bob, "trade_offer") == bob_offers_before, (
                "self-target 交易请求同样不应把 payload 发给无关玩家 bob"
            )

            alice.assert_alive("拒绝/坏 target 分支执行后")
            bob.assert_alive("拒绝/坏 target 分支执行后")
