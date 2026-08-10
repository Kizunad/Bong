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

防双授断言（本场景核心）：
1. 放棺后 mundane_coffin 从背包**消失**（消耗恰好一次，不残留）；
2. 回收后返料精确 = ling_mu_ban×6 + ling_mu_gun×2（不多不少，落在容器槽位）；
3. 回收后 mundane_coffin **不复现**（回收只返材料、不返棺材本体，杜绝自我复制）；
4. 同位置二次回收必须被拒——用无关 probe give 使库存推进，然后**回扫二次回收
   窗口内推送到客户端的每一个 inventory_snapshot**：返料数量都必须保持 6/2、且
   不得出现 mundane_coffin。若二次回收被误处理，第一个快照就已返料翻倍（多即
   双授、少即吞料），必然被窗口扫描捕获。

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
~1s 换位而非空等 45s×2（review #1），`_coffin_consumed_after`
兜底改扫全窗口而非首个快照（review #4）。

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

DESCRIPTION = "放凡物棺→G菜单回收→精确返料 ling_mu_ban×6+ling_mu_gun×2 且二次回收被拒无双授"
MODULES = ["inventory", "coffin"]

COFFIN_ITEM_ID = "mundane_coffin"
# coffin.mundane_coffin 配方原料（Reclaim 模式全量返还，确定性）。
RECLAIM_LING_MU_BAN = 6
RECLAIM_LING_MU_GUN = 2
# 反双授 probe：与棺材/返料无关的 dev give，用于让库存前进、从而采样二次回收
# 之后的首个 probe 快照（若二次回收被误处理，该窗口内任何快照都已返料翻倍）。
PROBE_ITEM_ID = "fan_tie"

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


def _reclaim_snapshot_ok(snapshot) -> bool:
    """二次回收窗口内合规快照判据：返料总数 6/2、每条落容器、棺材不复现。"""
    for item_id, expected in (
        ("ling_mu_ban", RECLAIM_LING_MU_BAN),
        ("ling_mu_gun", RECLAIM_LING_MU_GUN),
    ):
        entries = _entries_for_item(snapshot, item_id)
        if sum(int(e["item"]["stack_count"]) for e in entries) != expected:
            return False
        if any(e["location"]["kind"] != "container" for e in entries):
            return False
    return _entries_for_item(snapshot, COFFIN_ITEM_ID) == []


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


def _coffin_consumed_after(bot, anchor, timeout: float = STAGE_TIMEOUT) -> bool:
    """anchor 之后棺材是否已被消耗（不在背包）——probe give 超时后的兜底消耗确证。

    review #4：旧实现取窗口内**首个** post-anchor 快照作判据——无关周期快照可能在
    放置仍处理中时到达、显示棺材仍在，旧实现立即 return False，循环带同一 instance
    换位；放置随后完成时其消耗快照被下一候选的 wait 误认（张冠李戴，placed_at 记错、
    回收打空位）。改扫 anchor 之后窗口内**全部** inventory_snapshot：任一显示棺材已
    消失即消耗成立（return True）；窗口耗尽仍无消失证据才 return False。窗口由
    timeout 界定（覆盖全套件退化下的延迟处理）。注意本 helper 只是放置循环 probe give
    超时的兜底——probe give 因果快照（同连接包序，必在 coffin_place 之后）才是放置
    判负的确定性信号（放置循环主路径用 probe 判据，review #1）。
    """
    deadline = time.monotonic() + timeout
    cursor = 0
    while time.monotonic() < deadline:
        events = bot.events
        while cursor < len(events):
            e = events[cursor]
            cursor += 1
            if e.t <= anchor:
                continue
            if e.kind == "server_data" and e.data["payload_type"] == "inventory_snapshot":
                if find_item(e.data["payload"], COFFIN_ITEM_ID) is None:
                    return True
        time.sleep(0.25)
    return False


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

        # ── 放棺：真实 instance_id + 消耗恰好一次 ────────────────────────
        # wait_inventory_contains 无时间锚点、每次 wait_for 从事件历史起点扫描；若
        # 服务器已持久化过含棺材的 PlayerState（重连快照带棺材），`give` 后它会命中
        # 连接时（clearinv 前）的旧快照——instance_id 陈旧会让后续所有 coffin_place 被
        # 「missing item instance」拒掉、放置循环空转 45s/候选（fixture 实测）。锚定到
        # give 之后并要求快照含棺材，确保拿到 give 真正回推的新实例。
        coffin_anchor = last_event_time(bot)
        bot.cmd(f"give {COFFIN_ITEM_ID} 1")
        coffin_ev = bot.wait_for(
            lambda e: e.kind == "server_data"
            and e.data["payload_type"] == "inventory_snapshot"
            and e.t > coffin_anchor
            and find_item(e.data["payload"], COFFIN_ITEM_ID) is not None,
            timeout=STAGE_TIMEOUT,
            description=f"give {COFFIN_ITEM_ID} 后含棺材的 inventory_snapshot",
        )
        snapshot = coffin_ev.data["payload"]
        coffin_item = require_item(snapshot, COFFIN_ITEM_ID)

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
                "item_instance_id": int(coffin_item["item"]["instance_id"]) + 100000,
            },
            probe_item_id=PROBE_ITEM_ID,
            still_present=[(COFFIN_ITEM_ID, 1)],
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
            still_present=[(COFFIN_ITEM_ID, 1), (PROBE_ITEM_ID, fan_before)],
            label="非棺物品实例放棺",
        )

        # 放棺位置扫描：coffin 占据 (x,y,z)+(x+1,y,z) 两格，二者都必须为空气。
        # 地表 y≈73-74 平坦，但出生点附近仍有 POI 结构/树（实测东侧 px+2 被占、
        # 「not empty」被拒），故逐候选尝试、以 probe give 的包序终态（coffin_place
        # 之后按包序处理 give，其因果快照即放置终态）判成功，并记录实际落点供后续
        # 回收寻址。被拒的放棺服务端不推任何快照。
        placed_at = None
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
                    "item_instance_id": int(coffin_item["item"]["instance_id"]),
                }
            )
            # review #1：被拒候选不再空等 45s 的「成功快照」——服务端
            # 对被拒放棺不推任何快照，旧实现每个被拒候选等满 45s、再经
            # _coffin_consumed_after 又 45s（八个候选全被拒约 12min）。改 probe give 判
            # 终态：give 与 coffin_place 同连接按包序处理，give 的因果快照（fan_tie 总数
            # > probe_before）反映「coffin_place 处理之后」的库存——棺材消失 = 放置成功
            # （消耗恰好一次、不残留，成功快照判定未削弱），棺材仍在 = 放置被拒（立即换
            # 位，~1s/候选）。probe give 自身在 STAGE_TIMEOUT 内无因果快照（服务端连 give
            # 都不回推，极端退化）才退回到 _coffin_consumed_after 的扫描式消耗确证。
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
                if _coffin_consumed_after(bot, anchor):
                    placed_at = cpos
                    break
                continue
            if find_item(probe_ev.data["payload"], COFFIN_ITEM_ID) is None:
                placed_at = cpos
                break
            continue
        assert placed_at is not None, (
            "coffin_place 在所有候选格均被拒（出生点地形全占/异常），无法完成放置 leg"
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
        anchor = last_event_time(bot)
        bot.intent(
            {
                "type": "coffin_menu_reclaim",
                "v": 1,
                "x": placed_at[0],
                "y": placed_at[1],
                "z": placed_at[2],
            }
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
        assert find_item(reclaim_snapshot, COFFIN_ITEM_ID) is None, (
            f"回收后 {COFFIN_ITEM_ID} 不应复现——回收只返配方材料、不返棺材本体，"
            "若出现说明回收路径在复制道具"
        )

        # ── 反双授：二次回收必须被拒，二次回收窗口内任何快照都不得翻倍 ───
        anchor = last_event_time(bot)
        bot.intent(
            {
                "type": "coffin_menu_reclaim",
                "v": 1,
                "x": placed_at[0],
                "y": placed_at[1],
                "z": placed_at[2],
            }
        )
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
        # （按全部堆叠求和）必须始终 6/2、每条返料都落容器槽位、mundane_coffin 必须
        # 始终不出现。窗口内还有周期性 inventory_changed（教程灵鼠扣 qi 等），故不能
        # 只查 probe 快照；逐条扫描才能证明二次回收没有在任何一条快照里推过翻倍返料
        # 或棺材本体。
        bad_snapshots = [
            e.data["payload"]
            for e in bot.events
            if e.kind == "server_data"
            and e.data["payload_type"] == "inventory_snapshot"
            and anchor < e.t <= post_event.t
            and not _reclaim_snapshot_ok(e.data["payload"])
        ]
        assert not bad_snapshots, (
            f"二次回收必须被拒（registry 已摘除）：(anchor, probe] 窗口内出现 "
            f"{len(bad_snapshots)} 条违规快照（返料总数非 6/2、返料落非容器槽位或"
            f"棺材复现），首条={bad_snapshots[0]}——二次回收被误处理会先推 "
            "coffin_menu_reclaimed 并在当条快照翻倍返料"
        )
        _assert_exact_reclaim(post_snapshot, "ling_mu_ban", RECLAIM_LING_MU_BAN, "二次回收后")
        _assert_exact_reclaim(post_snapshot, "ling_mu_gun", RECLAIM_LING_MU_GUN, "二次回收后")
        assert find_item(post_snapshot, COFFIN_ITEM_ID) is None, (
            f"二次回收后 {COFFIN_ITEM_ID} 不应出现（放置时已消耗一次，回收不返棺材）"
        )

        bot.assert_alive("coffin_menu_reclaim 全链路后")
