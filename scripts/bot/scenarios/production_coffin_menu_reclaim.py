"""延寿棺 G 菜单 [回收] 全链路：放棺 → 回收 → 精确返料 + 反重复授予。

plan-coffin-tiers-v1 P2/P3 验收场景。黑盒契约面（与 server/src/coffin/mod.rs 对齐）：

- `coffin_place{x,y,z,item_instance_id}` 必须用真实 instance_id 且手持目标为
  延寿棺物品模板（`CoffinGrade::from_item_id` 按 id 校验）。放棺成功消耗该实例
  （`consume_item_instance_once`），回推 `coffin_place_consumed` 快照。
- `coffin_menu_reclaim{x,y,z}` 寻址 registry 中的棺：近距校验 → `remove_by_pos`
  摘除 lower/upper 双索引 + despawn marker → 按 grade 查 `coffin.<grade>` craft 配方
  用 `recipe_reclaim_drops(ReclaimMode::Reclaim)` 计算返还 → 全量发还 inventory，
  成功回推 `coffin_menu_reclaimed` 快照。
- 凡物棺 `coffin.mundane_coffin` 配方 materials = ling_mu_ban×6 + ling_mu_gun×2，
  Reclaim 模式是**确定性全量返还**（无 Break 的随机损耗），可直接断言精确数量。

**语义澄清（相对 GAP11 原始规格的更正）**：本场景的棺是**延寿棺**（放置即化器物、
按档位提供寿元倍率），**不是储物容器**——`CoffinEntity` 没有道具账本、`Coffin*`
意图族没有 deposit 类请求，回收返还的是配方合成材料而非「存入的道具」。因此原始
规格里「CoffinPlace→CoffinOpen→放入道具→CoffinMenuReclaim 断言道具原样回来」的
前提在服务端不存在（`CoffinOpen` 属于出生点教程石棺系统，只授龛石一次）。本场景
改为锁定同一批真实失败模式：**回收必须精确返还一次、且绝不双授**——这正是
GAP11 排第一的判据（处理不当复制/双授道具腐坏账本）。

防双授断言（本场景核心，finding [2] 双实例 fixture）：
1. 放棺后放置实例 a 从背包**消失**、未放置实例 b 恰好保留 1 条（实例级单次消费——
   「删整堆 / 按模板删全部 mundane_coffin」的错误实现，在只发一口棺的旧 fixture 下
   全绿，此处红）；
2. 回收后返料精确 = ling_mu_ban×6 + ling_mu_gun×2（不多不少，落在容器槽位）；
3. 回收后放置实例 a **不复现**、b 恰好保留 1 条（回收只返材料、不返棺材本体，
   杜绝自我复制）；
4. 同位置二次回收必须被拒——用无关 probe give 使库存推进，然后**回扫二次回收
   窗口内推送到客户端的每一个 inventory_snapshot**：返料数量都必须保持 6/2、且
   a 不得复现 / b 不得双授。若二次回收被误处理，第一个快照就已返料翻倍（多即
   双授、少即吞料），必然被窗口扫描捕获。

**G 菜单 [回收] 生产者驱动（review finding [1]）**：成功回收 leg 不再冷注入
`bot.intent({"type":"coffin_menu_reclaim",...})`——改经 `_g_menu_reclaim` 驱动生产
G 菜单交互的可观测前置与端点：① 派发前置（`CoffinEnterIntentHandler.candidate`
黑盒镜像）——目标 marker 渲染实体（kind=160 @ marker 位）必须存在且 live，marker
不 spawn / G 无对象的构建在此红；② 交互半径（candidate
`MAX_INTERACT_DISTANCE_SQ=36` 黑盒镜像）——玩家必须落在 marker 的 6 格交互半径内；
③ 菜单 [回收] 端点——发送 `_reclaim_payload` 的钉死字节（黑盒镜像
`ClientRequestProtocol.encodeCoffinMenuReclaim`；client golden test
`ClientRequestProtocolTest.encodesCoffinMenuReclaim` 钉死相同 JSON，菜单 emit 错
payload 的实现被 client 侧红）。远程/边界外拒绝 leg 与二次回收 leg 是**伪造请求的
防御性测试**：真实 G 交互在那些位置/时刻根本无法产生（candidate 距离检查返回
empty、主回收后 marker 已 despawn），它们钉的是服务端对异常请求的拒绝路径，与
生产者驱动的成功 leg 互补——「G 菜单 [回收] 全链路」由 client 生产者测试（G 派发
/ 按钮接线 / payload）与本场景消费端共同钉死，任何一环错实现都有红点。

**双实例 fixture（review finding [2]）**：`mundane_coffin` max_stack_count=1，两发
give 各得独立实例（a=放置、b=未放置对照）。成功放置后断言 b 恰好保留 1 条、a
永久消失；回收/二次回收后同样按实例断言（a 不返、b 恒在）——「删除整堆 / 按模板
删全部 mundane_coffin」与「回收复制道具」的错误实现在此红。

**原子结算（review finding [3]）**：放置循环的 wait 超时只说明「没等到该给的因果
快照」，不代表 coffin_place 已被处理——请求可能仍在途。超时后先 `_drain_settle`
（barrier give 的同连接包序因果快照必然排在 coffin_place 之后）在该快照上判定
终态，绝不在未结算时换位；`_coffin_consumed_after` 的窗口扫描只观察状态、不建立
请求已结束（迟到成功快照会被下一候选的 >probe_before 谓词误认、placed_at 记错），
已移除。

**精确数量 = 全部堆叠求和**：返料断言一律枚举 item_id 的**所有**匹配条目（容器/
装备/快捷栏）并对其 stack_count 求和，再逐条校验 location 为容器槽位——单条目
`find_item` 只看第一个堆叠，会让「合法 6/2 之外再各多一叠」的复制实现通过；求和
语义下任何额外的同 id 堆叠都会让总数偏离 6/2 而被捕获（review run 31409926323
复核点）。

**负面校验面（review run 31424073123 [major] 复核点）**：黑盒契约声明的三项校验
必须被负面用例实打实按压，否则「忽略 instance_id / 只看 item id / 删掉近距校验」
的错误实现能通过全部正向断言：
1. 伪造/陈旧的 `item_instance_id` 放棺必须被拒（`inventory_item_by_instance` 落空）——
   被拒不消耗任何实例，随后快照中 mundane_coffin 仍在背包；
2. 真实实例但非棺模板（fan_tie 的 instance_id）放棺必须被拒（`CoffinGrade::from_item_id`
   落空）——fan_tie 与棺材都不得被消耗；
3. 距棺 >6 格（`COFFIN_INTERACT_MAX_DISTANCE_SQ=36`，max 交互距离 6 格）的远程
   回收必须被拒（`coffin_target_is_close` 落空）——不得授予任何返料，且后续近距离
   回收仍能命中同一 registered 棺。
4. 六格边界与 off-by-one 过渡必须被钉死（review #3）：恰在边界上（d2=36.0）的回收
   必须成功、紧贴边界外（d2=37.0，center+(6,1,0)）必须被拒。旧案只采「明显有效
   （~2-3 格）+ 明显无效（≥9 格）」两点，`distance_sq < 36`（而非 `<= 36`）或边界
   外拒收的实现能通过全部断言。move_to 按包序提交精确 Position（server 无
   anticheat），落点逐值验证（`_stand_at`）后从恰在边界上的点发起主回收 leg。
服务端对放棺/回收被拒不推任何快照，故负面断言一律以「状态不变」为判据：被拒放棺
不得消耗实例、被拒回收不得授予返料。第 3 项用 probe give 把一张排在回收请求之后
的库存快照钉在窗口里断言返料恒为 0。
probe 快照识别一律用**总数递增**谓词（give 前总数 → 快照总数 > 之），不用
`find_item(...) is not None`：probe item 经此前若干次 probe give 已存在于背包，
存在性谓词匹配无关周期快照、会把验证窗口提前截断（review #2）。放棺判负同用 probe
give 的包序终态（coffin_place 之后按包序处理 give，其快照即放置终态）——被拒候选
~1s 换位而非空等 45s×2（review #1）；wait 超时先 `_drain_settle` 原子结算
（finding [3]），绝不在请求未结束时换位。

**位置稳定性**：bot 在出生点高空初始化（[8,150,8]），spawn_selector 按
（seed, InitialLogin）稳定哈希到 safe_y 高度的出生点，随后**下落**到平坦地表
（novice raster 地表 y≈73-74）。因此 `wait_for_ready` 后 bot.position 仍在下落中；
本场景先等待服务器位置连续 2s 不再变化（落地完成），再采样放棺坐标——否则全部
候选格都会因 bot 已落至地面而「too far」被拒（全套件实测的失败模式）。
"""

import time

from bot.bot import BotAssertionError

from bot.scenarios._combat_helpers import last_event_time, wait_for_ready
from bot.scenarios._inventory_helpers import (
    find_item,
    require_item,
    wait_inventory_contains,
)

DESCRIPTION = "放凡物棺(双实例)→G菜单[回收]交互→精确返料 ling_mu_ban×6+ling_mu_gun×2 且二次回收被拒无双授"
MODULES = ["inventory", "coffin"]

COFFIN_ITEM_ID = "mundane_coffin"
# coffin.mundane_coffin 配方原料（Reclaim 模式全量返还，确定性）。
RECLAIM_LING_MU_BAN = 6
RECLAIM_LING_MU_GUN = 2
# 反双授 probe：与棺材/返料无关的 dev give，用于让库存前进、从而采样二次回收
# 之后的首个 probe 快照（若二次回收被误处理，该窗口内任何快照都已返料翻倍）。
PROBE_ITEM_ID = "fan_tie"

# mundane_coffin marker 渲染实体 kind（server 侧 entity kind，S2C_ENTITY_SPAWN type）。
COFFIN_MARKER_ENTITY_KIND = 160
# G 菜单派发交互半径平方（CoffinEnterIntentHandler.MAX_INTERACT_DISTANCE_SQ = 6² 黑盒镜像）。
COFFIN_INTERACT_MAX_DISTANCE_SQ = 36.0

# 全套件长跑后 server TPS 退化（forge 场景 45s 先例），stage 等待统一 45s。
STAGE_TIMEOUT = 45.0
# 等待服务器位置稳定（出生下落完成）的判稳窗口。
STABLE_POSITION_WINDOW = 2.0


def _entries_for_item(snapshot, item_id):
    """枚举 inventory_snapshot 中 item_id 的**全部**匹配条目（容器/装备/快捷栏）。

    与 _inventory_helpers.find_item 返回结构一致（location+item），但返回所有
    匹配项而非第一个——多堆叠才可能被总数断言看见。条目各自携带
    container_id/row/col/instance_id，多个同 id 堆叠是可表示的；只查第一个
    会让额外的堆叠对每条回归断言不可见（复核：该疏漏允许「合法 6/2 之外再
    各多一叠」的复制实现通过全部断言）。
    """
    entries = []
    for placed in snapshot.get("placed_items", []):
        if placed["item"]["item_id"] == item_id:
            entries.append(
                {
                    "location": {
                        "kind": "container",
                        "container_id": placed["container_id"],
                        "row": placed["row"],
                        "col": placed["col"],
                    },
                    "item": placed["item"],
                }
            )
    for slot, values in snapshot.get("equipped", {}).items():
        if slot.endswith("_worn"):
            equip_slot = slot[: -len("_worn")]
            for item in values:
                if item["item_id"] == item_id:
                    entries.append(
                        {
                            "location": {"kind": "equip", "slot": equip_slot, "state": "worn"},
                            "item": item,
                        }
                    )
        elif slot.endswith("_held"):
            item = values
            if item and item["item_id"] == item_id:
                entries.append(
                    {
                        "location": {
                            "kind": "equip",
                            "slot": slot[: -len("_held")],
                            "state": "held",
                        },
                        "item": item,
                    }
                )
    for index, item in enumerate(snapshot.get("hotbar", [])):
        if item and item["item_id"] == item_id:
            entries.append({"location": {"kind": "hotbar", "index": index}, "item": item})
    return entries


def _total_stack_count(snapshot, item_id) -> int:
    """item_id 在所有堆叠上的总数（全部条目 stack_count 求和）。"""
    return sum(int(e["item"]["stack_count"]) for e in _entries_for_item(snapshot, item_id))


def _assert_exact_reclaim(snapshot, item_id, expected, stage_label):
    """断言 item_id 的总数恰好 == expected 且每条返料都落在容器槽位。

    总数按全部堆叠求和：任何额外的同 id 堆叠（多即双授）或缺失（少即吞料）
    都会让总数偏离 expected 而被捕获。location 逐条校验，杜绝返料落到错误
    槽位（装备/快捷栏）的绕过。
    """
    entries = _entries_for_item(snapshot, item_id)
    total = sum(int(e["item"]["stack_count"]) for e in entries)
    assert total == expected, (
        f"{stage_label} {item_id} 应恰好 ×{expected}，实际总数={total}"
        f"（{len(entries)} 条堆叠；多即双授、少即吞料）"
    )
    off_container = [e["location"] for e in entries if e["location"]["kind"] != "container"]
    assert not off_container, (
        f"{stage_label} {item_id} 每条返料都应落入背包容器槽位，实际 {off_container}"
    )


def _coffin_state_ok(snapshot, placed_id, untargeted_id) -> bool:
    """回收/二次回收后棺材状态不变量：放置实例（placed_id）不得复现，未放置实例
    （untargeted_id）必须恰好保留 1 条。任何复制/双授/复现放置实例都红。

    finding [2]：单棺 fixture 把「回收不返棺材」断言建立在「背包无棺材」上——一个
    「回收时把背包里另一口未放置棺也删掉」或「回收返一口新棺」的实现都能混过去。
    双实例下放置实例必须永久消失、未放置实例恒在，两路错误实现分别红。
    """
    entries = _entries_for_item(snapshot, COFFIN_ITEM_ID)
    if len(entries) != 1:
        return False
    return int(entries[0]["item"]["instance_id"]) == untargeted_id


def _reclaim_snapshot_ok(snapshot, placed_id, untargeted_id) -> bool:
    """二次回收窗口内合规快照判据：返料总数 6/2、每条落容器、棺材状态不变。"""
    for item_id, expected in (
        ("ling_mu_ban", RECLAIM_LING_MU_BAN),
        ("ling_mu_gun", RECLAIM_LING_MU_GUN),
    ):
        entries = _entries_for_item(snapshot, item_id)
        if sum(int(e["item"]["stack_count"]) for e in entries) != expected:
            return False
        if any(e["location"]["kind"] != "container" for e in entries):
            return False
    return _coffin_state_ok(snapshot, placed_id, untargeted_id)


def _wait_position_stable(bot, window: float = STABLE_POSITION_WINDOW, timeout: float = 30.0):
    """等待服务器位置连续 `window` 秒不变（出生下落落地）。

    S2C_POS_LOOK 更新 bot.position；下落期间位置持续变化，落地后静止。
    用连续同值时长判稳，而不是单次采样，避免瞬时静止误判。
    """
    deadline = time.monotonic() + timeout
    last_sample = None
    stable_since = None
    while time.monotonic() < deadline:
        current = bot.position
        if current is not None and current == last_sample:
            if stable_since is None:
                stable_since = time.monotonic()
            elif time.monotonic() - stable_since >= window:
                return
        else:
            stable_since = None
        last_sample = current
        time.sleep(0.25)
    raise BotAssertionError(
        f"等待服务器位置稳定超时（{timeout:.0f}s），最后位置={bot.position}"
        "——出生下落未在时限内落地"
    )


def _assert_place_rejected(bot, request, probe_item_id, still_present, label):
    """发送放棺请求并确证被拒：用 probe give 把一张排在请求之后的库存快照钉在
    窗口里，断言 `still_present`（(item_id, 期望总数) 列表）未被消耗。

    服务端对被拒放棺不推任何快照（只 warn）；probe give 在请求之后按包序处理，
    其快照反映「请求被处理之后」的库存。被拒放棺不得消耗任何实例，故各物品总数
    必须保持原值——伪造 instance / 非棺模板的错误实现若真去消耗，总数必然偏离。
    """
    anchor = last_event_time(bot)
    bot.intent(request)
    bot.cmd(f"give {probe_item_id} 1")
    post = bot.wait_for(
        lambda e: e.kind == "server_data"
        and e.data["payload_type"] == "inventory_snapshot"
        and e.t > anchor
        and find_item(e.data["payload"], probe_item_id) is not None,
        timeout=STAGE_TIMEOUT,
        description=f"probe give {probe_item_id} 后快照（{label} 被拒验证）",
    )
    snapshot = post.data["payload"]
    for item_id, expected in still_present:
        total = _total_stack_count(snapshot, item_id)
        assert total == expected, (
            f"{label}：{item_id} 总数应保持 {expected}（被拒放棺不得消耗任何实例），"
            f"实际={total}"
        )


def _probe_total_now(bot) -> int:
    """当前 probe item（fan_tie）的已知总数——最新一条 inventory_snapshot 的堆叠求和。

    give 因果快照的谓词必须是「总数 > 本函数返回值」而非 find_item 存在性：probe
    item 经此前若干次 probe give 已存在于背包，存在性谓词会匹配任何无关周期快照、
    把验证窗口提前截断（review #2 的疏漏点）。总数只被 give 推进，最新快照即当前
    真实总数。
    """
    payload = None
    for e in reversed(bot.events):
        if e.kind == "server_data" and e.data["payload_type"] == "inventory_snapshot":
            payload = e.data["payload"]
            break
    return _total_stack_count(payload, PROBE_ITEM_ID) if payload is not None else 0


def _find_coffin_instance(snapshot, instance_id) -> dict | None:
    """snapshot 中指定 instance_id 的 mundane_coffin 条目（无则 None）。

    实例级消费判据：放置成功 = 放置实例从背包消失（逐条比对 instance_id），不是
    find_item 存在性——双实例 fixture 下未放置实例 b 仍在，存在性谓词永不判负
    （旧实现单棺时靠「棺材从背包消失」判成功，双棺下必须改实例级，finding [2]）。
    """
    for e in _entries_for_item(snapshot, COFFIN_ITEM_ID):
        if int(e["item"]["instance_id"]) == instance_id:
            return e
    return None


def _drain_settle(bot, cpos, description) -> dict:
    """超时候选的原子结算（finding [3]）：barrier give 的同连接包序因果快照。

    wait_for 超时只说明「没等到该给的因果快照」，不代表 coffin_place 已被处理——
    请求可能仍在途。绝不在未结算时换位（旧 _coffin_consumed_after 窗口扫描只观察
    状态、不能建立请求已结束，迟到成功快照会被下一候选的 >probe_before 谓词误认、
    placed_at 记错，正是 finding 3 的攻击面）。再发一个 barrier give：同连接按包序
    处理，其因果快照（fan_tie 总数 > 现值）必然排在 coffin_place **之后**，在该
    快照上判定放置实例是否被消费即为该候选的确定性终态。barrier 也超时（服务端
    全面停摆）直接报错，不静默扫描、不换位。
    """
    barrier_before = _probe_total_now(bot)
    bot.cmd(f"give {PROBE_ITEM_ID} 1")
    barrier_ev = bot.wait_for(
        lambda e: e.kind == "server_data"
        and e.data["payload_type"] == "inventory_snapshot"
        and _total_stack_count(e.data["payload"], PROBE_ITEM_ID) > barrier_before,
        timeout=STAGE_TIMEOUT,
        description=(
            f"候选 {cpos} 结算 barrier give {PROBE_ITEM_ID} 因果快照"
            f"（总数 > {barrier_before}）"
        ),
    )
    return barrier_ev.data["payload"]


def _coffin_marker_pos(lower) -> tuple[float, float, float]:
    """mundane_coffin marker 渲染实体位置（server coffin/mod.rs coffin_marker_position）。"""
    return (lower[0] + 1.0, float(lower[1]), lower[2] + 0.5)


def _near(pos, x, y, z, tol: float = 0.01) -> bool:
    return (
        abs(pos[0] - x) < tol and abs(pos[1] - y) < tol and abs(pos[2] - z) < tol
    )


def _find_marker_entity(bot, lower) -> int | None:
    """lower 对应 coffin 的 marker 实体 id（entity_spawn kind=160 @ marker 位），
    事件流中最后一次匹配；无则 None。marker 是 G 菜单派发（CoffinEnterIntentHandler.
    candidate）的目标——找不到即「G 键无对象可开菜单」（finding [1]）。
    """
    marker_pos = _coffin_marker_pos(lower)
    found = None
    for e in bot.events:
        if e.kind == "entity_spawn" and e.data.get("type") == COFFIN_MARKER_ENTITY_KIND:
            if _near((e.data["x"], e.data["y"], e.data["z"]), *marker_pos):
                found = e.data["entity_id"]
    return found


def _marker_live(bot, entity_id) -> bool:
    """marker 实体是否仍存活（S2C_ENTITIES_DESTROY 后 bot.entity_pos 返回 None）。"""
    return bot.entity_pos(entity_id) is not None


def _reclaim_payload(lower) -> dict:
    """G 菜单 [回收] 的生产 payload 契约。

    黑盒镜像 client `ClientRequestProtocol.encodeCoffinMenuReclaim` 的字节——client/
    src/test/.../ClientRequestProtocolTest.encodesCoffinMenuReclaim 钉死相同 JSON
    （{"type":"coffin_menu_reclaim","v":1,x,y,z}）。本场景发送同一字节即消费与生产
    菜单相同的 wire 契约；「菜单 emit 错 payload」的实现被 client golden test 红。
    """
    return {
        "type": "coffin_menu_reclaim",
        "v": 1,
        "x": lower[0],
        "y": lower[1],
        "z": lower[2],
    }


def _g_menu_reclaim(bot, lower, description) -> int:
    """驱动生产 G 菜单 [回收] 交互（headless bot 无 Java GUI，驱动该交互的可观测
    前置与端点，而非冷注入）：

    1. 派发前置（CoffinEnterIntentHandler.candidate 黑盒镜像）：目标 marker 实体
       （kind=160 @ marker 位）必须存在且 live——marker 不 spawn / G 无对象的构建红；
    2. 交互半径（candidate MAX_INTERACT_DISTANCE_SQ=36 黑盒镜像）：玩家必须落在
       marker 的 6 格交互半径内，否则 G 派发返回 empty、菜单根本打不开；
    3. 菜单 [回收] 端点：发送 _reclaim_payload 的钉死字节（生产 encoder 契约）。

    返回 marker entity_id 供主回收后 despawn 断言复用。
    """
    marker_id = _find_marker_entity(bot, lower)
    assert marker_id is not None, (
        f"{description}：G 菜单派发目标 marker 实体不存在"
        f"（{_coffin_marker_pos(lower)}）——marker 未 spawn 或类型/位置不符"
    )
    assert _marker_live(bot, marker_id), (
        f"{description}：marker 实体 #{marker_id} 已不在世界（G 键无目标可开菜单）"
    )
    assert bot.position is not None, f"{description}：bot.position 缺失"
    marker_pos = _coffin_marker_pos(lower)
    dsq = sum((a - b) ** 2 for a, b in zip(bot.position, marker_pos))
    assert dsq <= COFFIN_INTERACT_MAX_DISTANCE_SQ + 1e-6, (
        f"{description}：玩家距 marker {dsq:.3f} > 36（G 菜单交互半径外，"
        "candidate() 返回 empty、菜单打不开）"
    )
    bot.intent(_reclaim_payload(lower))
    return marker_id


def _stand_at(bot, target, attempts: int = 3, settle: float = 0.8) -> None:
    """move_to 到目标并确认落点**逐值相等**（server 按包序提交客户端 Position）。

    review #3：边界检查把交互距离推到恰好 d2=36.0/37.0，落点差一格即改变比较结果，
    必须精确落位而不能近似。server 无移动 anticheat、valence 按包提交客户端位置，
    move_to 末包即精确目标（f64 精确往返）；万一末包滞后/被吞，重发同目标再等
    settle 应能收敛。连续 attempts 次仍未逐值相等则移动链路异常（连粗粒度落点都
    不可靠），直接报错。
    """
    for _ in range(attempts):
        bot.move_to(*target, speed=5.5)
        time.sleep(settle)
        if bot.position == target:
            return
    raise BotAssertionError(
        f"[{bot.username}] move_to {target} 未能精确落位（{attempts} 次），"
        f"实际={bot.position}"
    )


def run(env) -> None:
    with env.new_bot("Coffin") as bot:
        wait_for_ready(bot)
        bot.cmd("clearinv all")
        bot.expect_chat("[dev] clearinv", timeout=30.0)

        # ── 位置稳定：出生高空下落先落地，再采样放棺坐标 ──────────────────
        _wait_position_stable(bot)
        assert bot.position is not None, (
            "pos_look 后应有 bot.position（wait_for_ready 已保证）"
        )
        px, py, pz = (int(v) for v in bot.position)

        # ── 放棺：真实 instance_id + 双实例 fixture ──────────────────────
        # wait_inventory_contains 无时间锚点、每次 wait_for 从事件历史起点扫描；若
        # 服务器已持久化过含棺材的 PlayerState（重连快照带棺材），`give` 后它会命中
        # 连接时（clearinv 前）的旧快照——instance_id 陈旧会让后续所有 coffin_place 被
        # 「missing item instance」拒掉、放置循环空转 45s/候选（fixture 实测）。锚定到
        # give 之后并要求快照含棺材，确保拿到 give 真正回推的新实例。
        # ── 双实例 fixture（finding [2]）──
        # mundane_coffin max_stack_count=1，两发 give 各得独立实例。放置 leg 用 a、
        # b 为未放置对照——断言「放置恰好消费 a、b 恒在」才能区分实例级单次消费 vs
        # 整堆/模板级删除（「删掉整个选中堆叠」「按模板删全部 mundane_coffin」的错误
        # 实现在只发一口棺的旧 fixture 下全绿）。
        coffin_anchor = last_event_time(bot)
        bot.cmd(f"give {COFFIN_ITEM_ID} 1")
        coffin_ev = bot.wait_for(
            lambda e: e.kind == "server_data"
            and e.data["payload_type"] == "inventory_snapshot"
            and e.t > coffin_anchor
            and find_item(e.data["payload"], COFFIN_ITEM_ID) is not None,
            timeout=STAGE_TIMEOUT,
            description=f"give {COFFIN_ITEM_ID} #1 后含棺材的 inventory_snapshot",
        )
        snapshot_a = coffin_ev.data["payload"]
        coffin_a = require_item(snapshot_a, COFFIN_ITEM_ID)
        a_id = int(coffin_a["item"]["instance_id"])

        coffin_anchor2 = last_event_time(bot)
        bot.cmd(f"give {COFFIN_ITEM_ID} 1")
        coffin_ev2 = bot.wait_for(
            lambda e: e.kind == "server_data"
            and e.data["payload_type"] == "inventory_snapshot"
            and e.t > coffin_anchor2
            and _total_stack_count(e.data["payload"], COFFIN_ITEM_ID) >= 2,
            timeout=STAGE_TIMEOUT,
            description=f"give {COFFIN_ITEM_ID} #2 后总数为 2 的 inventory_snapshot",
        )
        snapshot_b = coffin_ev2.data["payload"]
        coffin_entries = _entries_for_item(snapshot_b, COFFIN_ITEM_ID)
        assert len(coffin_entries) == 2, (
            f"两发 give 后应有 2 条独立 mundane_coffin 实例，实际 {len(coffin_entries)}"
            "——max_stack_count=1 不应合并"
        )
        b_entry = next(e for e in coffin_entries if int(e["item"]["instance_id"]) != a_id)
        b_id = int(b_entry["item"]["instance_id"])
        assert a_id != b_id, "两发 give 必须产生不同 instance_id（双实例 fixture）"

        # ── 负面校验：黑盒契约的 instance 校验面必须被真负面用例按压 ──────
        # 服务端对被拒的放棺不推任何快照（只 warn），故负面断言以「状态不变」为
        # 判据：被拒不消耗任何实例。用 (px,py,pz)（玩家脚下）作目标，确保走到的是
        # instance 校验分支而不是近距校验分支。

        # (a) 伪造/陈旧的 item_instance_id：不存在的实例必须被拒
        #     （inventory_item_by_instance 落空）——棺材不得被消耗。
        _assert_place_rejected(
            bot,
            {
                "type": "coffin_place",
                "v": 1,
                "x": px,
                "y": py,
                "z": pz,
                "item_instance_id": a_id + 100000,
            },
            probe_item_id=PROBE_ITEM_ID,
            still_present=[(COFFIN_ITEM_ID, 2)],
            label="伪造 instance_id 放棺",
        )

        # (b) 真实实例但非棺模板：fan_tie 的 instance_id 不得被放棺
        #     （CoffinGrade::from_item_id 落空）——fan_tie 与棺材都不得被消耗。
        # 记录当前 fan_tie 总数（(a) 的 probe give 已引入 1），被拒后必须保持。
        # wait_inventory_contains 无时间锚点、每次 wait_for 都从事件历史起点扫描
        # （cursor=0 含历史），fan_tie 已存在时可能命中 (a) 的旧 probe 快照；用
        # 锚点 + 「总数 >= 2」谓词确保采样到 give 生效后的快照（只有该快照满足
        # >= 2），instance_id 与 fan_before 才可信。
        fan_anchor = last_event_time(bot)
        bot.cmd(f"give {PROBE_ITEM_ID} 1")
        fan_ev = bot.wait_for(
            lambda e: e.kind == "server_data"
            and e.data["payload_type"] == "inventory_snapshot"
            and e.t > fan_anchor
            and _total_stack_count(e.data["payload"], PROBE_ITEM_ID) >= 2,
            timeout=STAGE_TIMEOUT,
            description=(
                f"give {PROBE_ITEM_ID} 后 fan_tie 总数 >= 2（负样本 instance 源）"
            ),
        )
        fan_snapshot = fan_ev.data["payload"]
        fan_item = require_item(fan_snapshot, PROBE_ITEM_ID)
        fan_before = _total_stack_count(fan_snapshot, PROBE_ITEM_ID)
        _assert_place_rejected(
            bot,
            {
                "type": "coffin_place",
                "v": 1,
                "x": px,
                "y": py,
                "z": pz,
                "item_instance_id": int(fan_item["item"]["instance_id"]),
            },
            probe_item_id="wood_handle",
            still_present=[(COFFIN_ITEM_ID, 2), (PROBE_ITEM_ID, fan_before)],
            label="非棺物品实例放棺",
        )

        # 放棺位置扫描：coffin 占据 (x,y,z)+(x+1,y,z) 两格，二者都必须为空气。
        # 地表 y≈73-74 平坦，但出生点附近仍有 POI 结构/树（实测东侧 px+2 被占、
        # 「not empty」被拒），故逐候选尝试。成功判定绑定到**放置实例 a**：放置成功
        # = a 从背包消失（_find_coffin_instance is None），未放置实例 b 恒在
        # （finding [2]）。probe give 的同连接包序因果快照（fan_tie 总数 > 放置前）
        # 即放置终态；wait 超时先 _drain_settle 原子结算（finding [3]），绝不在请求
        # 未结束时换位。被拒的放棺服务端不推任何快照。
        # review #1：被拒候选不再空等 45s 的「成功快照」——服务端对被拒放棺不推任何
        # 快照，旧实现每个被拒候选等满 45s、再经 _coffin_consumed_after 又 45s（八个
        # 候选全被拒约 12min）。probe give 判终态：give 与 coffin_place 同连接按包序
        # 处理，give 的因果快照反映「coffin_place 处理之后」的库存——a 消失 = 放置成功
        # （消耗恰好一次），a 仍在 = 放置被拒（立即换位，~1s/候选）。
        placed_at = None
        placed_snapshot = None
        for cpos in (
            (px - 2, py, pz),  # 西 2（forge 砧位实测可放，先试）
            (px + 2, py, pz),  # 东 2（本次全量跑实测被占，兜底顺序靠后）
            (px - 3, py, pz),
            (px + 3, py, pz),
            (px, py, pz - 2),
            (px, py, pz + 2),
            (px, py + 3, pz),  # 头顶上方开阔天空兜底
            (px, py + 2, pz),
        ):
            anchor = last_event_time(bot)
            probe_before = _probe_total_now(bot)
            bot.intent(
                {
                    "type": "coffin_place",
                    "v": 1,
                    "x": cpos[0],
                    "y": cpos[1],
                    "z": cpos[2],
                    "item_instance_id": a_id,
                }
            )
            bot.cmd(f"give {PROBE_ITEM_ID} 1")
            try:
                probe_ev = bot.wait_for(
                    lambda e: e.kind == "server_data"
                    and e.data["payload_type"] == "inventory_snapshot"
                    and e.t > anchor
                    and _total_stack_count(e.data["payload"], PROBE_ITEM_ID) > probe_before,
                    timeout=STAGE_TIMEOUT,
                    description=f"coffin_place@{cpos} 后 probe give 因果快照（放置终态）",
                )
            except BotAssertionError:
                # finding [3]：等待超时 ≠ 请求已处理——原子结算该候选（barrier give
                # 因果快照排在 coffin_place 之后），按该快照判定终态；barrier 也超时
                # 直接报错，绝不在未结算时换位（旧 _coffin_consumed_after 窗口扫描
                # 不建立请求已结束，迟到成功快照会被下一候选误认）。
                settle_snapshot = _drain_settle(
                    bot, cpos, description=f"coffin_place@{cpos} 等待超时"
                )
                if _find_coffin_instance(settle_snapshot, a_id) is None:
                    placed_at = cpos
                    placed_snapshot = settle_snapshot
                    break
                continue
            if _find_coffin_instance(probe_ev.data["payload"], a_id) is None:
                placed_at = cpos
                placed_snapshot = probe_ev.data["payload"]
                break
            continue
        assert placed_at is not None, (
            "coffin_place 在所有候选格均被拒（出生点地形全占/异常），无法完成放置 leg"
        )
        # finding [2]：实例级单次消费——放置后必须恰好剩 1 口棺且是未放置的 b 实例。
        remaining = _entries_for_item(placed_snapshot, COFFIN_ITEM_ID)
        assert len(remaining) == 1 and int(remaining[0]["item"]["instance_id"]) == b_id, (
            f"放置后应恰好剩 1 口 mundane_coffin 且为未放置的 b 实例（#{b_id}），"
            f"实际 {len(remaining)} 条——整堆/模板级删除（a 连带 b 消失）或复制"
            "（多余实例）的实现在此红"
        )

        # ── 负面校验：远程回收必须被拒（近距校验 coffin_target_is_close） ──
        # 先垂直升到棺上方 >6 格（头顶天空由候选兜底确认开阔；move_to 走 20Hz 小步，
        # server 无移动 anticheat，按包序提交 Position——craft-refund 同款同步模式，
        # 等 0.8s 让最后一步移动落地）。目标取 py+12：距所有候选格（x∈±3,y∈[py,py+3],
        # z∈±2）都 ≥9 格，即使最后一步移动提交滞后一格（py+11.275）仍 >6，不会因
        # 中间位置恰在交互范围内而误通过近距校验。随后对 placed_at 发回收，近距校验
        # （player Position vs coffin.lower，max 6 格）必须落空——不得授予任何返料。
        # 被拒后服务端不推快照，用 probe give 把一张排在回收请求之后的库存快照钉在
        # 窗口里断言返料恒为 0（回收若被误处理会先推 coffin_menu_reclaimed 并在
        # 当条快照翻倍返料，probe 快照一并暴露）。
        far_pos = (px, py + 12, pz)
        bot.move_to(*far_pos, speed=5.5)
        time.sleep(0.8)
        far_anchor = last_event_time(bot)
        bot.intent(
            {
                "type": "coffin_menu_reclaim",
                "v": 1,
                "x": placed_at[0],
                "y": placed_at[1],
                "z": placed_at[2],
            }
        )
        # review #2：probe 快照谓词必须用「总数 > give 前」而非 find_item 存在性——
        # fan_tie 经此前若干次 probe give 已存在于背包，存在性谓词匹配无关周期快照，
        # 会把 (far_anchor, far_post.t] 验证窗口提前截断（回收结果落在窗口外被漏掉）。
        far_probe_before = _probe_total_now(bot)
        bot.cmd(f"give {PROBE_ITEM_ID} 1")
        far_post = bot.wait_for(
            lambda e: e.kind == "server_data"
            and e.data["payload_type"] == "inventory_snapshot"
            and e.t > far_anchor
            and _total_stack_count(e.data["payload"], PROBE_ITEM_ID) > far_probe_before,
            timeout=STAGE_TIMEOUT,
            description=(
                f"远程回收被拒后 probe give {PROBE_ITEM_ID} 的因果快照"
                f"（fan_tie 总数 > {far_probe_before}）"
            ),
        )
        far_snapshot = far_post.data["payload"]

        # 回扫 (far_anchor, far_post.t] 窗口内每一个 inventory_snapshot：任何一条带
        # 返料（ling_mu_ban>0 或 ling_mu_gun>0）即违规。远程回收若被误处理会先推
        # coffin_menu_reclaimed 并在当条快照授予返料，probe 快照一并暴露。
        material_snapshots = [
            e.data["payload"]
            for e in bot.events
            if e.kind == "server_data"
            and e.data["payload_type"] == "inventory_snapshot"
            and far_anchor < e.t <= far_post.t
            and (
                _total_stack_count(e.data["payload"], "ling_mu_ban") > 0
                or _total_stack_count(e.data["payload"], "ling_mu_gun") > 0
            )
        ]
        assert not material_snapshots, (
            f"远程回收必须被拒（近距校验，max 6 格）：(far_anchor, probe] 窗口内出现 "
            f"{len(material_snapshots)} 条带返料的快照，首条={material_snapshots[0]}"
            "——远程回收被误处理会推 coffin_menu_reclaimed 并授予返料"
        )
        assert _total_stack_count(far_snapshot, "ling_mu_ban") == 0, (
            f"远程回收必须被拒（近距校验，max 6 格）：不得授予 ling_mu_ban，"
            f"实际={_total_stack_count(far_snapshot, 'ling_mu_ban')}"
        )
        assert _total_stack_count(far_snapshot, "ling_mu_gun") == 0, (
            f"远程回收必须被拒（近距校验，max 6 格）：不得授予 ling_mu_gun，"
            f"实际={_total_stack_count(far_snapshot, 'ling_mu_gun')}"
        )

        # ── 负面校验：紧贴六格边界外的回收必须被拒（off-by-one 过渡） ──────
        # review #3：旧案只采「明显有效（~2-3 格）+ 明显无效（≥9 格）」两点，恰在边界
        # 上取 `<`（distance_sq < 36 而非 <= 36）的实现能通过全部断言。此处补边界外
        # 紧邻点 center+(6,1,0) → d2=37：必须被拒。server 无移动 anticheat、按包序
        # 提交 Position，move_to 末包即精确目标；落点必须逐值相等（差一格即改变 d2
        # 的比较结果），不符则重发收敛（_stand_at）。
        boundary_center = (placed_at[0] + 0.5, placed_at[1] + 0.5, placed_at[2] + 0.5)
        just_out = (boundary_center[0] + 6.0, boundary_center[1] + 1.0, boundary_center[2])
        _stand_at(bot, just_out)

        out_anchor = last_event_time(bot)
        bot.intent(
            {
                "type": "coffin_menu_reclaim",
                "v": 1,
                "x": placed_at[0],
                "y": placed_at[1],
                "z": placed_at[2],
            }
        )
        out_probe_before = _probe_total_now(bot)
        bot.cmd(f"give {PROBE_ITEM_ID} 1")
        out_post = bot.wait_for(
            lambda e: e.kind == "server_data"
            and e.data["payload_type"] == "inventory_snapshot"
            and e.t > out_anchor
            and _total_stack_count(e.data["payload"], PROBE_ITEM_ID) > out_probe_before,
            timeout=STAGE_TIMEOUT,
            description=(
                f"边界外回收被拒后 probe give {PROBE_ITEM_ID} 的因果快照"
                f"（fan_tie 总数 > {out_probe_before}）"
            ),
        )
        out_snapshot = out_post.data["payload"]
        out_material_snapshots = [
            e.data["payload"]
            for e in bot.events
            if e.kind == "server_data"
            and e.data["payload_type"] == "inventory_snapshot"
            and out_anchor < e.t <= out_post.t
            and (
                _total_stack_count(e.data["payload"], "ling_mu_ban") > 0
                or _total_stack_count(e.data["payload"], "ling_mu_gun") > 0
            )
        ]
        assert not out_material_snapshots, (
            f"边界外（d2=37，紧贴六格边界）回收必须被拒：窗口内出现 "
            f"{len(out_material_snapshots)} 条带返料的快照，首条={out_material_snapshots[0]}"
        )
        assert _total_stack_count(out_snapshot, "ling_mu_ban") == 0, (
            f"边界外回收必须被拒：不得授予 ling_mu_ban，"
            f"实际={_total_stack_count(out_snapshot, 'ling_mu_ban')}"
        )
        assert _total_stack_count(out_snapshot, "ling_mu_gun") == 0, (
            f"边界外回收必须被拒：不得授予 ling_mu_gun，"
            f"实际={_total_stack_count(out_snapshot, 'ling_mu_gun')}"
        )
        bot.assert_alive("边界外回收被拒后")

        # ── 回收：恰在六格边界上（d2=36.0）必须成功（主回收 leg） ──────────
        # review #3：旧主回收从 ~2-3 格发起，`<` 与 `<=` 不可区分。把主回收 leg 移到
        # 恰在边界上的 center+(6,0,0) → d2=36.0：`<=36`（正确）成功、`<36`（off-by-one
        # 错误）被拒，比较运算符被钉死。落点仍逐值验证（_stand_at）。
        boundary_in = (boundary_center[0] + 6.0, boundary_center[1], boundary_center[2])
        _stand_at(bot, boundary_in)

        # ── 回收：精确返料（Reclaim 模式确定性全量） ─────────────────────
        # finding [1]：成功回收 leg 经 _g_menu_reclaim 驱动生产 G 菜单 [回收] 交互
        # （marker 目标存在 + 6 格交互半径 + 钉死 payload），不再冷注入请求字节。
        anchor = last_event_time(bot)
        marker_id = _g_menu_reclaim(
            bot,
            placed_at,
            description="主回收 leg（G 菜单 [回收] 交互驱动）",
        )
        reclaim_event = bot.wait_for(
            lambda e: e.kind == "server_data"
            and e.data["payload_type"] == "inventory_snapshot"
            and e.t > anchor
            and _total_stack_count(e.data["payload"], "ling_mu_ban") == RECLAIM_LING_MU_BAN
            and _total_stack_count(e.data["payload"], "ling_mu_gun") == RECLAIM_LING_MU_GUN,
            timeout=STAGE_TIMEOUT,
            description=(
                f"回收应精确返还 ling_mu_ban×{RECLAIM_LING_MU_BAN} + "
                f"ling_mu_gun×{RECLAIM_LING_MU_GUN}（coffin.mundane_coffin Reclaim "
                "全量返还，coffin_menu_reclaimed 快照；总数按全部堆叠求和，"
                "额外同 id 堆叠会令总数偏离而不满足）"
            ),
        )
        reclaim_snapshot = reclaim_event.data["payload"]

        _assert_exact_reclaim(reclaim_snapshot, "ling_mu_ban", RECLAIM_LING_MU_BAN, "回收返料")
        _assert_exact_reclaim(reclaim_snapshot, "ling_mu_gun", RECLAIM_LING_MU_GUN, "回收返料")
        # finding [2]：回收不返棺材本体、放置实例 a 不复现、未放置实例 b 恰好保留 1 条。
        assert _coffin_state_ok(reclaim_snapshot, a_id, b_id), (
            f"回收后放置实例 #{a_id} 不得复现、未放置实例 #{b_id} 必须恰好保留 1 条"
            "——回收复制/返棺材本体的实现在此红"
        )

        # ── 反双授：二次回收必须被拒，二次回收窗口内任何快照都不得翻倍 ───
        # finding [1]：主回收已 despawn marker → G 菜单无目标可开，二次回收只能由
        # 伪造/陈旧请求产生（真实玩家此刻无法再发起 G 交互）。先断言 marker 已清场
        # （世界侧证据），再注入伪造重复请求验证服务端对 registry 已摘除坐标静默
        # no-op（不双授）——「回收不返棺材本体」的实例级断言（_coffin_state_ok）兜底。
        assert not _marker_live(bot, marker_id), (
            f"主回收后 marker #{marker_id} 必须已 despawn（G 菜单无目标可开二次回收）"
        )
        anchor = last_event_time(bot)
        bot.intent(_reclaim_payload(placed_at))
        # probe give 是二次回收后唯一应由我们自己引发的库存事件；若二次回收被误处理，
        # 会先推 coffin_menu_reclaimed 快照（此时返料已翻倍），probe give 再推一个。
        # review #2：probe 快照谓词用「总数 > give 前」——fan_tie 已存在于背包，存在性
        # 谓词会匹配无关周期快照并提前截断 (anchor, post_event.t] 窗口（异步处理的二次
        # 回收可能落在窗口外）。总数只被 give 推进，递增谓词唯一命中 give 的因果快照。
        probe_before = _probe_total_now(bot)
        bot.cmd(f"give {PROBE_ITEM_ID} 1")
        post_event = bot.wait_for(
            lambda e: e.kind == "server_data"
            and e.data["payload_type"] == "inventory_snapshot"
            and e.t > anchor
            and _total_stack_count(e.data["payload"], PROBE_ITEM_ID) > probe_before,
            timeout=STAGE_TIMEOUT,
            description=(
                f"probe give {PROBE_ITEM_ID} 的因果快照（fan_tie 总数 > {probe_before}）"
            ),
        )
        post_snapshot = post_event.data["payload"]

        # 回扫 (anchor, post_event.t] 窗口内的每一个 inventory_snapshot：返料总数
        # （按全部堆叠求和）必须始终 6/2、每条返料都落容器槽位、放置实例 a 不得复现、
        # 未放置实例 b 必须恰好 1 条（_reclaim_snapshot_ok 实例级判据）。窗口内还有
        # 周期性 inventory_changed（教程灵鼠扣 qi 等），故不能只查 probe 快照；逐条
        # 扫描才能证明二次回收没有在任何一条快照里推过翻倍返料或棺材本体。
        bad_snapshots = [
            e.data["payload"]
            for e in bot.events
            if e.kind == "server_data"
            and e.data["payload_type"] == "inventory_snapshot"
            and anchor < e.t <= post_event.t
            and not _reclaim_snapshot_ok(e.data["payload"], a_id, b_id)
        ]
        assert not bad_snapshots, (
            f"二次回收必须被拒（registry 已摘除）：(anchor, probe] 窗口内出现 "
            f"{len(bad_snapshots)} 条违规快照（返料总数非 6/2、返料落非容器槽位、"
            f"放置实例复现或 b 非恰好 1 条），首条={bad_snapshots[0]}——二次回收被"
            "误处理会先推 coffin_menu_reclaimed 并在当条快照翻倍返料"
        )
        _assert_exact_reclaim(post_snapshot, "ling_mu_ban", RECLAIM_LING_MU_BAN, "二次回收后")
        _assert_exact_reclaim(post_snapshot, "ling_mu_gun", RECLAIM_LING_MU_GUN, "二次回收后")
        assert _coffin_state_ok(post_snapshot, a_id, b_id), (
            f"二次回收后放置实例 #{a_id} 不得复现、未放置实例 #{b_id} 必须恰好保留 1 条"
        )

        bot.assert_alive("coffin_menu_reclaim 全链路后")
