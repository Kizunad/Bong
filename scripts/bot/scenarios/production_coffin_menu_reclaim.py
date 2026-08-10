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
# 单次放棺尝试窗口：放棺被拒（如目标格被占）时服务端不推任何快照，短窗即可判负并换位。
PLACE_ATTEMPT_TIMEOUT = 8.0
# 等待服务器位置稳定（出生下落完成）的判稳窗口。
STABLE_POSITION_WINDOW = 2.0


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
        bot.cmd(f"give {COFFIN_ITEM_ID} 1")
        snapshot = wait_inventory_contains(bot, COFFIN_ITEM_ID)
        coffin_item = require_item(snapshot, COFFIN_ITEM_ID)

        # 放棺位置扫描：coffin 占据 (x,y,z)+(x+1,y,z) 两格，二者都必须为空气。
        # 地表 y≈73-74 平坦，但出生点附近仍有 POI 结构/树（实测东侧 px+2 被占、
        # 「not empty」被拒），故逐候选尝试、以服务端 coffin_place_consumed 快照
        # （mundane_coffin 消失）为准判成功，并记录实际落点供后续回收寻址。被拒的
        # 放棺服务端不推任何快照，短窗判负后换下一候选。
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
            try:
                bot.wait_for(
                    lambda e: e.kind == "server_data"
                    and e.data["payload_type"] == "inventory_snapshot"
                    and e.t > anchor
                    and find_item(e.data["payload"], COFFIN_ITEM_ID) is None,
                    timeout=PLACE_ATTEMPT_TIMEOUT,
                    description=(
                        f"coffin_place@{cpos} 成功应消耗 {COFFIN_ITEM_ID}"
                        "（coffin_place_consumed）——放置必须恰好消耗一个实例，不得残留"
                    ),
                )
                placed_at = cpos
                break
            except BotAssertionError:
                continue
        assert placed_at is not None, (
            "coffin_place 在所有候选格均被拒（出生点地形全占/异常），无法完成放置 leg"
        )

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
            and (
                (find_item(e.data["payload"], "ling_mu_ban") or {}).get("item", {}).get(
                    "stack_count"
                )
                == RECLAIM_LING_MU_BAN
            )
            and (
                (find_item(e.data["payload"], "ling_mu_gun") or {}).get("item", {}).get(
                    "stack_count"
                )
                == RECLAIM_LING_MU_GUN
            ),
            timeout=STAGE_TIMEOUT,
            description=(
                f"回收应精确返还 ling_mu_ban×{RECLAIM_LING_MU_BAN} + "
                f"ling_mu_gun×{RECLAIM_LING_MU_GUN}（coffin.mundane_coffin Reclaim "
                "全量返还，coffin_menu_reclaimed 快照）"
            ),
        )
        reclaim_snapshot = reclaim_event.data["payload"]

        ban = require_item(reclaim_snapshot, "ling_mu_ban")
        gun = require_item(reclaim_snapshot, "ling_mu_gun")
        assert int(ban["item"]["stack_count"]) == RECLAIM_LING_MU_BAN, (
            f"回收返料 ling_mu_ban 应恰好 ×{RECLAIM_LING_MU_BAN}，"
            f"实际 stack_count={ban['item']['stack_count']}（多即双授、少即吞料）"
        )
        assert int(gun["item"]["stack_count"]) == RECLAIM_LING_MU_GUN, (
            f"回收返料 ling_mu_gun 应恰好 ×{RECLAIM_LING_MU_GUN}，"
            f"实际 stack_count={gun['item']['stack_count']}"
        )
        assert ban["location"]["kind"] == "container", (
            f"返料应落入背包容器槽位，实际 location={ban['location']}"
        )
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
        bot.cmd(f"give {PROBE_ITEM_ID} 1")
        post_event = bot.wait_for(
            lambda e: e.kind == "server_data"
            and e.data["payload_type"] == "inventory_snapshot"
            and e.t > anchor
            and find_item(e.data["payload"], PROBE_ITEM_ID) is not None,
            timeout=STAGE_TIMEOUT,
            description=(
                f"probe give {PROBE_ITEM_ID} 应回推 inventory_snapshot"
                "（二次回收被拒时这是窗口内唯一由我们引发的库存事件）"
            ),
        )
        post_snapshot = post_event.data["payload"]

        # 回扫 (anchor, post_event.t] 窗口内的每一个 inventory_snapshot：返料必须
        # 始终 6/2、mundane_coffin 必须始终不出现。窗口内还有周期性 inventory_changed
        # （教程灵鼠扣 qi 等），故不能只查 probe 快照；逐条扫描才能证明二次回收没有
        # 在任何一条快照里推过翻倍返料或棺材本体。
        bad_snapshots = [
            e.data["payload"]
            for e in bot.events
            if e.kind == "server_data"
            and e.data["payload_type"] == "inventory_snapshot"
            and anchor < e.t <= post_event.t
            and (
                ((find_item(e.data["payload"], "ling_mu_ban") or {}).get("item", {}).get(
                    "stack_count"
                ))
                != RECLAIM_LING_MU_BAN
                or ((find_item(e.data["payload"], "ling_mu_gun") or {}).get("item", {}).get(
                    "stack_count"
                ))
                != RECLAIM_LING_MU_GUN
                or find_item(e.data["payload"], COFFIN_ITEM_ID) is not None
            )
        ]
        assert not bad_snapshots, (
            f"二次回收必须被拒（registry 已摘除）：(anchor, probe] 窗口内出现 "
            f"{len(bad_snapshots)} 条违规快照（返料非 6/2 或棺材复现），"
            f"首条={bad_snapshots[0]}——二次回收被误处理会先推 coffin_menu_reclaimed"
            "并在当条快照翻倍返料"
        )
        post_ban = require_item(post_snapshot, "ling_mu_ban")
        post_gun = require_item(post_snapshot, "ling_mu_gun")
        assert int(post_ban["item"]["stack_count"]) == RECLAIM_LING_MU_BAN, (
            f"二次回收后 ling_mu_ban 应仍为 ×{RECLAIM_LING_MU_BAN}（未被双授），"
            f"实际={post_ban['item']['stack_count']}"
        )
        assert int(post_gun["item"]["stack_count"]) == RECLAIM_LING_MU_GUN, (
            f"二次回收后 ling_mu_gun 应仍为 ×{RECLAIM_LING_MU_GUN}（未被双授），"
            f"实际={post_gun['item']['stack_count']}"
        )
        assert find_item(post_snapshot, COFFIN_ITEM_ID) is None, (
            f"二次回收后 {COFFIN_ITEM_ID} 不应出现（放置时已消耗一次，回收不返棺材）"
        )

        bot.assert_alive("coffin_menu_reclaim 全链路后")
