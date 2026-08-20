"""延寿棺 CoffinPlace 全生命周期黑盒：放置消费 → 重复/非空/过远/异维拒绝 → 破坏清场后可重放。

plan-coffin-v1 放置链路（client_request_handler.rs → handle_coffin_place_requests）黑盒契约面：

- `coffin_place{v,x,y,z,item_instance_id}` 必须用真实 instance_id（inventory_item_by_instance
  按 id 校验）。放置成功消费**恰好一个实例**（consume_item_instance_once）；失败路径分文不动。
- 重复放置（already registered）在 registry 双键（lower/upper）碰撞处拒绝，**先于消费**——
  第二实例必须保留在背包（防双注册双扣料 / double-grant，本家族最重的重复/双发检查）。
  双键碰撞的两条分支都被演练：新 lower==既有 lower 的精确重复、以及新 lower==既有
  upper（=lower+1x）的 overlap 探针——只查 lower 键、漏查 upper 键的错误实现（接受
  overlap 放置并吞掉实例）在第二探针处红（review finding [major]）。
- 非空气目标（not empty）、距离 > 6.0（coffin_target_is_close 36.0 平方）均**静默拒绝**
  （warn 无回执）——只能靠实例计数不变断言。距离边界用 `_distance_boundary_pair` 的
  (pass, fail) 目标对夹紧：pass=空气层中 d2≤35.0 的**最大**值、fail=d2>36.0 的**最小**
  值——(35.0, 36.0] 区间整块 target 无法命中（bot 脚面 y 与目标块中心 y 恒非整差），
  `<= 36.0` 与 `< 36.0` 的排他区分需要**精确 d2==36.0**：exact 探针把 bot 精确
  set_position 到 (tx+6.5, ty+0.5, tz+0.5)（整数减半整数，IEEE 精确），server 算得
  dx=6.0/dy=0/dz=0 → d2==36.0，正确实现（含边界）必须接受并 spawn marker，排他实现
  在此误拒而红（review finding [major] round 6：旧场景只测固定 offset=10 的过远拒绝，
  比较方向无「恰内侧 + 恰外侧」配对保护；round 8：pass/fail 把比较方向夹在
  [≤35, >36] 仍放走排他实现，exact 探针补上 36.0 本身）。
- 异维（coffin_requires_overworld fail-closed，先于近距/实例校验）拒绝并逐字 chat：
  `§c[棺] 你不在主世界，无法操作延寿棺。`——对**完整文本**（含 §c 色码、[棺] 前缀、
  句末句号）整体相等断言，不接受任意前缀/后缀（review finding [major]：子串匹配
  放行「错误的前缀：你不在主世界…（错误后缀）」式的错误回复）。
- 拒绝路径（重复/非空气/过远/异维/无效实例 id）**不得 spawn 或注册任何 coffin marker**——
  有界窗口内**任何**新建 kind=160 实体即红，**不按位置过滤**——拒绝请求却把 marker
  错放到偏移/默认坐标的错误实现同样红（review finding [major]：旧实现按目标位置过滤，
  「目标附近无 spawn」≠「拒绝即无 spawn」；位置匹配只用于正向放置断言）。
  负向断言只见证「无 client-visible spawn」：先插入 registry 双键再早退（不消费、不
  spawn）的错误实现能全过——故**每个拒绝段**后都对**被拒坐标**做 state-applying 复查
  （合法实例成功放置 + marker spawn + teardown break）：拒绝时错误登记双键会使复查以
  already-registered 拒绝而超时红，暴露 registry-only 残留（review finding [major]
  round 6：复查此前只覆盖 stale_pos）。过远拒绝的 fail_t 靠近后必须能合法放置（最直接
  的残留探针）；非空气拒绝坐标（py-1，永久实心）无法放置——不能取无关空气格凑数
  （review finding [major] round 7：旧复查把 (px,py+1,pz+2) 当替代坐标，y/z 双偏，
  完全不碰被拒坐标的 registry 键，残留永不暴露）——改用 coffin_enter 探针直接对
  (px,py-1,pz) 施加状态：正确实现对该实心坐标无 registry 条目 → lookup 静默 no-op；
  错误登记的残留被命中 → 进棺状态转变（in_coffin:true / invisible / 瞬移）红。
  stale 复查仍覆盖 stale_pos。
- `coffin_place` 必须用真实 instance_id（inventory_item_by_instance 按 id 校验）；
  已消费的 stale id 同样拒绝、分文不动（review finding [major]：全场景只用合法 id，
  无视 item_instance_id 吞掉可用棺材的实现能全绿）。
- `coffin_break` 破坏单发（remove_by_pos → None 后静默 continue），破坏后 registry 清空——
  同一坐标再放置必须成功（"destroy path leaves nothing behind"）；且**空→空重复破坏**
  必须是无副作用 no-op：不 panic / 不断连 / 不再 despawn / 不扣料 / 不加料
  （review finding [major]：只演练了 populated→empty 一次，empty→empty 从未被断言）。
  首次 break 的全量库存基线必须锚在 break **之前**（review finding [major]：旧基线取在
  break 完成之后，despawn 时 grant 非棺回收料的错误实现把料并入基线、重复 break 的
  total_baseline+1 断言照过）——break 后 +1 give 快照的全量必须恰好 = break 前全量 + 1。

**世界侧观察（本场景对实体生命周期的直接断言，非代理）**：
- 放置成功必须在世界 spawn mundane_coffin marker 渲染实体（entity kind=160，
  `coffin_marker_position(lower)=(x+1, y, z+0.5)`）——「世界确实获得棺实体」；
  只消费实例而无实体 spawn 在此红。
- `coffin_break` 必须把该 marker 实体 despawn（`entities_destroy` 含首放 entity_id）——
  marker 无方块无 block update，entity 层移除（valence `Despawned` → S2C_ENTITIES_DESTROY）
  是唯一清场信号；只移 registry 条目而留 stale 实体在此红。
- 破坏后同一坐标重放必须再次 spawn 新 marker 实体——旧实体已 despawn + registry 清空的
  完整闭环。
- 异维拒绝的 world 侧在 tsy 内观察（跨维时主世界实体不可见），返回主世界后**必须再扫**：
  既有原 marker 以同 entity_id 重流（合法），任何新建且 entity_id != 原 marker 的
  kind=160 spawn 即孤儿——错误实现绕过/独立于维度守卫、在主世界实体层创建 marker，
  tsy 侧观察窗看不见、返回才流给 client，旧实现返回后直接进 break 断言（只等原 marker
  destroy），孤儿以不同 entity_id 存活至场景结束（review finding [major] round 8）。
- 场景收尾（全部 teardown break 之后）从事件流重建 live kind=160 实体集并断言为空——
  任何 spawn 未配对 destroy 的 marker（异维孤儿、重复放置孤儿、漏破的 teardown marker）
  在此红，锁定「destroy path 与拒绝路径不留任何世界 marker 残留」（round 8 收口）。

**场景零残留（review finding [major] round 6）**：本场景自建的一切 coffin（stale 复查、
距离边界复查、最终重放）都以 `coffin_break` 收尾——旧场景把两个注册棺（stale_pos 复查
+ place_pos 重放）+ 两个 marker 实体永久留在 server，同一长活 E2E server 重跑时这些
坐标已注册 → 首次放置以 already-registered 拒绝、count 断言超时红，且污染同 fixture 区
的后续场景。场景结束时零注册棺、零世界 marker 残留。

所有断言走实例计数（stack_count 求和，跨容器/装备/快捷栏），不是 presence——count 才是
复制增殖的暴露面。拒绝路径服务端静默（无回执快照），用「give 一棺 + 等其
inventory_snapshot 计数达到期望值」作 server-authoritative 水位线：give 快照落在同 tick
的 client_request 处理之后，收到它既证明此前每个 C2S（含静默拒绝）已处理完毕，又以该
计数断言「拒绝未扣料」（tutorial_coffin_open/gap12 同款约定）。

超时口径：45s——CI e2e 串行连跑长 server 后 TPS 退化（对齐 forge 场景先例），断言不放宽。
"""

from __future__ import annotations

import time

from bot.bot import BotAssertionError
from bot.scenarios._combat_helpers import last_event_time, wait_for_ready
from bot.scenarios._inventory_helpers import first_free_cell, latest_inventory_snapshot, send_move

DESCRIPTION = "CoffinPlace 生命周期：放置消费→重复/非空/过远（6m边界成对）/异维/stale实例拒绝（计数不变+无实体spawn）→单发破坏+空→空重破no-op→清场重放→全量teardown"
MODULES = ["coffin", "inventory", "dimension"]

COFFIN_ID = "mundane_coffin"
PLACE_OFFSET = -2  # 横向偏移，与 forge 放砧一致
# 放置目标从 bot 当前整数 y 向上逐层探测：生产出生点可能落在 raster 覆盖外的动态
# 地形/结构上，不能把固定 surface_y 或 py+1 当成空气保证。成功层会回写给所有后续
# 空气、边界和 stale 探针；非空气拒绝目标同样由真实放置消费结果向下探测。
AIR_LAYER_SEARCH_OFFSETS = range(1, 6)
# 拒绝路径没有 server 回执；一次周期快照足以观察保持不变。每次重置位置后再探针，
# 防止出生点处于未加载支撑面时，等待多个 5s 窗口让玩家重力下坠到放置半径外。
# marker spawn / not-empty 判定都在同一 tick 路径完成；窗口过长会让无支撑出生点在
# 拒绝探针之间下坠，下一层被错误归因于 too far。完整拒绝世界事件仍用 2s 窗口。
_AIR_LAYER_PROBE_TIMEOUT = 0.4
# marker entity spawn is emitted on a later server tick than the inventory snapshot;
# under the Redis-loaded runtime fixture the tail reached just over one second. Give
# temporary successful probes enough time to be observed and broken before the next
# rejection assertion window.
_MARKER_PROBE_TIMEOUT = 2.0
# Spawn columns may sit above a cave or a large unsupported gap.  Exponential samples
# reach the world floor without spending one full place/break/give cycle per air block.
_NON_AIR_PROBE_DEPTHS = (1, 2, 4, 8, 16, 32, 64, 128)
DISTANCE_SEARCH_RANGE = 8  # _distance_boundary_pair 的扫描半径：夹紧 6.0m 距离边界两侧的空气块
# server 放置距离契约（coffin/mod.rs coffin_target_is_close）：bot 位置到 target 块中心
# 平方距离 <= 36.0（6.0m 含边界）视为可放置，> 36.0 拒绝。
COFFIN_INTERACT_MAX_DISTANCE_SQ = 36.0
# pass 侧安全阈：client bot.position 与 server 权威位置有亚格残差 δ，d2 会抖动；
# pass_t 取 d2 <= 35.0（≈5.92m）留 1.0 平方余量——仍是「恰在边界内侧」，能捕获把半径
# 缩到 <5.92m 的错误实现，又不至于被位置抖动推过 36.0 造成假红。fail 侧取 min d2 > 36.0
# （fail 拒绝发生在 bot 静止段，位置抖动≈0，无需放宽）。
_DISTANCE_PASS_SQ = 35.0
# 完整、逐字的异维拒绝文本（服务端 COFFIN_DIMENSION_REJECTION_MESSAGE，coffin/mod.rs:1313）。
# 断言用整体相等（e.data["text"] == DIMENSION_REJECT_TEXT），含 §c 色码、[棺] 前缀与句号——
# 子串匹配会让「任意前缀/后缀包裹同一核心句」的错误回复漏网（review finding [major]）。
DIMENSION_REJECT_TEXT = "§c[棺] 你不在主世界，无法操作延寿棺。"

_STEP_TIMEOUT = 45.0
# 拒绝路径世界侧断言的有界观察窗（秒，事件时间）：inventory_snapshot 与实体
# spawn/destroy 走不同时序路径（reader 线程后台 append），「先发快照、后排队实体
# 事件」的实现会让晚到事件在持锁扫完当前前缀后漏检（review finding [major]）。
# 等待必须**有界**——负向断言不无限等，deadline = after_t + window 是确定性终点。
# 异维段必须**等满整窗仍在异维**（返回主世界后既有 marker 会重流，故返回锚点取在
# 观察结束之后）——以 sleep 提前闭合观察窗会让晚发（>0.5s）的错误 marker 漏检
# （review finding [major]）。
_REJECT_OBS_WINDOW = 2.0


def _stack_count(item) -> int:
    return int(round(float(item.get("stack_count", 1))))


def _coffin_count(snapshot) -> int:
    """统计 snapshot 中 mundane_coffin 的总持有数（跨容器/装备/快捷栏求和）。"""
    total = 0
    for placed in snapshot.get("placed_items", []):
        item = placed["item"]
        if item["item_id"] == COFFIN_ID:
            total += _stack_count(item)
    for slot, values in snapshot.get("equipped", {}).items():
        items = values if isinstance(values, list) else [values]
        for item in items:
            if item and item["item_id"] == COFFIN_ID:
                total += _stack_count(item)
    for item in snapshot.get("hotbar", []):
        if item and item["item_id"] == COFFIN_ID:
            total += _stack_count(item)
    return total


def _total_items(snapshot) -> int:
    """全量持有数（跨容器/装备/快捷栏求和）——空→空重复 break 的「不得加料/扣料」判据。

    _coffin_count 只看棺材；重复破坏若错误地再 grant 回收材料，棺材数不变而全量数
    超基线，此函数暴露它（review finding [major]：无副作用 no-op 断言）。
    """
    total = 0
    for placed in snapshot.get("placed_items", []):
        total += _stack_count(placed["item"])
    for slot, values in snapshot.get("equipped", {}).items():
        items = values if isinstance(values, list) else [values]
        for item in items:
            if item:
                total += _stack_count(item)
    for item in snapshot.get("hotbar", []):
        if item:
            total += _stack_count(item)
    return total


def _template_counts(snapshot) -> dict[str, int]:
    """按 item_id 汇总持有量，供 break 回收副作用做契约级差分。"""
    counts: dict[str, int] = {}

    def add(item) -> None:
        if item:
            item_id = item["item_id"]
            counts[item_id] = counts.get(item_id, 0) + _stack_count(item)

    for placed in snapshot.get("placed_items", []):
        add(placed["item"])
    for values in snapshot.get("equipped", {}).values():
        items = values if isinstance(values, list) else [values]
        for item in items:
            add(item)
    for item in snapshot.get("hotbar", []):
        add(item)
    return counts


def _first_coffin_instance_id(snapshot) -> int:
    for placed in snapshot.get("placed_items", []):
        item = placed["item"]
        if item["item_id"] == COFFIN_ID:
            return int(item["instance_id"])
    for slot, values in snapshot.get("equipped", {}).items():
        items = values if isinstance(values, list) else [values]
        for item in items:
            if item and item["item_id"] == COFFIN_ID:
                return int(item["instance_id"])
    for item in snapshot.get("hotbar", []):
        if item and item["item_id"] == COFFIN_ID:
            return int(item["instance_id"])
    raise BotAssertionError(f"期望 inventory_snapshot 中有 {COFFIN_ID} 实例可取，实际没有")


def _latest_coffin_instance_id(snapshot) -> int:
    """取本轮最新 /give 产生的实例（allocator id 单调递增）。"""
    ids = []
    for placed in snapshot.get("placed_items", []):
        if placed["item"]["item_id"] == COFFIN_ID:
            ids.append(int(placed["item"]["instance_id"]))
    for values in snapshot.get("equipped", {}).values():
        items = values if isinstance(values, list) else [values]
        ids.extend(
            int(item["instance_id"])
            for item in items
            if item and item["item_id"] == COFFIN_ID
        )
    ids.extend(
        int(item["instance_id"])
        for item in snapshot.get("hotbar", [])
        if item and item["item_id"] == COFFIN_ID
    )
    if not ids:
        raise BotAssertionError(f"inventory_snapshot 中没有 {COFFIN_ID} 实例")
    return max(ids)


def _coffin_item_by_instance(snapshot, instance_id: int):
    """按真实 instance_id 找棺材及其当前位置，避免 hotbar 多棺时取错实例。"""
    for placed in snapshot.get("placed_items", []):
        if placed["item"]["instance_id"] == instance_id:
            return placed["item"], {
                "kind": "container",
                "container_id": placed["container_id"],
                "row": placed["row"],
                "col": placed["col"],
            }
    for slot, values in snapshot.get("equipped", {}).items():
        items = values if isinstance(values, list) else [values]
        for item in items:
            if item and item["instance_id"] == instance_id:
                state = "worn" if slot.endswith("_worn") else "held"
                equip_slot = slot.rsplit("_", 1)[0]
                return item, {"kind": "equip", "slot": equip_slot, "state": state}
    for index, item in enumerate(snapshot.get("hotbar", [])):
        if item and item["instance_id"] == instance_id:
            return item, {"kind": "hotbar", "index": index}
    raise BotAssertionError(
        f"inventory_snapshot 中找不到 {COFFIN_ID} instance_id={instance_id}"
    )


def _park_coffin_in_hotbar(bot, instance_id: int, slot: int) -> None:
    """把被拒棺材移到独立 hotbar 槽，给下一个真实 /give 腾出 pack。"""
    snapshot = latest_inventory_snapshot(bot)
    _, source = _coffin_item_by_instance(snapshot, instance_id)
    anchor = last_event_time(bot)
    send_move(bot, instance_id, source, {"kind": "hotbar", "index": slot})
    bot.wait_for(
        lambda e: e.kind == "server_data"
        and e.data["payload_type"] == "inventory_snapshot"
        and e.t > anchor
        and any(
            hotbar_item is not None
            and hotbar_item.get("instance_id") == instance_id
            for hotbar_item in e.data["payload"].get("hotbar", [])
        ),
        timeout=_STEP_TIMEOUT,
        description=f"把棺材实例 {instance_id} 移入 hotbar[{slot}]，释放 pack 空间",
    )


def _park_non_coffin_items_in_body_pocket(bot) -> None:
    """把 break 回收材料从 pack 移到 body_pocket，保留 2x3 棺材的完整 footprint。"""
    while True:
        snapshot = latest_inventory_snapshot(bot)
        placed = next(
            (
                entry
                for entry in snapshot.get("placed_items", [])
                if entry.get("container_id", "").startswith("pack_")
                and entry["item"].get("item_id") != COFFIN_ID
            ),
            None,
        )
        if placed is None:
            return
        item = placed["item"]
        row, col = first_free_cell(
            snapshot,
            "body_pocket",
            int(item.get("grid_width", item.get("grid_w", 1))),
            int(item.get("grid_height", item.get("grid_h", 1))),
        )
        instance_id = int(item["instance_id"])
        anchor = last_event_time(bot)
        send_move(
            bot,
            instance_id,
            {
                "kind": "container",
                "container_id": placed["container_id"],
                "row": placed["row"],
                "col": placed["col"],
            },
            {"kind": "container", "container_id": "body_pocket", "row": row, "col": col},
        )
        bot.wait_for(
            lambda e: e.kind == "server_data"
            and e.data["payload_type"] == "inventory_snapshot"
            and e.t > anchor
            and any(
                entry.get("container_id") == "body_pocket"
                and entry["item"].get("instance_id") == instance_id
                for entry in e.data["payload"].get("placed_items", [])
            ),
            timeout=_STEP_TIMEOUT,
            description=f"把回收材料实例 {instance_id} 移入 body_pocket，释放棺材 footprint",
        )


def _clear_probe_reclaim_materials(bot) -> None:
    """清掉临时探针 break 产生的回收材料，避免污染后续棺材 footprint。

    这些材料来自仅用于寻找空气/验证 registry 的临时棺，不属于正式 break 的库存
    断言。运行时 pack id 是 ``pack_<instance_id>``，dev 命令的 ``clearinv pack``
    只匹配旧静态 id；这里用已验证的 ``clearinv all`` 清空携带面，再逐个恢复现有
    棺材到 hotbar，防止误删尚未消费的测试实例。
    """
    snapshot = latest_inventory_snapshot(bot)
    coffin_count = _coffin_count(snapshot)
    pack_items = [
        entry
        for entry in snapshot.get("placed_items", [])
        if entry.get("container_id", "").startswith("pack_")
    ]
    if not any(entry["item"].get("item_id") != COFFIN_ID for entry in pack_items):
        return

    anchor = last_event_time(bot)
    bot.cmd("clearinv all")
    bot.wait_for(
        lambda e: e.kind == "server_data"
        and e.data["payload_type"] == "inventory_snapshot"
        and e.t > anchor
        and _coffin_count(e.data["payload"]) == 0
        and not e.data["payload"].get("placed_items")
        and not any(e.data["payload"].get("hotbar", [])),
        timeout=_STEP_TIMEOUT,
        description="清掉临时探针 break 的携带面回收材料，准备恢复 hotbar 棺材",
    )

    for slot in range(coffin_count):
        give_anchor = last_event_time(bot)
        bot.cmd(f"give {COFFIN_ID} 1")
        given = bot.wait_for(
            lambda e: e.kind == "server_data"
            and e.data["payload_type"] == "inventory_snapshot"
            and e.t > give_anchor
            and _coffin_count(e.data["payload"]) >= 1,
            timeout=_STEP_TIMEOUT,
            description=f"恢复探针清理前的第 {slot + 1}/{coffin_count} 口棺材",
        )
        _park_coffin_in_hotbar(
            bot, _latest_coffin_instance_id(given.data["payload"]), slot
        )


def _wait_coffin_count(bot, expected, after_t, timeout=_STEP_TIMEOUT, description=None):
    event = bot.wait_for(
        lambda e: e.kind == "server_data"
        and e.data["payload_type"] == "inventory_snapshot"
        and e.t > after_t
        and _coffin_count(e.data["payload"]) == expected,
        timeout=timeout,
        description=description or f"mundane_coffin count=={expected} 的 inventory_snapshot",
    )
    return event.data["payload"]


def _coffin_locations(snapshot) -> str:
    """诊断用：列出 coffin 实例分布（location → 数量），暴露计数器漏扫的载体。"""
    parts = []
    for placed in snapshot.get("placed_items", []):
        item = placed["item"]
        if item["item_id"] == COFFIN_ID:
            parts.append(
                f"container:{placed.get('container_id', '?')}/r{placed.get('row')}c{placed.get('col')}"
                f"#{item.get('stack_count', 1)}"
            )
    for slot, values in snapshot.get("equipped", {}).items():
        items = values if isinstance(values, list) else [values]
        for item in items:
            if item and item["item_id"] == COFFIN_ID:
                parts.append(f"equip:{slot}#{item.get('stack_count', 1)}")
    for i, item in enumerate(snapshot.get("hotbar", [])):
        if item and item["item_id"] == COFFIN_ID:
            parts.append(f"hotbar:{i}#{item.get('stack_count', 1)}")
    return ", ".join(parts) or "∅"


def _container_debug(snapshot) -> str:
    """诊断容器容量/占用，定位动态探针后的 grant full。"""
    descriptions = []
    for container in snapshot.get("containers", []):
        items = [
            f"{placed['item'].get('item_id')}#{placed['item'].get('instance_id')}"
            f"@{placed.get('row')},{placed.get('col')}"
            for placed in snapshot.get("placed_items", [])
            if placed.get("container_id") == container.get("id")
        ]
        descriptions.append(
            f"{container.get('id')}={container.get('rows')}x{container.get('cols')}"
            f"[{';'.join(items) or '∅'}]"
        )
    return ", ".join(descriptions) or "∅"


def _give_barrier(bot, expected_count, timeout=_STEP_TIMEOUT, description=None):
    """give 一棺并以该 give 的 inventory_snapshot 作 server 权威水位线 + 断言载体。

    不用 give 的 chat ack 当水位线：实测（run4 证据）ack 在 client_request 处理**之前**
    发出（chat 命令 pass 先于 client_request pass），且在紧邻 give→give 交错下 ack 会丢
    （revision=14/16 收到、barrier 的 r17 未到）；而 give 的 inventory_snapshot 落在同 tick
    的 client_request 处理**之后**——收到 count==expected 的新快照，既证明此前每个 C2S
    （含静默拒绝）已处理完毕，又以该计数作「拒绝未扣料」的断言。give 同时推进实例载体。

    **时间锚定在 give 之前瞬间、不用调用方锚点**（review finding [major]）：调用方传入
    的 after_t 在「操作 + 1.0s sleep」**之前**截取。若前一 C2S 被错误实现额外 grant，
    其 inventory_snapshot（count 恰为期望值）在 sleep 期间就到——e.t > after_t 会让
    wait_for 扫历史事件时把它误认作 barrier 的 give 快照，barrier 提前返回、真 give 的
    更高计数快照未被观察 → 假绿（重复 break 侧的 _total_items 断言同款被绕过）。因此
    sleep 之后、cmd(give) **之前**取 give_anchor = last_event_time(bot)，排除 give 之前
    一切快照（含错误 grant），wait_for 只能命中 give 自己的快照。

    静默拒绝路径无回执、无「处理完成」信号：若 give 与前一 C2S intent
    （coffin_place / coffin_break）落入同一 tick 窗口，执行/快照可能与前一个请求的
    处理交错（调试构建实测过的窗口；release 20tps 下同样可能同批到达）。此处固定
    sleep 1.0s（≈20 tick）让 give 落在后续独立 tick，保证 give 命令**执行**且其快照
    可观测。该 sleep 是防御性措辞，不放松任何计数断言。
    """
    _park_non_coffin_items_in_body_pocket(bot)
    time.sleep(1.0)
    give_anchor = last_event_time(bot)
    bot.cmd(f"give {COFFIN_ID} 1")
    event = bot.wait_for(
        lambda e: e.kind == "server_data"
        and e.data["payload_type"] == "inventory_snapshot"
        and e.t > give_anchor
        and _coffin_count(e.data["payload"]) == expected_count,
        timeout=timeout,
        description=(
            description
            or f"give {COFFIN_ID} 后 count=={expected_count} 的水位线快照"
        )
        + _snapshot_debug(bot, give_anchor),
    )
    return event.data["payload"]


def _snapshot_debug(bot, after_t):
    from bot.server_data import decode_server_data_payload

    snaps = [
        e
        for e in bot.events
        if e.kind == "server_data" and e.data["payload_type"] == "inventory_snapshot"
    ]
    raw = [
        e
        for e in bot.events
        if e.kind == "payload"
        and e.data.get("channel") == "bong:server_data"
    ]
    raw_desc = []
    for e in raw[-10:]:
        try:
            dec = decode_server_data_payload(e.data["data"])
        except Exception as exc:  # noqa: BLE001
            raw_desc.append(f"t={e.t:.2f} {len(e.data['data'])}B EXC={exc!r}")
        else:
            kind = "NULL" if dec is None else dec.get("type")
            raw_desc.append(f"t={e.t:.2f} {len(e.data['data'])}B {kind}")
    chat_dev = [
        e for e in bot.events if e.kind == "chat" and "[dev]" in e.data.get("text", "")
    ]
    return (
        f"（诊断: after_t={after_t:.3f}, 最近 {len(snaps[-8:])} 条快照 "
        + "; ".join(
            f"t={e.t:.2f} rev={e.data['payload'].get('revision')} "
            f"cnt={_coffin_count(e.data['payload'])} [{_coffin_locations(e.data['payload'])}]"
            for e in snaps[-8:]
        )
        + f"；containers: {_container_debug(snaps[-1].data['payload']) if snaps else '∅'}"
        + f"；最近 {len(raw[-10:])} 条 raw bong:server_data: "
        + "; ".join(raw_desc)
        + f"；最近 {len(chat_dev[-4:])} 条 [dev] chat: "
        + "; ".join(f"t={e.t:.2f} {e.data['text']!r}" for e in chat_dev[-4:])
        + "）"
    )


def _give_coffin_instance(bot, after_t, timeout=_STEP_TIMEOUT) -> int:
    """give 一棺并返回锚定 after_t 之后新快照中的实例 id。

    不能用 wait_inventory_contains + latest_inventory_snapshot：wait_for 扫历史事件，
    第二次 give 会瞬间命中先前 give 的旧快照，latest 取到的仍是旧计数快照（无实例）。
    必须 e.t > after_t 锚定「本次 give 之后」的新快照。
    """
    _park_non_coffin_items_in_body_pocket(bot)
    bot.cmd(f"give {COFFIN_ID} 1")
    event = bot.wait_for(
        lambda e: e.kind == "server_data"
        and e.data["payload_type"] == "inventory_snapshot"
        and e.t > after_t
        and _coffin_count(e.data["payload"]) >= 1,
        timeout=timeout,
        description=f"give {COFFIN_ID} 之后含实例的新 inventory_snapshot",
    )
    return _latest_coffin_instance_id(event.data["payload"])


def _send_coffin_place(bot, pos, instance_id) -> None:
    bot.intent(
        {
            "type": "coffin_place",
            "v": 1,
            "x": pos[0],
            "y": pos[1],
            "z": pos[2],
            "item_instance_id": instance_id,
        }
    )


def _send_coffin_break(bot, pos) -> None:
    bot.intent(
        {
            "type": "coffin_break",
            "v": 1,
            "x": pos[0],
            "y": pos[1],
            "z": pos[2],
        }
    )


def _send_coffin_enter(bot, pos) -> None:
    bot.intent(
        {
            "type": "coffin_enter",
            "v": 1,
            "x": pos[0],
            "y": pos[1],
            "z": pos[2],
        }
    )


def _anchor_coffin_probe_position(bot, candidate) -> None:
    """把玩家锚到候选棺材上方，隔离出生点下坠对距离判定的干扰。"""
    bot.set_position(
        candidate[0] + 0.5,
        candidate[1] + 2.0,
        candidate[2] + 0.5,
        on_ground=False,
    )


def _place_on_first_air_layer(bot, px, py, pz, instance_id):
    """通过真实放置消费结果找出当前列首个可放置空气层。

    Bot 协议观察面没有方块查询；逐层提交同一个实例，只有服务端确认消费后才接受
    该层。非空气层会静默拒绝并保留实例，下一层继续探测。这样既不引入测试专用
    server API，也把实际成功坐标交给后续所有契约断言。
    """
    last_attempt = None
    for offset in AIR_LAYER_SEARCH_OFFSETS:
        candidate = (px + PLACE_OFFSET, py + offset, pz)
        _anchor_coffin_probe_position(bot, candidate)
        attempt_anchor = last_event_time(bot)
        _send_coffin_place(bot, candidate, instance_id)
        try:
            consumed = _wait_coffin_count(
                bot,
                0,
                attempt_anchor,
                timeout=_AIR_LAYER_PROBE_TIMEOUT,
                description=f"动态空气层探测 {candidate} 成功消费棺材实例",
            )
        except BotAssertionError:
            last_attempt = candidate
            continue
        return candidate, attempt_anchor, consumed
    raise BotAssertionError(
        f"动态空气层探测失败：从 y={py} 向上尝试 {list(AIR_LAYER_SEARCH_OFFSETS)}，"
        f"最后目标={last_attempt}；服务端未确认任何目标为空气并消费实例"
    )


def _find_non_air_layer(bot, px, first_air_y, pz, instance_id):
    """从首个空气层向下找第一个真实静默拒绝层，并清理途中误放置。

    玩家 Position 的整数 y 不一定等于地表 support y：出生点可能悬在多层空气中，
    因而 ``py - 1`` 不能证明目标非空气。每个向下候选都只走 coffin_place 协议；若
    真实 coffin marker spawn 则说明它确实放置成功，立即用 coffin_break 清场并给下一层
    一个新实例；首个未 spawn marker 的候选在已排除距离、维度和 registry 碰撞后只能是
    非空气拒绝。这里不能用库存 count 变化作为成功判据：inventory_snapshot 与实体事件
    走不同的异步路径，快照可能晚到而把已消费实例误判成拒绝。
    """
    for depth in _NON_AIR_PROBE_DEPTHS:
        candidate = (px, first_air_y - depth, pz)
        _anchor_coffin_probe_position(bot, candidate)
        attempt_anchor = last_event_time(bot)
        _send_coffin_place(bot, candidate, instance_id)
        try:
            marker = _wait_coffin_marker_spawn(
                bot,
                candidate,
                attempt_anchor,
                timeout=_MARKER_PROBE_TIMEOUT,
                description=f"向下探测 {candidate} 成功后的临时 marker",
            )
        except BotAssertionError:
            return candidate, instance_id

        teardown_anchor = last_event_time(bot)
        _send_coffin_break(bot, candidate)
        _wait_coffin_marker_destroy(bot, marker, teardown_anchor)
        _clear_probe_reclaim_materials(bot)

        give_anchor = last_event_time(bot)
        instance_id = _give_coffin_instance(bot, give_anchor)

    raise BotAssertionError(
        f"向下探测深度 {list(_NON_AIR_PROBE_DEPTHS)} 仍未找到非空气拒绝点："
        f"first_air_y={first_air_y}"
    )


def _distance_boundary_pair(bot, air_y, exclude=()):
    """在 bot 附近空气层把 6.0m 放置距离边界夹得最紧的 (pass, fail) 目标对。

    server 契约（coffin/mod.rs coffin_target_is_close）：bot 位置到 target 块中心
    （x+0.5, y+0.5, z+0.5）的平方距离 <= 36.0 视为可放置，> 36.0 拒绝。本函数在
    bot 周围 ±DISTANCE_SEARCH_RANGE 的空气块里扫（exclude 里的块跳过——已注册/被占
    的块不参与，否则 pass/fail 可能落在 not_empty 拒处造成假红）：
    - pass = d2 <= _DISTANCE_PASS_SQ 中**最大**者——恰在边界内侧，正确实现必须接受；
      距离半径缩到 <5.92m 的错误实现会在此误拒而红（review finding [3]：旧场景只测
      固定 offset=10 的过远拒绝，比较方向/半径大小无配对保护）；
    - fail = d2 > 36.0 中**最小**者——恰过边界，正确实现必须拒绝；过松 bound
      （如 <= 49）会在此接受而红。
    pass/fail 把比较方向钉在网格可构造的最窄区间 [d2_pass, d2_fail]。
    exact-36.0 无法用整块目标构造（bot 脚面 y vs 目标中心 y 恒差 ≈1.2-1.5m，dy² 总
    是拉偏），pass/fail 是网格能给出的最紧近似；pass 侧额外留 1.0 平方的
    client/server 位置残差余量防假红。

    返回 ((pass_x, air_y, pass_z), (fail_x, air_y, fail_z))。pass 距 bot ≤ 6.0（当前
    位置即可放），fail 距 bot > 6.0（原位置必拒，靠近后合法放置必成——被拒坐标的
    state-applying 复查用）。"""
    bx, by, bz = bot.position
    ix, iz = int(bx), int(bz)
    pass_t = None
    pass_d2 = -1.0
    fail_t = None
    fail_d2 = float("inf")
    for kx in range(-DISTANCE_SEARCH_RANGE, DISTANCE_SEARCH_RANGE + 1):
        for kz in range(-DISTANCE_SEARCH_RANGE, DISTANCE_SEARCH_RANGE + 1):
            tx, tz = ix + kx, iz + kz
            cell = (tx, air_y, tz)
            if cell in exclude:
                continue
            dx = bx - (tx + 0.5)
            dy = by - (air_y + 0.5)
            dz = bz - (tz + 0.5)
            d2 = dx * dx + dy * dy + dz * dz
            if d2 <= _DISTANCE_PASS_SQ:
                if d2 > pass_d2:
                    pass_d2 = d2
                    pass_t = cell
            elif d2 > COFFIN_INTERACT_MAX_DISTANCE_SQ and d2 < fail_d2:
                fail_d2 = d2
                fail_t = cell
    assert pass_t is not None and fail_t is not None, (
        f"距离边界配对失败：pass={pass_t} d2={pass_d2:.3f} fail={fail_t} d2={fail_d2:.3f} "
        f"(bot={bx:.2f},{by:.2f},{bz:.2f}, air_y={air_y})"
    )
    return pass_t, fail_t


def _exact_boundary_target(bot, air_y, exclude=()):
    """找一个可放置的空气 target T：bot set_position 到 (T.x+6.5, T.y+0.5, T.z+0.5) 后
    server 侧 d2 恰为 36.0 —— `<= 36.0`（含边界）与 `< 36.0`（排他）的唯一区分点。

    `_distance_boundary_pair` 只产出 d2≤35.0 与 d2>36.0 的样本：(35.0, 36.0] 区间整块
    target 无法命中（bot 脚面 y 与目标块中心 y 恒非整差，dy² 总把 d2 拉偏）。但 player
    坐标连续——把 bot 精确 set_position 到 target 正 x 侧 6.5 格、y/z 对齐 target 中心
    （整数减半整数，全部 IEEE 精确），server 计算 dx=6.0/dy=0.0/dz=0.0 → d2==36.0 精确，
    `<=` 接受、`<` 拒绝，是排他边界唯一可观测的区分点。server 对 client 位置无移动
    校验（combat anticheat 只管 reach/cooldown，非位移），set_position 的位置即权威值。

    返回 T：T 与 T+1x（棺横跨两格）均不得在 exclude（首放棺占两格、其双键已注册，
    probe 不能撞键）。在 ±DISTANCE_SEARCH_RANGE 里取距 bot 最近的可放置格。
    """
    bx, by, bz = bot.position
    ix, iz = int(bx), int(bz)
    best = None
    best_d2 = None
    for kx in range(-DISTANCE_SEARCH_RANGE, DISTANCE_SEARCH_RANGE + 1):
        for kz in range(-DISTANCE_SEARCH_RANGE, DISTANCE_SEARCH_RANGE + 1):
            cell = (ix + kx, air_y, iz + kz)
            if cell in exclude:
                continue
            if (cell[0] + 1, cell[1], cell[2]) in exclude:
                continue
            d2 = kx * kx + kz * kz
            if best_d2 is None or d2 < best_d2:
                best_d2 = d2
                best = cell
    assert best is not None, (
        f"exact-36.0 边界探针找不到可放置 target：bot={bx:.2f},{by:.2f},{bz:.2f} "
        f"air_y={air_y} exclude={sorted(exclude)}"
    )
    return best


# server 侧 mundane coffin marker 实体的 entity kind（entity_model.rs COFFIN_MUNDANE_ENTITY_KIND）
# 与放置位置映射（coffin/mod.rs coffin_marker_position）：marker = (lower.x+1, lower.y, lower.z+0.5)。
COFFIN_MARKER_ENTITY_KIND = 160


def _coffin_marker_pos(place_pos) -> tuple[float, float, float]:
    return (place_pos[0] + 1.0, float(place_pos[1]), place_pos[2] + 0.5)


def _near(pos, x, y, z, tol: float = 0.01) -> bool:
    return (
        abs(pos[0] - x) < tol and abs(pos[1] - y) < tol and abs(pos[2] - z) < tol
    )


def _wait_coffin_marker_spawn(
    bot, place_pos, after_t, timeout=_STEP_TIMEOUT, description=None
) -> int:
    """等 mundane coffin 的 marker 渲染实体 spawn（kind==160 @ coffin_marker_position）。

    证明「世界确实获得了棺实体」：若实现只消费实例却不 spawn 世界实体（或类型/位置不符），
    此处超时红。返回 entity_id 供破坏清场断言复用。
    """
    marker_pos = _coffin_marker_pos(place_pos)

    def matches(event) -> bool:
        return (
            event.kind == "entity_spawn"
            and event.t > after_t
            and event.data["type"] == COFFIN_MARKER_ENTITY_KIND
            and _near(
                (event.data["x"], event.data["y"], event.data["z"]),
                marker_pos[0],
                marker_pos[1],
                marker_pos[2],
            )
        )

    event = bot.wait_for(
        matches,
        timeout,
        description
        or f"mundane_coffin marker 实体 spawn（kind={COFFIN_MARKER_ENTITY_KIND}）于 {marker_pos}",
    )
    return event.data["entity_id"]


def _wait_coffin_marker_destroy(bot, entity_id, after_t, timeout=_STEP_TIMEOUT) -> None:
    """等 coffin_break 后 marker 实体被 despawn（entities_destroy 含 entity_id）。

    证明 destroy path 清掉了世界实体：若实现只移 registry 条目却留下 stale 的
    client-visible 实体（review finding 的 concrete broken implementation），
    此处超时红。coffin 用左键破坏，marker 无方块无 block update，entity 层移除
    （valence Despawned → S2C_ENTITIES_DESTROY）是唯一的清场信号。
    """
    bot.wait_for(
        lambda e: e.kind == "entities_destroy"
        and e.t > after_t
        and entity_id in e.data["entity_ids"],
        timeout=timeout,
        description=f"coffin_break 后 marker 实体 #{entity_id} 应被 despawn（entities_destroy）",
    )


def _assert_no_coffin_marker_spawn(
    bot,
    after_t,
    description,
    window=_REJECT_OBS_WINDOW,
) -> None:
    """拒绝路径不变量：有界窗口内不得出现**任何**新建的 kind=160 marker spawn。

    review finding [major]：旧实现先按放置目标坐标过滤（_near(...) 只收集目标附近
    的 spawn），把「没有在目标附近 spawn」误当成「拒绝即不得 spawn 世界 marker」——
    一个拒绝请求但把 marker 错放到偏移/默认坐标的错误实现能全绿。位置匹配只适用于
    **正向**放置断言；拒绝断言必须拒绝窗口内任何新建 kind=160 实体。本场景每个拒绝
    段的 anchor 都取在首放 marker spawn **之后**（且观察期内 bot 不移动、不跨维，
    无重流来源），因此 e.t > after_t 天然排除既有的首放 marker，窗口内出现的任何
    kind=160 spawn 都只能是拒绝路径错误留下的孤儿实体，无需位置过滤。

    review finding [major]（观察窗）：同 _assert_no_coffin_marker_destroy——旧实现持锁
    扫完**当前**事件前缀就返回——reader 线程后台 append，inventory_snapshot 与实体
    spawn 走不同时序路径，拒绝实现先发快照、后排队 marker spawn 时晚到的 spawn
    从未被检查。因此必须等满一个有界窗口再下结论：窗口内逐帧重扫，出现匹配即红，
    窗口结束仍无才通过。扫描持 bot._lock（last_event_time 同款约定）。

    review finding [major]（round 6，观察窗锚点）：deadline 必须锚在**观察开始**——
    旧代码 deadline = after_t + window（after_t 是 intent 前的事件时间），而调用方在
    intent 之后先跑 _give_barrier（无条件 sleep 1s + 独立等 give 快照）再进本 helper；
    若 request 处理或 barrier 耗尽 2s 窗口，helper 扫一遍就因
    time.monotonic()-bot.t0 >= deadline 立即退出——错误实现先发期望快照、deadline 之后
    才排队 marker spawn 时晚到事件漏检。改在 helper 入口取
    time.monotonic() - bot.t0 + window：无论 barrier 耗时多长，观察期都是完整的
    window 秒（事件过滤仍用 e.t > after_t，intent 后的一切 spawn 都纳入）。
    """
    deadline = time.monotonic() - bot.t0 + window
    spawned = []
    while True:
        with bot._lock:
            spawned = [
                e
                for e in bot.events
                if e.kind == "entity_spawn"
                and e.t > after_t
                and e.data["type"] == COFFIN_MARKER_ENTITY_KIND
            ]
        if spawned:
            break
        if time.monotonic() - bot.t0 >= deadline:
            break
        time.sleep(0.1)
    assert not spawned, (
        f"{description}：拒绝路径不得 spawn mundane_coffin marker 实体，"
        f"实际 {len(spawned)} 个 @ "
        + "; ".join(
            f"#{s.data['entity_id']} ({s.data['x']:.1f},{s.data['y']:.1f},{s.data['z']:.1f})"
            f" t={s.t:.2f}"
            for s in spawned
        )
    )


def _assert_no_orphan_coffin_marker_spawn(
    bot,
    after_t,
    allowed_entity_ids,
    description,
    window=_REJECT_OBS_WINDOW,
) -> None:
    """返回主世界后的孤儿 marker 不变量：窗口内新建 kind=160 spawn 只能是原 marker 重流。

    异维拒绝段的 world 侧在 tsy 内观察——跨维时主世界实体不可见（server 按维度过滤
    实体包）。返回主世界后，既有的原 marker 以**同 entity_id** 重流（实体在
    OverworldLayer 持续存在，dimension switch 只是 client 重新收 spawn 包），这是唯一
    合法的返回后 spawn。错误实现若绕过/独立于维度守卫、在主世界实体层创建 marker，其
    entity_id 是新分配的（!= 原 marker）——tsy 侧观察窗看不见它（请求在 tsy 处理时
    spawn 就被按维度过滤），返回主世界才流给 client。旧实现返回后直接进 break 断言
    （只等原 marker destroy），孤儿以不同 entity_id 存活至场景结束（review finding
    [major] round 8）。此处以 **entity_id 区分**而非位置（孤儿可能在任意坐标，位置
    过滤会漏）：窗口内出现 entity_id 不在 allowed 里的 kind=160 spawn 即红。

    观察窗约定同 _assert_no_coffin_marker_spawn：deadline 锚在 helper 入口，事件过滤
    用 e.t > after_t（after_t 取 tpdim overworld **之前**的 ret_anchor，返回全程的重流
    与孤儿——含晚到的排队 spawn——都落入窗口）。
    """
    deadline = time.monotonic() - bot.t0 + window
    orphans = []
    while True:
        with bot._lock:
            orphans = [
                e
                for e in bot.events
                if e.kind == "entity_spawn"
                and e.t > after_t
                and e.data["type"] == COFFIN_MARKER_ENTITY_KIND
                and e.data["entity_id"] not in allowed_entity_ids
            ]
        if orphans:
            break
        if time.monotonic() - bot.t0 >= deadline:
            break
        time.sleep(0.1)
    assert not orphans, (
        f"{description}：返回主世界后窗口内出现 {len(orphans)} 个孤儿 coffin marker spawn"
        f"（只允许原 marker 重流 #{sorted(allowed_entity_ids)}）: "
        + "; ".join(
            f"#{s.data['entity_id']} ({s.data['x']:.1f},{s.data['y']:.1f},{s.data['z']:.1f})"
            f" t={s.t:.2f}"
            for s in orphans[:3]
        )
    )


def _assert_no_live_coffin_marker_after_teardown(bot, description) -> None:
    """场景收尾不变量：事件流重建的 live kind=160 实体集必须为空。

    全部 teardown break（含最终重放的清场）之后，场景声称零世界 marker 残留。从事件流
    重建 live 集（entity_spawn 增、entities_destroy 删）——任何 spawn 未配对 destroy 的
    marker（异维孤儿、重复放置孤儿、teardown 漏破的 marker）都留下 live 条目而红
    （review finding [major] round 8 收口：返回后只扫新建 spawn 不足以证明「破坏后无
    额外 marker 存活」，须验证整个生命周期结束时实体层零残留）。
    """
    with bot._lock:
        live = set()
        for e in bot.events:
            if e.kind == "entity_spawn" and e.data["type"] == COFFIN_MARKER_ENTITY_KIND:
                live.add(e.data["entity_id"])
            elif e.kind == "entities_destroy":
                live.difference_update(e.data["entity_ids"])
    assert not live, (
        f"{description}：场景收尾后仍有 {len(live)} 个存活 coffin marker 实体"
        f" #{sorted(live)}（spawn 未配对 destroy 的孤儿/漏破 marker）"
    )


def _assert_no_coffin_marker_destroy(
    bot, entity_id, after_t, description, window=_REJECT_OBS_WINDOW
) -> None:
    """空→空重复破坏不变量：窗口内不得再次 despawn 指定 marker 实体。

    review finding [major]：单发声称的 empty→empty 转移从未被演练。首破坏的
    despawn 在 after_t 之前；第二次 coffin_break 若再发 entities_destroy 含同一
    entity_id，即实现把「已清场」状态又错误清了一遍（removes unrelated state）。

    review finding [major]（观察窗）：同 spawn 侧——entities_destroy 与
    inventory_snapshot 走不同时序路径，重复 despawn 可能晚于 give 水位线快照到达。
    先等满有界窗口再断言，晚到的重复 despawn 不漏检。

    review finding [major]（round 6，观察窗锚点）：deadline 锚在 helper 入口
    （time.monotonic() - bot.t0 + window）而非 after_t + window——调用方在 intent 之后
    先跑 _give_barrier（sleep 1s + 等快照）才进来，旧锚点会被 barrier 耗尽、扫一遍即退，
    晚到的重复 despawn 漏检（同 _assert_no_coffin_marker_spawn 同款缺陷）。
    """
    deadline = time.monotonic() - bot.t0 + window
    redestroyed = []
    while True:
        with bot._lock:
            redestroyed = [
                e
                for e in bot.events
                if e.kind == "entities_destroy"
                and e.t > after_t
                and entity_id in e.data["entity_ids"]
            ]
        if redestroyed:
            break
        if time.monotonic() - bot.t0 >= deadline:
            break
        time.sleep(0.1)
    assert not redestroyed, (
        f"{description}：窗口内发现 {len(redestroyed)} 次对实体 #{entity_id} 的重复 despawn"
    )


def _assert_no_enter_transition_after(
    bot, after_t, enter_pos, description, window=_REJECT_OBS_WINDOW
) -> None:
    """非空气拒绝坐标的残留探针不变量：enter 探针后窗口内不得出现任何进棺状态转变。

    非空气拒绝坐标 (px, py-1, pz) 是永久实心地面，无法用合法放置复查 registry 残留
    （放置必被 not empty 拒，与 already-registered 拒同为静默、无法区分）。改用
    coffin_enter 探针直接对该坐标施加状态：正确实现对被拒坐标无 registry 条目 →
    registry.lookup 返回 None 静默 continue（coffin/mod.rs:618-624），零状态变化；
    而「拒绝时先插 registry 双键再早退（不消费、不 spawn）」的错误实现会在该坐标留
    下残留条目 → enter 的 lookup 命中、set_occupied 成功 → 进棺状态转变
    （CoffinState{in_coffin:true} + bot 自身 invisible 位置位 + 瞬移到
    coffin_player_position）。三路信号任一出现即红——enter 对坐标无空气门
    （coffin/mod.rs:651 只查已进棺 + 距离），是唯一能对永久实心坐标的 registry 键
    施加状态并留下可观测信号的探针（review finding [major] round 7）。

    同 _assert_no_coffin_marker_spawn 的观察窗约定：deadline 锚在 helper 入口
    （time.monotonic() - bot.t0 + window），调用方在 enter intent 之后立即进入，
    窗口完整覆盖 server 对 enter 请求的处理；事件过滤用 e.t > after_t（intent 前的
    一切事件排除）。entered 位置用 _near 容差匹配 coffin_player_position 浮点。
    """
    enter_inside = (
        float(enter_pos[0]) + 0.5,
        float(enter_pos[1]) + 0.05,
        float(enter_pos[2]) + 0.5,
    )
    deadline = time.monotonic() - bot.t0 + window
    hits = []
    while True:
        with bot._lock:
            hits = []
            for e in bot.events:
                if e.t <= after_t:
                    continue
                if (
                    e.kind == "server_data"
                    and e.data["payload_type"] == "coffin_state"
                    and e.data["payload"].get("in_coffin") is True
                ):
                    hits.append(f"coffin_state(in_coffin:true)@{e.t:.2f}")
                elif (
                    e.kind == "entity_metadata"
                    and e.data["entity_id"] == bot.entity_id
                    and e.data["flags"] is not None
                    and e.data["flags"] & 0x20
                ):
                    hits.append(f"invisible 位置位@{e.t:.2f}")
                elif e.kind == "pos_look" and _near(
                    (e.data["x"], e.data["y"], e.data["z"]),
                    enter_inside[0],
                    enter_inside[1],
                    enter_inside[2],
                ):
                    hits.append(f"瞬移到棺内坐标 {enter_inside}@{e.t:.2f}")
        if hits:
            break
        if time.monotonic() - bot.t0 >= deadline:
            break
        time.sleep(0.1)
    assert not hits, (
        f"{description}：enter 残留探针后 {window:.1f}s 内不得出现进棺状态转变"
        f"（正确实现对实心被拒坐标 {enter_pos} 无 registry 条目 → 静默），"
        f"实际 {len(hits)} 条: "
        + "; ".join(hits[:3])
    )


def run(env) -> None:
    with env.new_bot("CoPl") as bot:
        wait_for_ready(bot)
        # The dev fallback world materializes its flat test surface only in the loaded
        # center chunk area, while spawn_distribution also contains out-of-area anchors.
        # Keep the scenario black-box but anchor this lifecycle test to that loaded
        # surface; the coffin target y is still discovered dynamically below.
        bot.set_position(0.5, 66.0, 0.5, on_ground=True)
        # `clearinv all` 保留有效 worn 背包，但出生剑仍占主手；先把出生剑移进
        # body_pocket，再清第二次携带面。直接 `clearinv naked` 会留下 orphan pack_*，
        # 之后所有 /give 都会被容器 owner 校验拒绝（对齐其它库存场景的两步清场）。
        initial = bot.wait_for(
            lambda e: e.kind == "server_data"
            and e.data["payload_type"] == "inventory_snapshot"
            and isinstance(
                e.data["payload"].get("equipped", {}).get("main_hand_held"), dict
            )
            and e.data["payload"]["equipped"]["main_hand_held"].get("item_id")
            == "iron_sword",
            timeout=_STEP_TIMEOUT,
            description="join 后取得出生剑实例，作为清场同步前的权威快照",
        ).data["payload"]
        starter_item = initial["equipped"]["main_hand_held"]
        bot.cmd("clearinv all")
        # clearinv handler 的成功回执是 chat，且不主动 emit snapshot；给 command 一个
        # tick 进入 Update 后再发 move，move 自己的 inventory_snapshot 才是可靠屏障。
        time.sleep(0.5)
        move_anchor = last_event_time(bot)
        send_move(
            bot,
            int(starter_item["instance_id"]),
            {"kind": "equip", "slot": "main_hand", "state": "held"},
            {"kind": "container", "container_id": "body_pocket", "row": 0, "col": 0},
        )
        bot.wait_for(
            lambda e: e.kind == "server_data"
            and e.data["payload_type"] == "inventory_snapshot"
            and e.t > move_anchor
            and e.data["payload"].get("equipped", {}).get("main_hand_held") is None,
            timeout=_STEP_TIMEOUT,
            description="出生剑移入 body_pocket 后主手为空",
        )
        clear_anchor = last_event_time(bot)
        bot.cmd("clearinv all")
        time.sleep(0.5)
        bot.wait_for(
            lambda e: e.kind == "server_data"
            and e.data["payload_type"] == "inventory_snapshot"
            and e.t > clear_anchor
            and e.data["payload"].get("placed_items") == [],
            timeout=_STEP_TIMEOUT,
            description="二次 clearinv all 后清空携带面并保留有效 worn 背包",
        )

        # 默认 spawn_distribution 允许落在 raster 覆盖外的动态地形，出生点高度也不
        # 固定。保留 server 发来的权威位置，再由真实放置请求向上确认首个空气层；这
        # 仍只走 C2S coffin_place 协议，不读 server 方块状态或测试专用 API。
        assert bot.position is not None, "需要 pos_look 后的 bot.position 来定棺位"
        px, py, pz = (int(v) for v in bot.position)

        # ── 正向：放置消费恰好一个实例 ──────────────────────────────────
        give_anchor = last_event_time(bot)
        first_instance = _give_coffin_instance(bot, give_anchor)
        place_pos, anchor, consumed = _place_on_first_air_layer(
            bot, px, py, pz, first_instance
        )
        assert _coffin_count(consumed) == 0, "首放必须恰好消费 1 个实例"
        # 世界侧：等 mundane coffin 的 marker 渲染实体 spawn——证明世界确实获得了棺实体，
        # 而不只是背包实例被消费。
        marker_entity_id = _wait_coffin_marker_spawn(
            bot,
            place_pos,
            anchor,
            description="首放成功后应出现 mundane_coffin marker 实体（kind=160 @ 放置位）",
        )

        # ── 负向：重复放置 → already registered，第二实例保留 ───────────
        give_anchor = last_event_time(bot)
        second_instance = _give_coffin_instance(bot, give_anchor)
        anchor = last_event_time(bot)
        _send_coffin_place(bot, place_pos, second_instance)
        _park_coffin_in_hotbar(bot, second_instance, 0)
        duplicate_snapshot = _give_barrier(
            bot,
            2,
            description=(
                "双放被拒后 +1 give 应恰好 count==2（第二实例 + 新增 1）——"
                "若 count==1 说明双放偷扣了第二实例（double-grant）"
            ),
        )
        # 世界侧（review finding [major]）：重复放置被拒不得 spawn 第二个 marker。
        # 首放的 marker 在 anchor 之前已 spawn（被时间锚定排除），窗口内再出现
        # **任何** kind=160 spawn（不按位置过滤）即重复路径错误地留下世界孤儿实体。
        _assert_no_coffin_marker_spawn(
            bot,
            anchor,
            description="重复放置拒绝路径不得 spawn 第二个 coffin marker",
        )
        _park_coffin_in_hotbar(bot, _latest_coffin_instance_id(duplicate_snapshot), 1)

        # ── 负向：registry 双键 upper-key 碰撞（review finding [major]）──
        # 已注册棺占 (lower.x, y, z)..(lower.x+1, y, z) 两格，registry 同时以 lower 与
        # upper(=lower+1x) 为键登记、insert 同时查两键（coffin/mod.rs:133-134）。
        # 上面精确重复只把新 lower 打在既有 lower 键上；若实现只查新 lower 对已登记
        # **lower** 键的碰撞、漏查已登记 upper 键，把新放置打在 first 棺的 upper 坐标
        # （upper_pos = place_pos + 1x）会被**接受**并吞掉一个实例（且 spawn 错误 marker）。
        # 正确实现必须以 already-registered 拒绝、实例保留、不得 spawn。
        upper_pos = (place_pos[0] + 1, place_pos[1], place_pos[2])
        upper_give_anchor = last_event_time(bot)
        upper_instance = _give_coffin_instance(bot, upper_give_anchor)
        upper_anchor = last_event_time(bot)
        _send_coffin_place(bot, upper_pos, upper_instance)
        _park_coffin_in_hotbar(bot, upper_instance, 2)
        upper_snapshot = _give_barrier(
            bot,
            4,
            description=(
                "upper-key 重叠放置被拒后 +1 give 应恰好 count==4（三个拒绝实例 + 新增 1）——"
                "若 count==3 说明实现吞掉了 upper-key 碰撞路径的实例"
            ),
        )
        _assert_no_coffin_marker_spawn(
            bot,
            upper_anchor,
            description="upper-key 重叠拒绝路径不得 spawn coffin marker",
        )
        _park_coffin_in_hotbar(bot, _latest_coffin_instance_id(upper_snapshot), 3)
        # review finding [major] round 8：upper-key 拒绝段只查「库存保留 + 无 spawn」——
        # 一个先写新棺 secondary 键（upper_pos+1x）再在 primary 键 upper_pos 处发现碰撞、
        # 不消费不 spawn 就返回的错误插入路径全过。该泄漏键 (px,py+1,pz) 永不被后续
        # break/re-place 覆盖（只碰原 place_pos 双键），残留使碰撞其格的无关放置被误拒。
        # 对**新尝试棺的另一个 registry 坐标**（upper_pos+(1,0,0)，即 place_pos+2x）做
        # state-applying 复查。该坐标在动态地形上可能是实心块，不能再用 place 复查把
        # not-empty 混进结论；coffin_enter 直接命中 registry，正确实现静默 no-op，
        # 泄漏实现会产生进棺状态转变并红。
        uk_leak_pos = (place_pos[0] + 2, place_pos[1], place_pos[2])
        uk_leak_anchor = last_event_time(bot)
        _send_coffin_enter(bot, uk_leak_pos)
        _assert_no_enter_transition_after(
            bot,
            uk_leak_anchor,
            uk_leak_pos,
            description=(
                "upper-key 拒绝段后对 place_pos+2x 的 registry 残留探针：正确实现无条目"
                " → coffin_enter 静默；若拒绝时写入 secondary 键则进棺状态转变红"
            ),
        )

        # ── 负向：非空气目标（脚下方块）+ 过远目标，各静默拒绝 ──────────
        probe_instance = _first_coffin_instance_id(latest_inventory_snapshot(bot))
        # 非空气 probe 从首个成功空气层向下逐层走真实协议，找到首个静默拒绝层；
        # 过远 probe 用 fail_t（空气层、恰过 6.0m 边界），保证唯一拒绝理由是距离
        # 而非 not empty。pass_t/fail_t 把距离比较方向夹在网格可构造的最窄带
        # （review finding [3]，round 6：旧 offset=10 只测「过远必拒」，比较方向
        # 无「恰内侧 + 恰外侧」配对保护——`< 36.0` 或过松 bound 都能全绿）。
        air_y = place_pos[1]
        # 首放棺占 (place_pos)..(place_pos+1x) 两格，从边界扫描中排除——pass_t 若落
        # 在被占格会以 not_empty 拒，造成与距离无关的假红。
        coffin_cells = {
            (place_pos[0], place_pos[1], place_pos[2]),
            (place_pos[0] + 1, place_pos[1], place_pos[2]),
        }
        boundary_origin = bot.position
        assert boundary_origin is not None, "需要边界探针计算前的权威玩家站位"
        pass_t, fail_t = _distance_boundary_pair(bot, air_y, exclude=coffin_cells)
        non_air_pos, probe_instance = _find_non_air_layer(
            bot, px, air_y, pz, probe_instance
        )
        # _find_non_air_layer temporarily anchors the player at each lower probe layer.
        # Restore the station used to compute pass_t/fail_t; otherwise the subsequent
        # boundary placement is evaluated from the probe's lower y and pass_t becomes
        # an unrelated too-far target.
        bot.set_position(*boundary_origin)
        time.sleep(1.0)
        # The rejected probe remains in the runtime pack.  A mundane coffin occupies
        # the pack's full 2x3 footprint, so park it before the next /give barrier can
        # allocate another instance; hotbar[4] is reserved after slots 0..3 above.
        _park_coffin_in_hotbar(bot, probe_instance, 4)
        # Exponential search legitimately spawned and tore down temporary markers;
        # start the rejection-only observation window after that cleanup.
        anchor = last_event_time(bot)
        _send_coffin_place(bot, non_air_pos, probe_instance)
        _send_coffin_place(bot, fail_t, probe_instance)
        non_air_snapshot = _give_barrier(
            bot,
            5,
            description="非空气+过远双拒后 +1 give 应恰好 count==5，实例分文未扣",
        )
        # 世界侧（review finding [major]）：非空气 / 过远拒绝不得 spawn marker——
        # 窗口内任何新建 kind=160 实体都不应出现。
        _assert_no_coffin_marker_spawn(
            bot,
            anchor,
            description="非空气/过远拒绝路径不得 spawn coffin marker",
        )
        _park_coffin_in_hotbar(bot, _latest_coffin_instance_id(non_air_snapshot), 5)

        # review finding [2]（round 6）：registry 残留复查此前只覆盖 stale 拒绝段——
        # 非空气/过远拒绝段的「无 spawn + 计数不变」会被「先插 registry 双键再早退
        # （不消费、不 spawn）」的错误实现全过。对**每个被拒坐标**做 state-applying
        # 复查 + teardown break（review finding [4] 零残留）：
        #   (a) 非空气坐标 (px,py-1,pz) 是永久实心地面、无法放置——coffin_enter
        #       探针直接对该坐标施加状态：正确实现无 registry 条目 → 静默 no-op；
        #       错误登记的残留被 lookup 命中 → 进棺状态转变红（review finding
        #       [major] round 7：旧替代坐标 (px,py+1,pz+2) y/z 双偏、不碰被拒键，
        #       残留永不暴露）；
        #   (b) 边界 pass_t（d2≤35.0 的最大值，含 client/server 位置余量）合法放置
        #       必须成功——距离半径缩到 <5.92m 在此红（review finding [3] 的 pass 侧）；
        #   (c) 过远 fail_t（d2>36.0 最小值）靠近后合法放置必须成功——最直接的残留
        #       探针：若过远拒绝时错误登记了 fail_t 双键，此处 already-registered 拒
        #       绝、marker 不 spawn → 超时红。
        # 每次复查先给一棺：_give_barrier(bot, 6) 同时断言上一复查放置恰好消费 1 个
        # 实例（消费 0 或 2 个都让 barrier 计数对不上而红）。(a) 的 enter 探针不消费
        # 实例，其「未偷扣」由 (b) 的 count==6 兜底断言。

        # (a) 非空气坐标 enter 残留探针（对被拒坐标本身施加状态）
        na_anchor = last_event_time(bot)
        _send_coffin_enter(bot, non_air_pos)
        _assert_no_enter_transition_after(
            bot,
            na_anchor,
            non_air_pos,
            description="非空气拒绝坐标的 enter 残留探针：正确实现应无进棺状态转变",
        )

        # (b) 边界 pass_t 复查 + teardown
        bp_give = _give_barrier(
            bot,
            6,
            description="非空气 enter 探针（不消费）后再 give 应恰好 count==6（5→6）",
        )
        bp_instance = _latest_coffin_instance_id(bp_give)
        bp_anchor = last_event_time(bot)
        _send_coffin_place(bot, pass_t, bp_instance)
        bp_marker = _wait_coffin_marker_spawn(
            bot,
            pass_t,
            bp_anchor,
            description=(
                "边界 pass_t（d2≤35.0 最大值）合法放置必须成功并 spawn marker——"
                "距离半径缩到 <5.92m 的错误实现在此红"
            ),
        )
        bp_break_anchor = last_event_time(bot)
        _send_coffin_break(bot, pass_t)
        _wait_coffin_marker_destroy(bot, bp_marker, bp_break_anchor)
        _clear_probe_reclaim_materials(bot)

        # (c) 过远 fail_t 复查 + teardown（需靠近后放置才有效，故 bot 先走到
        # fail_t 正下方地面块、放完 break 再走回原位——stale 段按原站位几何判定）
        tf_give = _give_barrier(
            bot,
            6,
            description="边界 pass_t 复查恰好消费 1 个实例后再 give 应恰好 count==6",
        )
        tf_instance = _latest_coffin_instance_id(tf_give)
        tf_origin = bot.position
        assert tf_origin is not None, "需要 bot.position 定 fail_t 复查站位"
        bot.move_to(fail_t[0], tf_origin[1], fail_t[2], speed=5.5)
        time.sleep(1.0)
        tf_anchor = last_event_time(bot)
        _send_coffin_place(bot, fail_t, tf_instance)
        tf_marker = _wait_coffin_marker_spawn(
            bot,
            fail_t,
            tf_anchor,
            description=(
                "过远 fail_t（d2>36.0 最小值）靠近后合法放置必须成功并 spawn marker"
                "——若过远拒绝时错误登记了 fail_t 双键，此处 already-registered 超时红"
            ),
        )
        tf_break_anchor = last_event_time(bot)
        _send_coffin_break(bot, fail_t)
        _wait_coffin_marker_destroy(bot, tf_marker, tf_break_anchor)
        _clear_probe_reclaim_materials(bot)
        bot.move_to(tf_origin[0], tf_origin[1], tf_origin[2], speed=5.5)
        time.sleep(1.0)

        # ── 正向：d2==36.0 精确边界探针（review finding [major] round 8）──
        # pass/fail 配对把比较方向夹在 [d2≤35.0, d2>36.0]，(35.0, 36.0] 区间无样本——
        # `< 36.0` 排他实现能全绿。exact d2==36.0 无法用整块 target 构造，但 player
        # 坐标连续：set_position 把 bot 精确放到 target 正 x 侧 6.5 格、y/z 对齐 target
        # 中心（半整数差，IEEE 精确），server 算得 dx=6.0/dy=0/dz=0 → d2==36.0。正确
        # 实现（<= 36.0 含边界）必须接受并 spawn marker；排他实现在此误拒 → 无 spawn →
        # 超时红。放置后立即 break teardown，bot 复位回 fail_t 复查原点（stale 段按原
        # 站位几何判定）。
        exact_target = _exact_boundary_target(bot, air_y, exclude=coffin_cells)
        exact_bot_pos = (
            exact_target[0] + 6.5,
            exact_target[1] + 0.5,
            exact_target[2] + 0.5,
        )
        exact_give = _give_barrier(
            bot,
            6,
            description="fail_t 复查段（net 5）后再 give 应恰好 count==6，exact-36 探针前置",
        )
        exact_instance = _latest_coffin_instance_id(exact_give)
        exact_anchor = last_event_time(bot)
        bot.set_position(*exact_bot_pos)
        time.sleep(1.0)
        _send_coffin_place(bot, exact_target, exact_instance)
        exact_marker = _wait_coffin_marker_spawn(
            bot,
            exact_target,
            exact_anchor,
            description=(
                "d2==36.0 精确边界放置必须成功并 spawn marker——`< 36.0` 排他实现在此"
                "误拒（`<=` 与 `<` 唯一区分点）而红"
            ),
        )
        exact_break_anchor = last_event_time(bot)
        _send_coffin_break(bot, exact_target)
        _wait_coffin_marker_destroy(bot, exact_marker, exact_break_anchor)
        _clear_probe_reclaim_materials(bot)
        bot.set_position(*tf_origin)
        time.sleep(1.0)

        # ── 负向（review finding [major]）：无效（已消费 stale）item_instance_id ──
        # first_instance 已被首放消费（consume_item_instance_once 移除实例行，
        # inventory/mod.rs:4171），inventory_item_by_instance 找不到 → 拒绝。位置选
        # 空气层、距玩家 ≈1.4 格（≤6.0 近距）且未注册处，唯一拒绝理由是实例缺失而非
        # not empty / 距离 / already-registered。若实现无视 item_instance_id 而吞掉
        # 任一可用棺材，count 会少 1 → 红。
        stale_pos = (px, air_y, pz + 1)
        stale_anchor = last_event_time(bot)
        _send_coffin_place(bot, stale_pos, first_instance)
        stale_snapshot = _give_barrier(
            bot,
            6,
            description=(
                "stale（已消费）instance id 拒绝后 +1 give 应恰好 count==6——"
                "实现不得无视 item_instance_id 吞掉可用棺材"
            ),
        )
        _assert_no_coffin_marker_spawn(
            bot,
            stale_anchor,
            description="stale instance id 拒绝路径不得 spawn coffin marker",
        )
        _park_coffin_in_hotbar(bot, _latest_coffin_instance_id(stale_snapshot), 6)

        # review finding [2]（round 5）：拒绝路径只断言「无 spawn + 计数不变」——一个
        # 在 stale_pos 拒绝时先插入 registry 双键（lower/upper）再早退（不消费实例、
        # 不 spawn marker）的错误实现两个负向断言全过；场景此后从未在 stale_pos 放置
        # /破坏，最终 destroy/re-place 只复查原 place_pos，invisible registry 残留
        # 永不暴露。拒绝段后必须对**被拒坐标**做 state-applying 复查：用合法实例在
        # stale_pos 成功放置——若拒绝时错误登记了双键，此放置会以 already-registered
        # 拒绝、marker 不 spawn、实例不消费 → _wait_coffin_marker_spawn 超时红。
        stale_recheck_give = last_event_time(bot)
        stale_recheck_instance = _give_coffin_instance(bot, stale_recheck_give)
        stale_recheck_anchor = last_event_time(bot)
        _send_coffin_place(bot, stale_pos, stale_recheck_instance)
        stale_recheck_marker = _wait_coffin_marker_spawn(
            bot,
            stale_pos,
            stale_recheck_anchor,
            description=(
                "stale 拒绝段后在被拒坐标 stale_pos 成功放置并 spawn marker"
                "（证明拒绝未在 registry 留残留，否则此处 already-registered 超时红）"
            ),
        )
        # review finding [4]（round 6）：stale 复查的棺没有 teardown——marker 与
        # registry 条目永久残留，同一长活 E2E server 重跑时 stale_pos 已注册 → 超时
        # 红（且污染同 fixture 区的后续场景）。立即 break 清场。
        stale_teardown_anchor = last_event_time(bot)
        _send_coffin_break(bot, stale_pos)
        _wait_coffin_marker_destroy(bot, stale_recheck_marker, stale_teardown_anchor)
        _clear_probe_reclaim_materials(bot)

        # ── 负向：异维 → 逐字 chat 拒绝，实例保留 ───────────────────────
        tp_anchor = last_event_time(bot)
        bot.cmd("tpdim tsy")
        bot.wait_for(
            lambda e: e.kind == "chat"
            and e.t > tp_anchor
            and "Queued /tpdim tsy within current XYZ gate." in e.data["text"],
            timeout=_STEP_TIMEOUT,
            description="tpdim tsy 排队反馈",
        )
        bot.wait_for(
            lambda e: e.kind == "respawn" and e.t > tp_anchor,
            timeout=_STEP_TIMEOUT,
            description="tpdim tsy 跨维 Respawn",
        )
        bot.wait_for(
            lambda e: e.kind == "pos_look" and e.t > tp_anchor,
            timeout=_STEP_TIMEOUT,
            description="跨维后的坐标确认脉冲",
        )
        cross_anchor = last_event_time(bot)
        _send_coffin_place(
            bot, place_pos, _first_coffin_instance_id(latest_inventory_snapshot(bot))
        )
        bot.wait_for(
            lambda e: e.kind == "chat"
            and e.t > cross_anchor
            and e.data["text"] == DIMENSION_REJECT_TEXT,
            timeout=_STEP_TIMEOUT,
            description=f"异维放置必须逐字拒绝（整体相等）：{DIMENSION_REJECT_TEXT}",
        )
        # 世界侧（review finding [major]）：异维拒绝不得 spawn marker。主世界 marker
        # 早已 spawn（cross_anchor 前）；bot 此刻仍在 tsy，主世界实体不可见、不会重流，
        # 因此整窗 (cross_anchor, cross_anchor+window) 内出现的任何 kind=160 spawn 都
        # 必然是拒绝路径错误留下的孤儿实体——旧实现以 0.5s sleep 提前闭合观察窗
        # （until_t=ret_anchor），拒绝 0.6s 后才发出的错误 marker 会被滤掉而漏检；
        # 必须**先等满整个有界观察窗（仍在异维）再返回**，返回锚点取在观察结束之后。
        _assert_no_coffin_marker_spawn(
            bot,
            cross_anchor,
            description="异维拒绝路径不得 spawn coffin marker",
        )
        ret_anchor = last_event_time(bot)
        bot.cmd("tpdim overworld")
        bot.wait_for(
            lambda e: e.kind == "chat"
            and e.t > ret_anchor
            and "Queued /tpdim overworld within current XYZ gate." in e.data["text"],
            timeout=_STEP_TIMEOUT,
            description="tpdim overworld 排队反馈",
        )
        bot.wait_for(
            lambda e: e.kind == "respawn" and e.t > ret_anchor,
            timeout=_STEP_TIMEOUT,
            description="返回主世界 Respawn",
        )
        # review finding [major] round 8：异维拒绝的 world 侧在 tsy 内观察（跨维时主
        # 世界实体不可见、server 按维度过滤实体包），返回主世界后**必须再扫**：既有原
        # marker 以同 entity_id 重流（合法，allowed），任何新建且 entity_id !=
        # marker_entity_id 的 kind=160 spawn 都是异维拒绝路径绕过/独立于维度守卫、在
        # 主世界实体层错误创建的孤儿——tsy 侧观察窗看不见它（请求在 tsy 处理时 spawn
        # 就被维度过滤），返回才流给 client，旧实现返回后直接进 break 断言（只等原
        # marker destroy），孤儿以不同 entity_id 存活至场景结束。锚点取 ret_anchor
        # （tpdim overworld 之前）：返回全程的重流与孤儿（含晚到的排队 spawn）都落入
        # e.t > ret_anchor 窗口。
        _assert_no_orphan_coffin_marker_spawn(
            bot,
            ret_anchor,
            allowed_entity_ids={marker_entity_id},
            description="返回主世界后不得出现非原 marker 的孤儿 coffin marker spawn",
        )
        pre_break_snapshot = _give_barrier(
            bot,
            7,
            description="异维拒后 +1 give 应恰好 count==7，实例未被异维请求偷扣",
        )
        # review finding [1]（round 5）：首次成功 break 的库存副作用基线必须锚在
        # break **之前**。旧代码 total_baseline 取在 break 完成之后——错误实现
        # despawn marker 时 grant 非棺回收料，该料被并入基线，重复 break 的
        # total_baseline+1 断言照样过。此 count==7 快照（异维拒后权威状态、break
        # intent 之前）即 break 前全量基线，供 break 后 +1 give 快照与之比对。
        total_pre_break = _total_items(pre_break_snapshot)
        _park_coffin_in_hotbar(bot, _latest_coffin_instance_id(pre_break_snapshot), 7)

        # ── 破坏：CoffinBreak 单发清场，同一坐标可重放 ──────────────────
        break_anchor = last_event_time(bot)
        bot.intent(
            {
                "type": "coffin_break",
                "v": 1,
                "x": place_pos[0],
                "y": place_pos[1],
                "z": place_pos[2],
            }
        )
        # 世界侧：coffin_break 必须把首放的 marker 实体 despawn（entities_destroy 含其 id）。
        # 若实现只移 registry 条目却留下 stale client-visible 实体，此断言超时红——
        # 这是本场景对「destroy path 清掉世界实体」的直接观察，不是 registry 重用的代理。
        _wait_coffin_marker_destroy(bot, marker_entity_id, break_anchor)

        # review finding [major]：单发清场声称的 empty→empty 重复破坏从未被演练。
        # 第二次对同一坐标 coffin_break：server 端 remove_by_pos → None 后静默 continue
        # （coffin/mod.rs:811-813），不得 panic / 断连 / 再 despawn / 扣料 / 加料。
        # 先以 +1 give 快照（count==8）作全量库存基线——含首次破坏的回收料。
        pre_second = _give_barrier(
            bot,
            8,
            description="首次破坏后 +1 give 应恰好 count==8（破坏只清世界实体，不碰背包实例）",
        )
        # 首次 break 按生产契约会返还随机部分配方材料；锁定完整差分，避免把合法
        # reclaim 误判成副作用，也避免实现额外发放任意物品。+1 give 只能新增一口棺，
        # 非棺材新增只能来自 mundane 配方的两种材料及其配方上限。
        pre_break_items = _template_counts(pre_break_snapshot)
        pre_second_items = _template_counts(pre_second)
        first_break_delta = {
            item_id: count - pre_break_items.get(item_id, 0)
            for item_id, count in pre_second_items.items()
            if count != pre_break_items.get(item_id, 0)
        }
        assert first_break_delta.get(COFFIN_ID) == 1, (
            f"首次 coffin_break 后 +1 give 必须恰好新增 1 口棺材，"
            f"break 前后差分={first_break_delta}"
        )
        reclaim_delta = {
            item_id: count
            for item_id, count in first_break_delta.items()
            if item_id != COFFIN_ID
        }
        assert set(reclaim_delta) <= {"ling_mu_ban", "ling_mu_gun"}, (
            f"首次 coffin_break 的非棺新增只能是 mundane 配方材料，实际差分={reclaim_delta}"
        )
        assert 0 <= reclaim_delta.get("ling_mu_ban", 0) <= 6, (
            f"ling_mu_ban 回收数量必须落在 Break 配方上限 [0,6]，实际={reclaim_delta}"
        )
        assert 0 <= reclaim_delta.get("ling_mu_gun", 0) <= 2, (
            f"ling_mu_gun 回收数量必须落在 Break 配方上限 [0,2]，实际={reclaim_delta}"
        )
        assert _total_items(pre_second) >= total_pre_break + 1, (
            f"首次 coffin_break 后至少应包含 +1 give 的棺材，break 前全量 {total_pre_break}，"
            f"实际 {_total_items(pre_second)}"
        )
        total_baseline = _total_items(pre_second)
        _park_coffin_in_hotbar(bot, _latest_coffin_instance_id(pre_second), 8)
        noop_anchor = last_event_time(bot)
        bot.intent(
            {
                "type": "coffin_break",
                "v": 1,
                "x": place_pos[0],
                "y": place_pos[1],
                "z": place_pos[2],
            }
        )
        # 第二次 break 处理完毕的水位线：+1 give 后只新增一口棺——若重复破坏再
        # grant 回收料，模板差分会暴露 double-grant。
        post_second = _give_barrier(
            bot,
            9,
            description=(
                "重复破坏（空→空）后 +1 give 应恰好 count==9——"
                "第二次 break 不得消费任何实例"
            ),
        )
        baseline_items = _template_counts(pre_second)
        post_second_items = _template_counts(post_second)
        second_break_delta = {
            item_id: count - baseline_items.get(item_id, 0)
            for item_id, count in post_second_items.items()
            if count != baseline_items.get(item_id, 0)
        }
        assert second_break_delta == {COFFIN_ID: 1}, (
            f"重复 coffin_break（空→空）必须只新增 give 的 1 个棺材，"
            f"实际模板差分={second_break_delta}（基线全量={total_baseline}）"
        )
        _assert_no_coffin_marker_destroy(
            bot,
            marker_entity_id,
            noop_anchor,
            description="重复 coffin_break 不得再次 despawn 已清场的 marker 实体",
        )

        replace_anchor = last_event_time(bot)
        _send_coffin_place(
            bot, place_pos, _first_coffin_instance_id(post_second)
        )
        replaced = _wait_coffin_count(
            bot,
            8,
            replace_anchor,
            description=(
                "破坏清场后同一坐标重放必须成功（9→8）——"
                "证明 destroy path 未在 registry 留 residue"
            ),
        )
        assert _coffin_count(replaced) == 8, "破坏后重放应恰好消费 1 个实例"
        # 世界侧：重放同样必须在世界重新 spawn marker 实体——旧实体已被 despawn、
        # registry 已清空后，新放置能产生新的世界实体（destroy path 无残留的完整闭环）。
        replace_marker = _wait_coffin_marker_spawn(
            bot,
            place_pos,
            replace_anchor,
            description="破坏清场后重放应再次 spawn mundane_coffin marker 实体（kind=160 @ 放置位）",
        )
        # review finding [4]（round 6）：最终重放的 marker 与 registry 条目同样无
        # teardown——break 清场，场景结束时零注册棺、零世界 marker 残留（同一长活
        # E2E server 可原样重跑本场景，也把 place_pos 交还给同 fixture 区的后续场景）。
        replace_teardown_anchor = last_event_time(bot)
        _send_coffin_break(bot, place_pos)
        _wait_coffin_marker_destroy(bot, replace_marker, replace_teardown_anchor)

        # review finding [major] round 8 收口：全部 teardown 之后从事件流重建 live
        # kind=160 实体集并断言为空——任何 spawn 未配对 destroy 的 marker（异维孤儿、
        # 重复放置孤儿、漏破的 teardown marker）在此红（场景声称零世界 marker 残留）。
        _assert_no_live_coffin_marker_after_teardown(
            bot,
            description="CoffinPlace 全生命周期场景收尾后",
        )

        bot.assert_alive("CoffinPlace 全生命周期场景完成后")
