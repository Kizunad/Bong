"""暗器链：carrier 抛射（throw_carrier）的空手前置黑盒契约。

`throw_carrier_intents`（server/src/combat/carrier.rs）只从手槽持有的暗器载体读取
投掷物（清空手槽、生成弹道、命中耗尽发 `bong:combat/projectile_despawned`）。由于裸
暗器载体 `anqi_yibian_shougu` 在物品目录是 `category="misc"` 且 `validate_equip_to`
手槽档只放行 weapon/tool（见姊妹场景 combat_anqi_charge_carrier），真实客户端无法
把载体挂进 main_hand —— 因而抛射的合法命中路径同样不可端到端触达。

本场景锁定的是这条**空手护栏**：在无持载体状态下发出 throw_carrier intent，
服务器端静默不产生任何抛射——手上无伤、无 despawn 事件、无库存改动、玩家存活。
这证明抛射链在"无载体"输入下是搁置的非破坏性 no-op，不会误伤或误发事件。

空手 no-op 对 server→client 无可观测副作用，所以纯负向断言（无 despawn / 无
库存改动 / 存活）无法区分「护栏真的走了」和「意图在序列化/传输/反序列化环节被
丢弃的空转」。本场景因此追加**正向派发证据**：读 server 日志（需 `BONG_SERVER_LOG`
指向 server 日志，bot-e2e.sh 已导出），断言发出的 throw_carrier intent（精确
payload 字节数）确实产生了一条 `client_request received` 且没有 `deserialize
failed`——证明生产→线上→反序列化链条被实际遍历。

**已知缺口（记录不隐瞒）**：合法抛射（清空手部 + 弹道耗竭事件）的成功分支同样只在
server 单测里用直接写槽的 `inventory_with_main_hand()` helper 覆盖；真"装弹→抛射"
链路请在物品目录把载体档位回填为 anqi/hidden_weapon 后再补该场景。`client_request
received` 只能证明意图被反序列化成 ThrowCarrier 并派发；`throw_carrier_intents`
系统自身未被注册时（server 接线断裂）仍无可观测信号，属 server 侧 instrumentation
盲区。
"""

import json
import os
import re
import time

from bot.scenarios._inventory_helpers import (
    latest_inventory_snapshot,
    wait_join_and_inventory,
)
from bot._redis_helpers import RedisPubSub

DESCRIPTION = "抛射护栏：无持载体发出 throw_carrier → 静默 no-op（无 despawn 事件 / 无库存改动 / 存活）"
MODULES = ["anqi", "combat", "inventory"]
DEFAULT_ENABLED = False
REQUIRED_ENV = "BOT_E2E_ANQI_REDIS"
RUN_IN_ALL_WHEN_ENV = REQUIRED_ENV

DESPAWN_CH = "bong:combat/projectile_despawned"


def _self_carrier(bot) -> str:
    """从 server 周期下发的 CarrierState 取本 bot 的线缆 wire id。

    bong:combat/projectile_despawned 是全局频道，其他玩家的抛射也会发布到这里；
    server 每 tick 周期向每个客户端推送自身 CarrierState（field 49，
    carrier=`player:{uuid}`），用它把 despawn 事件归属钉到本 bot。
    """
    evt = bot.wait_for(
        lambda e: e.kind == "server_data"
        and e.data.get("payload_type") == "carrier_state"
        and e.data.get("payload", {}).get("carrier", "").startswith("player:"),
        timeout=15.0,
        description="server 下发本 bot 的 CarrierState（carrier=player: 线缆 id）",
    )
    return evt.data["payload"]["carrier"]


_RECEIVED_RE = re.compile(r"client_request received .*payload_bytes=(\d+)")
_DESERIALIZE_FAILED_RE = re.compile(r"client_request deserialize failed")


def _server_log_markers(path: str) -> tuple[list[int], int]:
    """扫描 server 日志，返回 (payload_bytes 序列, deserialize-failed 条数)。

    `client_request received ... payload_bytes=N` 是 handle_client_request_payloads
    在 JSON 反序列化成功后的 info 日志（N = 请求载荷字节数）；`client_request
    deserialize failed` 是 warn 日志，payload 与 ClientRequestV1 schema 不匹配时
    出现。两者构成 throw 意图「生产→线上→反序列化」的正向证据：空手 no-op 对
    server→client 无可观测副作用，只有这段日志能证明链条真的被遍历（review
    finding：无正向证据时，意图被序列化丢弃 / type 改名 / schema 失配都会
    静默空转却照样通过负向断言）。
    """
    received: list[int] = []
    failed = 0
    with open(path, "r", encoding="utf-8", errors="replace") as fh:
        for line in fh:
            match = _RECEIVED_RE.search(line)
            if match:
                received.append(int(match.group(1)))
            if _DESERIALIZE_FAILED_RE.search(line):
                failed += 1
    return received, failed


def run(env) -> None:
    pubsub = RedisPubSub.from_env()
    try:
        pubsub.subscribe(DESPAWN_CH)
        with env.new_bot("Throw") as bot:
            wait_join_and_inventory(bot)
            carrier = _self_carrier(bot)
            before = latest_inventory_snapshot(bot)
            prev_rev = int(before["revision"])
            initial_held = (before.get("equipped", {}).get("main_hand_held") or {}).get(
                "item_id"
            )
            assert initial_held is None or not initial_held.startswith("anqi_"), (
                f"前置：默认手槽应是非暗器武器（新手村 fixture 通常给 iron_sword），"
                f"实际 {initial_held!r}——若服务器默认就发暗器，本护栏前置失效需重选"
            )

            # payload 须命中 server 端 ClientRequestV1::ThrowCarrier 实际 schema
            # （slot/dir_unit/power；字段名写错会被 deny_unknown_fields 整包拒收，
            # throw_carrier_intents 根本不会执行）。main_hand 持默认武器（无 anqi
            # imprint），走 throw_carrier_intents 的 imprint 查找 miss 分支静默
            # no-op。request 对象即 bot.intent 实际发送的 JSON（同序同值），其
            # 字节长度必须与 server 日志 payload_bytes 严格一致。
            request = {
                "type": "throw_carrier",
                "v": 1,
                "slot": "main_hand",
                "dir_unit": [0.0, 0.0, 1.0],
                "power": 0.5,
            }
            payload_bytes = len(json.dumps(request).encode("utf-8"))

            log_path = os.environ.get("BONG_SERVER_LOG")
            assert log_path and os.path.isfile(log_path), (
                "正向派发证据需要 server 日志：export BONG_SERVER_LOG=<server log>"
                f"（bot-e2e.sh 已导出；实际 log_path={log_path!r}）"
            )
            before_received, before_failed = _server_log_markers(log_path)

            # 手持非暗器（默认武器）发出投掷 intent —— 无载体投掷应被静默忽略。
            bot.intent(request)

            # 正向派发证据（3s 兼作 despawn 观察窗口）：空手 no-op 对 server→client
            # 无可观测副作用，只有 server 日志的 `client_request received` 能证明这次
            # 生产→线上→反序列化路径被实际遍历。若 intent 被客户端序列化丢弃 / type
            # 改名 / schema 字段失配，负向断言依旧通过，正由此处拦下（review finding）。
            time.sleep(3.0)
            after_received, after_failed = _server_log_markers(log_path)
            new_received = after_received[len(before_received):]
            new_failed = after_failed - before_failed
            assert payload_bytes in new_received, (
                f"正向派发证据缺失：发送 throw_carrier intent（payload_bytes="
                f"{payload_bytes}）后 server 日志应新增同字节数 client_request "
                f"received，实际新增 {new_received!r}——意图可能在客户端序列化/"
                f"线上传输/服务端反序列化环节被丢弃，空手护栏断言无法证明链条走了"
            )
            assert new_failed == 0, (
                f"throw_carrier 请求不应触发 client_request deserialize failed，"
                f"实际新增 {new_failed} 条——payload 字段与 server 端 "
                f"ClientRequestV1::ThrowCarrier 不一致，链条在反序列化处断裂"
            )

            # 无本 bot 事件：窗口内不应冒出归属本 bot 的 projectile_despawned。
            # 该频道全局共享，订阅又先于 bot 建立，其他玩家的抛射也会发布进来；
            # 必须按 owner 过滤到本 bot，才证明空手抛射被静默忽略。
            fired = [
                e for e in pubsub.events_for(DESPAWN_CH) if e.get("owner") == carrier
            ]
            assert not fired, f"空手抛射不应产生本 bot 的 despawn 事件，实际 {fired!r}"

            # 无库存改动：revision 不变，且手持项仍为同一非暗器武器（未被清空/替换）。
            after = latest_inventory_snapshot(bot)
            assert int(after["revision"]) == prev_rev, (
                f"空抛射不应改动 inventory，revision {prev_rev} -> {after['revision']}"
            )
            still_held = (after.get("equipped", {}).get("main_hand_held") or {}).get(
                "item_id"
            )
            assert still_held == initial_held, (
                f"空手抛射不应触碰手槽（仍应持 {initial_held!r}），实际 {still_held!r}"
            )

            bot.assert_alive("空手抛射后")
    finally:
        pubsub.stop()