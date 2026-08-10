"""真元色探察 qi_color_inspect（实体/空间探知流，plan 感知 §真元色）。

C2S：`qi_color_inspect { observed: "entity:<protocol_id>" }`——protocol id 来自
S2C PlayerSpawn 包（game_join 的 entity_id 恒为 0，只能经 player_spawn 拿同服
玩家 id；bot.py 已补该包解码）。

服务端链路：
- dispatch（client_request_handler.rs:2128）：`entity:<id>` → EntityManager
  get_by_id → 同维度 + ≤QI_COLOR_INSPECT_MAX_DISTANCE(6.0) 才放行，否则静默丢弃；
- emit（qi_color_observed_emit.rs）：observer/observed 均需 Cultivation，observed
  需 QiColor；`realm_diff = rank(observer) - rank(observed)`，≤0 静默 continue；
  ≥2 全量 payload（main/secondary/is_chaotic/is_hunyuan/realm_diff），==1 脱敏
  （secondary=None、is_chaotic=false、is_hunyuan=false）。

断言面（两 bot 同出生点，相距 <6m）：
1. host 凝脉 + victim 醒灵 → diff=2 全量 payload：main=Mellow（QiColor 默认）、
   secondary 缺省、is_chaotic=false、is_hunyuan=false、realm_diff=2、
   observer/observed 为 offline:<username> canonical id；
2. host 降回醒灵（同境界）→ 静默（realm_diff=0 continue，无 S2C）；
3. 不存在协议 id 的 observed → dispatch 静默丢弃（无 S2C、无聊天）。
"""

import time

from bot.bot import BotAssertionError

from ._inventory_helpers import wait_join_and_inventory

DESCRIPTION = "qi_color_inspect：跨境界全量 payload、同境界静默、坏 target 静默"
MODULES = ["cultivation", "network"]

INSPECT_REQUEST = {"type": "qi_color_inspect", "v": 1}
SILENT_WINDOW = 4.0


def run(env) -> None:
    with env.new_bot("QiH") as host:
        wait_join_and_inventory(host)
        host.cmd("realm set condense")
        host.expect_chat("[dev] realm set ", timeout=10.0)

        with env.new_bot("QiV") as victim:
            wait_join_and_inventory(victim)

            spawn = host.wait_for(
                lambda e: e.kind == "player_spawn",
                timeout=15.0,
                description="host 收到 victim 的 PlayerSpawn（protocol entity id）",
            )
            victim_protocol_id = spawn.data["entity_id"]
            if not isinstance(victim_protocol_id, int) or victim_protocol_id <= 0:
                raise BotAssertionError(
                    f"[{host.username}] 期望 player_spawn 携带正数 protocol entity_id，"
                    f"实际 {victim_protocol_id!r}"
                )
            host.assert_alive("victim 就绪后")

            # 1. 凝脉 host 探醒灵 victim → diff=2 全量 payload
            host.intent({**INSPECT_REQUEST, "observed": f"entity:{victim_protocol_id}"})
            observed = host.expect_server_data("qi_color_observed", timeout=10.0)
            payload = observed.data["payload"]
            _assert_full_payload(host, payload, victim.username)
            host.assert_alive("跨境界 qi_color_inspect 后")

            # 2. host 降回醒灵（同境界）→ realm_diff=0 静默
            host.cmd("realm set awaken")
            host.expect_chat("[dev] realm set ", timeout=10.0)
            sent_at = host.events[-1].t if host.events else 0.0
            host.intent({**INSPECT_REQUEST, "observed": f"entity:{victim_protocol_id}"})
            _assert_silent(host, sent_at, "同境界 qi_color_inspect 应静默（realm_diff=0 continue）")

            # 3. 不存在的协议 id → dispatch 静默丢弃
            sent_at = host.events[-1].t if host.events else 0.0
            host.intent({**INSPECT_REQUEST, "observed": "entity:999999999"})
            _assert_silent(host, sent_at, "坏 observed 协议 id 应被 dispatch 静默丢弃")

            victim.assert_alive("qi_color_inspect 全程 victim")
            host.assert_alive("qi_color_inspect 拒绝面全程")


def _assert_full_payload(host, payload: dict, victim_username: str) -> None:
    if payload.get("main") != "Mellow":
        raise BotAssertionError(
            f"[{host.username}] 期望 QiColor.main=Mellow（默认真元色），实际 {payload.get('main')}"
        )
    if payload.get("realm_diff") != 2:
        raise BotAssertionError(
            f"[{host.username}] 期望 realm_diff=2（凝脉-醒灵），实际 {payload.get('realm_diff')}"
        )
    if payload.get("is_chaotic") is not False or payload.get("is_hunyuan") is not False:
        raise BotAssertionError(
            f"[{host.username}] 期望 is_chaotic/is_hunyuan=false，实际 {payload}"
        )
    if "secondary" in payload and payload["secondary"] is not None:
        raise BotAssertionError(
            f"[{host.username}] 期望 secondary 缺省/None，实际 {payload.get('secondary')}"
        )
    if payload.get("observed") != f"offline:{victim_username}":
        raise BotAssertionError(
            f"[{host.username}] 期望 observed=offline:{victim_username}，"
            f"实际 {payload.get('observed')}"
        )
    if not payload.get("observer", "").startswith("offline:"):
        raise BotAssertionError(
            f"[{host.username}] 期望 observer 为 offline:<user> canonical id，"
            f"实际 {payload.get('observer')}"
        )


def _assert_silent(bot, sent_at: float, description: str) -> None:
    end_at = sent_at + SILENT_WINDOW
    while True:
        now = bot.events[-1].t if bot.events else 0.0
        for e in bot.events_of("server_data"):
            if e.t > sent_at and e.data["payload_type"] == "qi_color_observed":
                raise BotAssertionError(
                    f"[{bot.username}] {description}，实际收到 qi_color_observed（t={e.t:.3f}）"
                )
        for e in bot.events_of("chat"):
            # dev 修为切换的确认回显可能晚于 sent_at 到达（实测出现在窗口内），
            # 属场景 setup 噪音；静默断言只看 qi_color_observed 与真实新聊天。
            if e.t > sent_at and "realm set" not in e.data.get("text", ""):
                raise BotAssertionError(
                    f"[{bot.username}] {description}，实际出现聊天 {e.data['text']!r}"
                )
        if now >= end_at:
            return
        time.sleep(0.1)
