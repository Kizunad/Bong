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
丢弃的空转」。本场景因此追加**消费者信号**：`throw_carrier_intents` 在空手
no-op 早退处发 `[bong][combat] throw_carrier guard carrier=player:{uuid}
reason=<guard>` 日志（carrier.rs，review 要求补的 instrumentation）。场景读
server 日志（需 `BONG_SERVER_LOG`，bot-e2e.sh 已导出），断言发出 intent 后新增
了**归属本 bot 线缆 id** 的 guard 标记——既证明生产→线上→反序列化→派发→消费者
链条被实际遍历，又把证据相关性钉在本 bot 身份上（review finding [major]：
payload 字节数不唯一，无法把 `client_request received` 归属到本请求）。

**已知缺口（记录不隐瞒）**：合法抛射（清空手部 + 弹道耗竭事件）的成功分支同样只在
server 单测里用直接写槽的 `inventory_with_main_hand()` helper 覆盖；真"装弹→抛射"
链路请在物品目录把载体档位回填为 anqi/hidden_weapon 后再补该场景。guard 日志
只能证明空手护栏分支被执行，不覆盖弹道生成/命中/耗竭的成功路径。
"""

import os
import re
import time

from bot.scenarios._inventory_helpers import (
    latest_inventory_snapshot,
    wait_join_and_inventory,
)
from bot._redis_helpers import RedisPubSub
from bot._server_log import ServerLogScanner

DESCRIPTION = "抛射护栏：无持载体发出 throw_carrier → 静默 no-op（无 despawn 事件 / 无库存改动 / 存活）"
MODULES = ["anqi", "combat", "inventory"]
DEFAULT_ENABLED = False
REQUIRED_ENV = "BOT_E2E_ANQI_REDIS"
RUN_IN_ALL_WHEN_ENV = REQUIRED_ENV

DESPAWN_CH = "bong:combat/projectile_despawned"
# 消费者 guard 标记轮询上限：意图经线上→反序列化→派发→系统执行，tick 内完成，
# 8s 远超所需，同时给 despawn 观察留足窗口。
GUARD_TIMEOUT = 8.0
# 观察窗截止后的投递宽限：pump 的 recv/入队与场景的截止判定异步，server 在截止
# 前发布的 despawn 可能晚几 tick 才入队。settle 留出这段宽限做投递屏障，最终
# 扫描按「截止 + 宽限」的入队时刻边界捞回窗口内事件（review finding [major]）。
DELIVERY_GRACE = 0.5


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


_GUARD_RE = re.compile(
    r"throw_carrier guard carrier=(?P<carrier>\S+) .*reason=(?P<reason>\S+)"
)

# 空手护栏 reason 白名单：no_carrier_item（手槽无载体）/ no_anqi_imprint（持非暗器
# 物品，新手村 fixture 主手通常是 iron_sword）。若 fixture 换主手武器，两分支任一
# 出现都算护栏执行，但不接受其他 reason 蒙混。
_GUARD_REASONS = ("no_carrier_item", "no_anqi_imprint")


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
            # no-op。request 对象即 bot.intent 实际发送的 JSON（同序同值）。
            request = {
                "type": "throw_carrier",
                "v": 1,
                "slot": "main_hand",
                "dir_unit": [0.0, 0.0, 1.0],
                "power": 0.5,
            }

            log_path = os.environ.get("BONG_SERVER_LOG")
            assert log_path and os.path.isfile(log_path), (
                "正向派发证据需要 server 日志：export BONG_SERVER_LOG=<server log>"
                f"（bot-e2e.sh 已导出；实际 log_path={log_path!r}）"
            )
            # 按文件偏移增量扫描：全 run 共享的 server 日志随前面所有场景持续
            # 增长，若每 200ms 从头全扫会产生几十 GB I/O 且单次扫描时长不受
            # 截止时间约束（review finding [minor]：guard 轮询反复全量重扫无界
            # 日志）。ServerLogScanner 只读上次偏移之后的追加段。
            scanner = ServerLogScanner(log_path, _GUARD_RE)
            scanner.scan()
            before_guard = scanner.guard_markers(carrier)
            before_failed = scanner.deserialize_failed(bot.username)

            # despawn 观察窗起点锚定在 intent **之前**：错误生成的弹道会在 intent
            # 后几个 tick 内发布 despawn，若锚点取在 guard 轮询之后，intent 后早
            # 到的事件已入队、被 seq >= 锚点排除——那正是本场景要抓的泄漏。
            despawn_anchor = pubsub.anchor()

            # 手持非暗器（默认武器）发出投掷 intent —— 无载体投掷应被静默忽略。
            bot.intent(request)

            # 正向派发证据（主）：轮询等待消费者系统为本 bot 的 carrier 新增 guard
            # 标记。空手 no-op 对 server→client 无可观测副作用，唯有 throw_carrier_
            # intents 在空手早退处发出的这条消费者信号能证明链条被实际遍历，且由
            # carrier 线缆 id 归属到本 bot（review findings [major]：缺消费者信号 +
            # payload 字节数相关性不成立）。轮询窗口兼作 despawn 观察窗。
            guard_deadline = time.monotonic() + GUARD_TIMEOUT
            while True:
                scanner.scan()
                after_guard = scanner.guard_markers(carrier)
                if len(after_guard) > len(before_guard):
                    break
                if time.monotonic() >= guard_deadline:
                    break
                time.sleep(0.2)

            new_guard = after_guard[len(before_guard):]
            assert new_guard, (
                f"throw_carrier 空手护栏正向证据缺失：发出 intent 后 "
                f"{GUARD_TIMEOUT:.0f}s 内 server 未为 carrier={carrier!r} 新增 "
                f"guard 标记，实际新增 {new_guard!r}——throw_carrier_intents "
                f"消费者可能未注册、意图未抵达派发、或在序列化/传输环节被丢弃，"
                f"负向断言无法区分这些空转"
            )
            for reason in new_guard:
                assert reason in _GUARD_REASONS, (
                    f"guard 标记 reason 不在空手护栏白名单 {_GUARD_REASONS!r} 内："
                    f"{new_guard!r}"
                )

            # 辅助诊断：guard 标记只在反序列化成功后才会出现，故此计数在 guard
            # 断言通过后提供 schema 漂移的精确定位（若 schema 失配，guard 断言
            # 先行失败，这里不会掩盖）。归属按 user=<name> 字段精确匹配，避免
            # 重叠用户名子串误归因（review finding [minor]）。
            scanner.scan()
            after_failed = scanner.deserialize_failed(bot.username)
            new_failed = after_failed - before_failed
            assert new_failed == 0, (
                f"窗口内出现 client_request deserialize failed（新增 {new_failed} 条）"
                f"——有请求的 payload 与 server 端 ClientRequestV1 schema 不一致，"
                f"链条在反序列化处断裂"
            )

            # 无本 bot 事件：观察窗必须覆盖 intent 后的完整 GUARD_TIMEOUT 窗口，
            # 而不是在 guard 标记一出现就拍照——错误生成的弹道若在 guard 之后
            # 几个 tick 才创建并发布 despawn，快照会把它放走（review finding
            # [major]：no-despawn 断言可早于错误生成的弹道 despawn）。guard_
            # deadline 自 intent 发出起算，轮询到该截止为止，窗口内任何时刻到达
            # 的本 bot despawn 都算数。该频道全局共享，订阅又先于 bot 建立，
            # 其他玩家的抛射也会发布进来；必须按 owner 过滤到本 bot，才证明
            # 空手抛射被静默忽略。window_events 加锁按 seq >= 锚点（intent 前）只扫
            # 本窗口新增事件，其他玩家的抛射也落在窗口内，由 owner 过滤到本 bot。
            while True:
                fired = [
                    e
                    for e in pubsub.window_events(DESPAWN_CH, after=despawn_anchor)
                    if e.get("owner") == carrier
                ]
                assert not fired, (
                    f"空手抛射不应产生本 bot 的 despawn 事件，实际 {fired!r}"
                )
                if time.monotonic() >= guard_deadline:
                    break
                time.sleep(0.2)

            # 观察窗截止后的投递屏障 + 最终窗口化扫描：截止判定与 pump 的
            # recv/入队异步，server 在截止前发布的 despawn 可能仍滞留在 socket/
            # 缓冲里、截止判定通过后 pump 才把它入队——上面的循环在快照与截止
            # 判定之间退出时，直接 done 会漏掉窗口内事件（review finding
            # [major]）。settle 留出投递宽限让 pump 收尾，最终扫描按 seq >= 锚点
            # 且 入队时刻 <= 截止 + 宽限 的边界，把窗口内最后入队的事件捞回。
            pubsub.settle(DELIVERY_GRACE)
            fired = [
                e
                for e in pubsub.window_events(
                    DESPAWN_CH,
                    after=despawn_anchor,
                    max_ts=guard_deadline + DELIVERY_GRACE,
                )
                if e.get("owner") == carrier
            ]
            assert not fired, (
                f"空手抛射不应产生本 bot 的 despawn 事件，实际 {fired!r}"
            )

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