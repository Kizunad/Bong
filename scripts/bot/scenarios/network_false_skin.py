"""伪皮肤组：forge_false_skin / equip_false_skin。

黑盒契约面（server/src/combat/tuike.rs + client_request_handler.rs）：
- `forge_false_skin` → FalseSkinForgeRequest → handle_false_skin_forge_requests：
  成功 → 扣材料（ash_spider_silk×1）+ 扣真元（qi_cost=5.0，走 zone ledger）+
  产出 tuike_false_skin_silk 入包 + revision bump（Changed<PlayerInventory> 推快照）；
  失败（RealmTooLow 先于 NotEnoughQi 先于 MissingMaterial）→ 仅 warn log，
  库存/灵气均不动、revision 不变（负断言用「窗口内无更高 revision 的快照」——
  服务端每 100 tick 周期 flush 会推 revision 不变的当前状态快照，必须容忍）。
  SpiderSilk min_realm=Induce（Awaken → RealmTooLow）。
- `equip_false_skin` → 忽略请求里的 slot（plan-layered-equip-v1 P0.1 伪皮归
  Chest/Worn 层），恒走 handle_inventory_move 到 Equip{Chest, Worn}：
  伪皮物品进胸槽受境界门控（can_equip_false_skin）；境界不足 →
  `inventory_move_rejected`{reason=RealmTooLow, required_realm=Induce} + 权威回推
  （revision 不变、物品保留）；境界够 → 物品移入 equipped.chest_worn + revision bump；
  instance 不存在 → warn + revision 不变（负断言同上）。
"""

import re
import time

from bot.bot import BotAssertionError
from bot.scenarios._combat_helpers import last_event_time, wait_for_ready
from bot.scenarios._inventory_helpers import (
    drain_inventory_quiet,
    equip_location,
    find_instance,
    find_item,
    latest_inventory_snapshot,
    require_item,
    wait_inventory_revision_after,
)

DESCRIPTION = (
    "伪皮肤：forge 境界/灵气/材料拒绝、forge 产出、equip 正路径/境界拒绝/静默"
)
MODULES = ["inventory", "cultivation", "combat"]

SILK = "ash_spider_silk"
FALSE_SKIN = "tuike_false_skin_silk"
NEGATIVE_WINDOW = 2.0
# forge 正路径的 qi 契约（server/src/combat/tuike.rs qi_cost()）：SpiderSilk
# 扣 qi_cost=5.0，经 release_qi_amount_to_zone → zone ledger（zone.spirit_qi
# 归一化存储，QI_ZONE_UNIT_CAPACITY=50.0），目的地 zone 增加 5.0/50.0 = 0.10。
FORGE_QI_COST = 5.0
QI_ZONE_UNIT_CAPACITY = 50.0
# 目的地 zone 信用断言容差（normalized spirit_qi 单位，期望增量 0.10 的 5%）。
# zone qi 可能被 NPC regen drain / skill cast 在两次探针读之间轻微扰动；但「直接扣
# qi_current 却完全不入账」的绕过（delta=0）与错额入账都远超此容差，仍被抓红。
ZONE_QI_DELTA_TOLERANCE = 0.01
# 拒绝回执之后的因果窗口：拒绝处理在**同一 tick** 同步 emit rejection + resync
# 回推（毫秒级到达），而 revision 不变的周期 flush 约 5s 一条。把回推快照限定在
# rejected_t 之后 1.0s 内。单靠 5s 周期假设不能排除 flush 恰好落在窗口里
# （review finding [2]），所以拒绝意图锚定前必须排干到 quiet=2.0s 静默
# （window 1.0 < quiet 2.0）——排干后下一次 flush 至少 2.0s 才可能到，窗口内不可能。
ROLLBACK_WINDOW = 1.0


def _give_and_wait(bot, item_id: str) -> dict:
    """锚定 give 之后的快照，且物品必须在随身层（非 equipped）。

    equipped 层也搜得到同模板物品（如 step 4 已穿上的伪皮），不排除的话
    give-wait 会命中旧快照里的已装备实例，真正的 give 快照落入后续负断言窗口。"""
    anchor = last_event_time(bot)
    bot.cmd(f"give {item_id} 1")
    time.sleep(0.5)  # give 走 chat→command 通道，先落一拍再等快照（冷启动实测坑）
    event = bot.wait_for(
        lambda e: (
            e.kind == "server_data"
            and e.data.get("payload_type") == "inventory_snapshot"
            and e.t > anchor
            and (item := find_item(e.data["payload"], item_id)) is not None
            and item["location"]["kind"] != "equip"
        ),
        timeout=10.0,
        description=f"give 后（时间锚之后）随身层含 {item_id} 的 inventory_snapshot",
    )
    return event.data["payload"]


def _inventory_signature(snapshot: dict) -> tuple:
    """背包占用字段的规范投影：placed/equipped/hotbar/resource 等会随 forge 消耗
    而变的字段。排除推导字段（weight/body_level/qi_*/realm/revision）——周期 flush
    与拒绝回推共享同一份当前状态，推导字段可能被无关系统漂移，占用字段则不会
    在没有 mutation 的情况下变化。"""
    placed = sorted(
        (
            p["container_id"],
            p["row"],
            p["col"],
            p["item"]["instance_id"],
            p["item"].get("count", 1),
            p["item"]["item_id"],
        )
        for p in snapshot.get("placed_items", [])
    )
    equipped = {
        slot: [
            (i["instance_id"], i.get("count", 1), i["item_id"])
            for i in (items if isinstance(items, list) else ([items] if items else []))
        ]
        for slot, items in snapshot.get("equipped", {}).items()
    }
    hotbar = [
        None
        if slot is None
        else (slot["instance_id"], slot.get("count", 1), slot["item_id"])
        for slot in snapshot.get("hotbar", [])
    ]
    return (placed, equipped, hotbar, snapshot.get("bone_coins"))


def _expect_no_inventory_change(bot, anchor_t: float, baseline: dict) -> None:
    """失败路径负断言：窗口内不得出现 inventory 变更（revision bump **或**占用字段漂移）。

    服务端每 100 tick 会周期 flush 一次「revision 不变的当前状态快照」（实测 ~5s
    一条），负判据必须容忍它；拒绝路径（RealmTooLow/NotEnoughQi/instance 不存在）
    从不 bump revision，所以「窗口内无更高 revision」通常等价于「无变更」。
    但 review finding [8]：revision 不动不代表内容不动——错误实现可直接改容器/装备
    字段而不走 bump_revision API（绕过 revision 感知的写路径），仍产生一条 revision
    不变的快照。故在 revision 检查之外再对权威快照做占用字段签名比对：基线捕获在
    intent 前、期间无任何 inventory 命令，签名任何漂移都是拒绝路径违规。
    review finding [5]（round 10）：签名比对必须拿 **e.t > anchor_t** 的权威快照——
    错误实现不 bump revision 也不立即发快照时，NEGATIVE_WINDOW 内可能没有新快照，
    `latest_inventory_snapshot` 仍指向 intent 前的基线，签名比对落空。本实现 wait_for
    下一条 > anchor_t 的快照（拒绝路径无回推，唯一来源是周期 flush ~5s 一条），
    其携带的才是请求后的权威状态。"""
    baseline_revision = int(baseline["revision"])
    time.sleep(NEGATIVE_WINDOW)
    stray = [
        e
        for e in bot.events_of("server_data")
        if e.data.get("payload_type") == "inventory_snapshot"
        and e.t > anchor_t
        and int(e.data["payload"].get("revision", -1)) > baseline_revision
    ]
    if stray:
        raise BotAssertionError(
            f"[{bot.username}] 期望 {NEGATIVE_WINDOW}s 内无 revision 变更"
            f"（>{baseline_revision}），实际收到 {len(stray)} 条"
        )
    # central-review 31438252846 finding [3]：warn-only 拒绝路径（forge 境界/灵气/材料
    # 拒绝 + equip instance 不存在）不得下发任何**用户可见拒绝回执**。旧实现只断
    # 库存/qi 守恒，一个「保留库存与灵气、却在每步误发 inventory_move_rejected」的
    # 服务端实现也能全过，违背场景声明的 wire 契约。inventory_move_rejected 是本组
    # 唯一的拒绝 payload 类型（realm-gated equip 正路径同管线下发，见
    # _wait_move_rejected——那一步走正向断言、不经本 helper），窗口内出现即红。
    stray_rejected = [
        e
        for e in bot.events_of("server_data")
        if e.data.get("payload_type") == "inventory_move_rejected" and e.t > anchor_t
    ]
    if stray_rejected:
        raise BotAssertionError(
            f"[{bot.username}] 期望 warn-only 拒绝路径不发送 inventory_move_rejected"
            f"（无用户可见回执），实际收到 {len(stray_rejected)} 条"
        )
    # review finding [5]：签名比对必须拿 **e.t > anchor_t** 的权威快照。拒绝路径
    # （RealmTooLow/NotEnoughQi/MissingMaterial/instance 不存在）从不主动回推，唯一
    # > anchor_t 的快照来源是周期 flush（~5s 一条，revision 不变）——等到下一条
    # （若窗口内已到达则立即返回），其携带的才是请求后的权威状态。
    post = bot.wait_for(
        lambda e: (
            e.kind == "server_data"
            and e.data.get("payload_type") == "inventory_snapshot"
            and e.t > anchor_t
        ),
        timeout=10.0,
        description=f"请求后（t>{anchor_t:.2f}）的权威 inventory_snapshot（周期 flush）",
    ).data["payload"]
    # central-review 2012 #5 回归：post 快照本身必须保持基线 revision——旧实现只比对
    # 占用字段签名，错误实现「只 bump revision、不改任何物品位置/数量」发出一条签名
    # 不变的快照就能通过。revision 单调递增，post 是窗口内第一条 post-anchor 快照；
    # 若等待期间已有 bump，post 必然携带 > 基线的 revision，此处直接抓红。
    if int(post["revision"]) != baseline_revision:
        raise BotAssertionError(
            f"[{bot.username}] 拒绝后 revision 应保持 {baseline_revision}（无 bump），"
            f"实际 {post['revision']}"
        )
    if _inventory_signature(post) != _inventory_signature(baseline):
        raise BotAssertionError(
            f"[{bot.username}] 拒绝后背包占用字段应保持不变（基线 revision="
            f"{baseline_revision}），实际内容漂移"
        )
    # central-review 2012 #5 回归（续）：wait_for 取的是窗口内**第一条** post-anchor
    # 快照——若它是 bump 前到达的周期 flush，而 bump 快照在其后到达，上面 post 断言
    # 已放过。此刻 bump 事件已进入事件列表，全量重扫补漏（含等待 post 期间到达的
    # revision bump，旧实现从不在取到 post 后再扫）。
    stray_late = [
        e
        for e in bot.events_of("server_data")
        if e.data.get("payload_type") == "inventory_snapshot"
        and e.t > anchor_t
        and int(e.data["payload"].get("revision", -1)) > baseline_revision
    ]
    if stray_late:
        raise BotAssertionError(
            f"[{bot.username}] 期望窗口内无 revision 变更（>{baseline_revision}），"
            f"实际 wait_for 之后重扫仍发现 {len(stray_late)} 条"
        )


def _assert_qi_unchanged_after_rejection(bot, expected_qi: float, realm_id: str) -> None:
    """拒绝路径 qi 守恒：warn-only 拒绝不改 Cultivation，player_state 不会自动重发。

    `qi set <expected_qi>` 会无条件重写 qi_current（cmd/dev/qi.rs 只有 Set/Max 变体），
    把错误拒绝路径已扣掉的真元修回去、再自证清白——探针读回的就是它自己写的值
    （review finding [1] 的根因）。改用 `realm set <当前境界>` 做 qi 无关的重发探针：
    handle_realm 恒赋值 cultivation.realm（realm.rs，同值写仍触发 Changed<Cultivation>
    的 player_state 发射，与 `qi set` 同值写同款机制），且**从不碰 qi**——重发读回的
    spirit_qi 就是权威当前值。错误实现若在拒绝时扣了 qi，重发值必然 != expected_qi，
    wait_for 超时即红；探针无法再修复扣减。请求锚定：e.t > probe_anchor 排除探针前
    一切历史 player_state（含 setup 的 realm/qi 写旧态）。"""
    probe_anchor = last_event_time(bot)
    bot.cmd(f"realm set {realm_id}")
    bot.expect_chat("[dev] realm set", timeout=10.0)
    bot.wait_for(
        lambda e: (
            e.kind == "server_data"
            and e.data.get("payload_type") == "player_state"
            and e.t > probe_anchor
            and abs(float(e.data["payload"].get("spirit_qi", -1.0)) - expected_qi) < 1e-6
        ),
        timeout=10.0,
        description=f"拒绝后（realm-set 重发探针）spirit_qi 保持 {expected_qi}",
    )


def _wait_move_rejected(bot, anchor_t: float, timeout: float = 10.0) -> tuple[dict, float]:
    event = bot.wait_for(
        lambda e: (
            e.kind == "server_data"
            and e.data.get("payload_type") == "inventory_move_rejected"
            and e.t > anchor_t
        ),
        timeout=timeout,
        description=f"inventory_move_rejected（t>{anchor_t:.2f}）",
    )
    return event.data["payload"], event.t


def _read_current_zone_qi(bot) -> float:
    """`zone_qi get` 只读探针：回显执行者当前所在 zone 的权威 spirit_qi。

    server/src/cmd/dev/zone_qi.rs：GetCurrent 用执行者 Position + CurrentDimension
    解析所在 zone，回显 `[dev] zone_qi <name> spirit_qi=... zone_total=...`。
    场景在 forge intent 前后各读一次，断言目的地 zone 增量 == qi_cost /
    QI_ZONE_UNIT_CAPACITY——这是「扣真元走 zone ledger」契约在 wire 上可观察的
    目的地侧证据（source 侧由 player_state.spirit_qi 断言，两侧合起来才是守恒对）。
    锚定 e.t > anchor 排除历史回显；读不到值直接抛错（探针必须成功，不能静默跳过）。"""
    anchor = last_event_time(bot)
    bot.cmd("zone_qi get")
    event = bot.wait_for(
        lambda e: (
            e.kind == "chat"
            and e.t > anchor
            and "spirit_qi=" in e.data["text"]
        ),
        timeout=10.0,
        description="zone_qi get 回显（[dev] zone_qi <name> spirit_qi=...）",
    )
    match = re.search(r"spirit_qi=([-0-9.]+)", event.data["text"])
    if not match:
        raise BotAssertionError(
            f"[{bot.username}] zone_qi get 回显无法解析 spirit_qi：{event.data['text']!r}"
        )
    return float(match.group(1))


def run(env) -> None:
    with env.new_bot("FalseSkin") as bot:
        wait_for_ready(bot)
        # join 初始 inventory_snapshot 与 game_join/pos_look 异步到达（不同推送），
        # 先消费掉再锚，否则它会落进负断言窗口（实测坑）。
        bot.wait_for(
            lambda e: (
                e.kind == "server_data"
                and e.data.get("payload_type") == "inventory_snapshot"
            ),
            timeout=10.0,
            description="join 初始 inventory_snapshot",
        )
        # /reset 归零 realm/qi/背包——保证「fresh Awaken + qi=0」前提成立，
        # 否则 persisted bong.db 会恢复上轮 run 的 realm=Induce（实测坑）。
        bot.cmd("reset")
        bot.expect_chat("[dev] reset self", timeout=10.0)
        time.sleep(1.0)  # 命令通道冷却：reset 后 0.6s 内 give 仍会被静默丢弃（实测坑，1.0s 稳）
        # 首个 drain 在会话早期（reset mutation 不落 5.0s 网格、其与 flush@5 不成对），
        # 网格确认需等第二条真 flush（flush@5 + flush@10，~11.25s 处 age=1.25 返回）；
        # max_wait 8.0（deadline ≈9.5s）会在确认前误超时，放 14.0（review finding
        # [1][2]：回推不可锚，确认前只能等）。
        drain_inventory_quiet(bot, quiet=1.2, max_wait=14.0)

        # ── 1. forge 拒绝-境界：fresh Awaken → RealmTooLow，无回推 ──
        # central-review 2012 #3：qi=0 时扣 qi 不可观测（0−5 被 clamp 回 0），先抬到
        # 1 让「拒绝时误扣真元」的错误实现可被探针读到差异。
        bot.cmd("qi set 1")
        bot.expect_chat("[dev] qi set", timeout=10.0)
        anchor = last_event_time(bot)
        baseline = latest_inventory_snapshot(bot)
        bot.intent({"type": "forge_false_skin", "v": 1, "kind": "spider_silk"})
        _expect_no_inventory_change(bot, anchor, baseline)
        # reset 归零 realm=Awaken（见上方 reset 注释）；realm-set 探针同值写不改变境界。
        _assert_qi_unchanged_after_rejection(bot, 1.0, "awaken")

        # ── 2. forge 拒绝-灵气：境界够但 qi=1（< qi_cost 5）→ NotEnoughQi，无回推 ──
        bot.cmd("realm set induce")
        bot.expect_chat("[dev] realm set", timeout=10.0)
        # qi 仍是 1（上步 realm-set 探针为同值写，未改 qi_current）；拒绝路径同样断言 qi 守恒。
        anchor = last_event_time(bot)
        baseline = latest_inventory_snapshot(bot)
        bot.intent({"type": "forge_false_skin", "v": 1, "kind": "spider_silk"})
        _expect_no_inventory_change(bot, anchor, baseline)
        # 当前境界为 Induce（本步开头 realm set induce）；探针同值写不改变境界。
        _assert_qi_unchanged_after_rejection(bot, 1.0, "induce")

        # ── 2b. forge 拒绝-材料：境界够 + qi 够但缺 ash_spider_silk → MissingMaterial ──
        # review finding [1]（round 9）：拒收步 Awaken qi=1 / Induce qi=1 都停在更靠前
        # 的 realm/qi 检查；唯一 qi 够（5）的请求先 give silk 后成功——没有任何请求
        # 到达 realm=Induce、qi>=5、无 ash_spider_silk 的状态，服务端 forge 顺序
        # RealmTooLow→NotEnoughQi→MissingMaterial（tuike.rs forge_false_skin）的
        # 最后分支从未被触达。错误实现若在 MissingMaterial 分支创建伪皮、先扣 qi
        # 再返回 MissingMaterial、选错结果或变异库存，会全过六步。必须把请求推到
        # 该分支并验证文档化契约：qi 不变 + 库存占用不变 + revision 不变（无回推）。
        bot.cmd("qi set 5")
        bot.expect_chat("[dev] qi set", timeout=10.0)
        anchor = last_event_time(bot)
        baseline = latest_inventory_snapshot(bot)
        bot.intent({"type": "forge_false_skin", "v": 1, "kind": "spider_silk"})
        _expect_no_inventory_change(bot, anchor, baseline)
        # 当前境界为 Induce（step 2 开头 realm set induce）；qi 已抬到 5——qi>=qi_cost
        # 才能越过 NotEnoughQi 到达材料检查。MissingMaterial 是 warn-only 拒绝，
        # 探针（realm-set 同值写，不碰 qi）读回 spirit_qi 必须仍是 5.0；错误实现
        # 若在材料检查前扣真元，探针读回 != 5.0，wait_for 超时即红。
        _assert_qi_unchanged_after_rejection(bot, 5.0, "induce")

        # ── 3. forge 正路径：补灵气后产出 + 扣材料 + revision bump ──
        bot.cmd("qi set 5")
        bot.expect_chat("[dev] qi set", timeout=10.0)
        silk_snapshot = _give_and_wait(bot, SILK)
        forge_revision = int(silk_snapshot["revision"])
        # central-review 2012 #2：forge 正路径必须同时验证「扣真元走 zone ledger」的
        # 目的地侧——只断言 player_state.spirit_qi==0 会让「直接扣 qi_current、绕过
        # ledger、不进 zone」的错误实现也通过。intent 前用 zone_qi get 探针读一次
        # 当前 zone 的权威 spirit_qi 作基线；forge 成功后读回、断言增量 == qi_cost
        # / QI_ZONE_UNIT_CAPACITY（0.10），与 source 侧扣减合起来才是守恒对。
        zone_baseline = _read_current_zone_qi(bot)
        # central-review 2012 #3 回归：qi 断言必须锚在 forge intent 之后——否则
        # 「qi set 5 之前」spirit_qi=0 的旧 player_state 会满足 <5.0，扣真元缺失的
        # forge 也能通过。watermark 取 intent 前（含 zone 探针读之后），排除一切历史快照。
        forge_anchor = last_event_time(bot)
        bot.intent({"type": "forge_false_skin", "v": 1, "kind": "spider_silk"})
        forged = wait_inventory_revision_after(bot, forge_revision, timeout=10.0)
        assert int(forged["revision"]) > forge_revision, (
            f"forge 成功后 revision 应 bump（>{forge_revision}），实际 {forged['revision']}"
        )
        require_item(forged, FALSE_SKIN)
        assert find_item(forged, SILK) is None, (
            f"forge 后材料 {SILK} 应被消耗，实际仍在快照中"
        )
        # 真元走 zone ledger 扣除：forge intent 之后下发的 player_state.spirit_qi
        # 必须恰好等于 5.0 − qi_cost(5.0) = 0.0（守恒敏感契约，central-review 2012
        # #10）——只断言「< 5.0」会让扣 0.1、双扣成负等错误实现也通过。e.t >
        # forge_anchor 排除历史 qi=0 快照；浮点容差 1e-6 吸收序列化抖动。
        bot.wait_for(
            lambda e: (
                e.kind == "server_data"
                and e.data.get("payload_type") == "player_state"
                and e.t > forge_anchor
                and abs(float(e.data["payload"].get("spirit_qi", 9.9)) - 0.0) < 1e-6
            ),
            timeout=10.0,
            description="forge 扣真元后（intent 之后）spirit_qi == 0.0（5.0 − 5.0）",
        )
        # central-review 2012 #2 回归：destination zone 信用断言。源（player -5.0，
        # 上面 player_state）与目的地（zone +0.10）两侧都在，才证明 qi 确实走了
        # zone ledger 而不是直接蒸发。预期增量 = FORGE_QI_COST / QI_ZONE_UNIT_CAPACITY。
        zone_after = _read_current_zone_qi(bot)
        expected_zone_delta = FORGE_QI_COST / QI_ZONE_UNIT_CAPACITY
        zone_delta = zone_after - zone_baseline
        if abs(zone_delta - expected_zone_delta) > ZONE_QI_DELTA_TOLERANCE:
            raise BotAssertionError(
                f"[{bot.username}] forge 成功后当前 zone spirit_qi 应增加 "
                f"{expected_zone_delta}（qi_cost {FORGE_QI_COST} / capacity "
                f"{QI_ZONE_UNIT_CAPACITY}），实际 {zone_baseline} -> {zone_after}"
                f"（delta={zone_delta}）；绕过 zone ledger 的扣真元实现会在此抓红"
            )

        # ── 4. equip 正路径：伪皮进 equipped.chest_worn + revision bump ──
        false_skin = require_item(forged, FALSE_SKIN)
        false_skin_instance = int(false_skin["item"]["instance_id"])
        equip_revision = int(forged["revision"])
        # central-review 2012 #2：请求 slot 用非规范值 "legs"——若服务端错误地
        # honor 请求字段，伪皮会落 legs 而非 chest，下方 location 断言即红。旧场景
        # 两个 equip 请求都发规范槽 "chest"，无法区分「恒落 Chest/Worn」与「按请求
        # slot 转换」两种实现。
        bot.intent(
            {
                "type": "equip_false_skin",
                "v": 1,
                "slot": "legs",
                "item_instance_id": false_skin_instance,
            }
        )
        equipped = wait_inventory_revision_after(bot, equip_revision, timeout=10.0)
        worn = find_item(equipped, FALSE_SKIN)
        # central-review 2012 #6：equip_false_skin 必须忽略请求里的 slot、恒落
        # Equip{Chest, Worn}——只断言 kind=="equip" 会让错误实现把伪皮塞进手/饰品等
        # 别的装备层也通过。精确 pin 槽位 chest + 状态 worn（请求发的是 legs）。
        assert worn is not None and worn["location"] == equip_location("chest", "worn"), (
            f"equip（请求 slot=legs）后 {FALSE_SKIN} 应落 equipped.chest_worn"
            f"（Chest/Worn），实际 {worn!r}"
        )

        # ── 5. equip 拒绝-境界：降回 Awaken 再穿 → RealmTooLow + 回推 revision 不变 ──
        bot.cmd("realm set awaken")
        bot.expect_chat("[dev] realm set", timeout=10.0)
        spare_snapshot = _give_and_wait(bot, FALSE_SKIN)
        spare = require_item(spare_snapshot, FALSE_SKIN)
        spare_instance = int(spare["item"]["instance_id"])
        # review finding [2]：周期 flush（~5s 一条，revision 不变）可落进
        # ROLLBACK_WINDOW 冒充拒绝回推。锚定前排干到 quiet=2.0s——下一次
        # flush 至少 2.0s 后才可能到，而回推窗口只有 1.0s（window < quiet）。
        drain_inventory_quiet(bot, quiet=2.0)
        reject_anchor = last_event_time(bot)
        reject_revision = int(latest_inventory_snapshot(bot)["revision"])
        # central-review 2012 #2：请求 slot 同样用非规范值 "off_hand"——境界拒绝
        # 与 slot 无关（realm 门控先于槽位解析），但错误实现若 honor 请求字段并落
        # off_hand，realm 门控下同样拒绝——此处验证拒绝仍按 Chest/Worn 契约发生
        # （reason=realm_too_low + 被拒实例原位不动）。
        bot.intent(
            {
                "type": "equip_false_skin",
                "v": 1,
                "slot": "off_hand",
                "item_instance_id": spare_instance,
            }
        )
        rejected, rejected_t = _wait_move_rejected(bot, reject_anchor)
        assert rejected.get("reason") == "realm_too_low", (
            f"inventory_move_rejected.reason 应为 realm_too_low（serde snake_case），"
            f"实际 {rejected.get('reason')!r}"
        )
        assert rejected.get("required_realm") == "Induce", (
            f"inventory_move_rejected.required_realm 应为 Induce，实际 "
            f"{rejected.get('required_realm')!r}"
        )
        # central-review 2012 #3 回归：回推快照必须因果锚在拒绝回执**之后**
        # （rejected_t），不是 intent 前水位——否则请求与拒绝之间任意周期性 flush
        # 的「revision 不变」快照都能冒充权威回推，拒绝停发回推时断言仍过。
        # 服务端 emit_inventory_move_rejected 先于 resync_snapshot 下发，rejected_t
        # 之后必有真实回推快照。再加 ROLLBACK_WINDOW 上限：拒绝处理在同一 tick 同步
        # 回推（毫秒级到达），周期 flush ~5s 一条，窗口把「回执之后较晚的无关 flush」
        # 也排除——回推必须是对拒绝回执的即时后果。
        kept = bot.wait_for(
            lambda e: (
                e.kind == "server_data"
                and e.data.get("payload_type") == "inventory_snapshot"
                and e.t > rejected_t
                and e.t < rejected_t + ROLLBACK_WINDOW
            ),
            timeout=10.0,
            description=(
                f"拒绝回执之后 {ROLLBACK_WINDOW}s 内（t∈({rejected_t:.2f}, "
                f"{rejected_t + ROLLBACK_WINDOW:.2f})）的回推 inventory_snapshot"
            ),
        ).data["payload"]
        assert int(kept["revision"]) == reject_revision, (
            f"境界拒绝不得移动物品：revision 应保持 {reject_revision}，实际 {kept['revision']}"
        )
        # central-review 2012 #2：第 4 步已装备一件同模板伪皮，模板级 require_item
        # 会被旧装备件满足——错误的拒绝实现「丢失新给 spare_instance 却不动旧件」
        # 也能过。必须 pin 到被拒实例本身 + 非 equipped 原位。
        kept_spare = find_instance(kept, spare_instance)
        assert kept_spare is not None, (
            f"境界拒绝后被拒伪皮实例 {spare_instance} 应仍在背包"
            f"（原位 {spare['location']!r}），实际快照中未找到该实例"
        )
        assert kept_spare["location"] == spare["location"], (
            f"境界拒绝不得移动被拒实例：位置应保持 {spare['location']!r}，"
            f"实际 {kept_spare['location']!r}"
        )

        # ── 6. equip 静默：instance 不存在 → warn + revision 无变更 ──
        anchor = last_event_time(bot)
        baseline = latest_inventory_snapshot(bot)
        bot.intent(
            {
                "type": "equip_false_skin",
                "v": 1,
                "slot": "chest",
                "item_instance_id": 999999,
            }
        )
        _expect_no_inventory_change(bot, anchor, baseline)

        bot.assert_alive("伪皮肤 7 步正负路径后")
