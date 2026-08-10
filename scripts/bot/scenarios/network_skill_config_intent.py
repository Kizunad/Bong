"""技能配置组：skill_config_intent。

黑盒契约面（server/src/skill/config.rs + client_request_handler.rs）：
- 合法配置 → 校验通过 → 写入 store → 推 `skill_config_snapshot`（configs 含该 skill，
  json_config 为序列化 JSON 字符串）。zhenmai.sever_chain schema：
  meridian_id（MeridianId::ALL，20 个合法值）+ backfire_kind（enum [real_yuan,
  physical_carrier, tainted_yuan, array]）。
- 未知 skill → UnknownSkill 拒绝：仍回推权威快照（当前 store，无该 skill 条目）。
- 字段值非法（不在 enum 白名单）→ 拒绝：回推快照中该 skill 配置保持原值。
- 空 config {} → 清配置：回推快照不含该 skill 条目。

拒绝路径不踢线不 panic，且永远以权威 skill_config_snapshot 收尾。
"""

import json

from bot.scenarios._combat_helpers import last_event_time, wait_for_ready

DESCRIPTION = "技能配置：合法写入穷举/未知skill拒绝/非法字段拒绝保持全量配置/空config清空"
MODULES = ["skill"]

SKILL = "zhenmai.sever_chain"
BACKFIRE_KINDS = ["real_yuan", "physical_carrier", "tainted_yuan", "array"]
# MeridianId::ALL（server/src/cultivation/components.rs，serde camelCase 序列化）。
MERIDIANS = [
    "Lung",
    "LargeIntestine",
    "Stomach",
    "Spleen",
    "Heart",
    "SmallIntestine",
    "Bladder",
    "Kidney",
    "Pericardium",
    "TripleEnergizer",
    "Gallbladder",
    "Liver",
    "Ren",
    "Du",
    "Chong",
    "Dai",
    "YinQiao",
    "YangQiao",
    "YinWei",
    "YangWei",
]


def _expect_config_snapshot(bot, anchor_t: float, timeout: float = 10.0) -> dict:
    """锚定到本次 intent 之后的 skill_config_snapshot，避开 join 时的空快照。

    `anchor_t` 必须在 `bot.intent(...)` 之前取：锚后取会把快服务器已发出并记录
    的权威快照排除掉，导致 intent 成功后场景仍超时（central-review 2012 #1）。
    """
    event = bot.wait_for(
        lambda e: (
            e.kind == "server_data"
            and e.data.get("payload_type") == "skill_config_snapshot"
            and e.t > anchor_t
        ),
        timeout=timeout,
        description=f"skill_config_snapshot（t>{anchor_t:.2f}，当前 intent 响应）",
    )
    return event.data["payload"]


def _configs_of(payload: dict) -> dict[str, str]:
    configs = payload.get("configs", [])
    assert isinstance(configs, list), (
        f"skill_config_snapshot.configs 应为 list，实际 {configs!r}"
    )
    return {entry["skill_id"]: entry.get("json_config", "") for entry in configs}


def _write_config(bot, config: dict) -> dict:
    """下发 skill_config_intent 并返回该 intent 回推的权威快照。"""
    anchor = last_event_time(bot)
    bot.intent(
        {
            "type": "skill_config_intent",
            "v": 1,
            "skill_id": SKILL,
            "config": config,
        }
    )
    return _expect_config_snapshot(bot, anchor)


def run(env) -> None:
    with env.new_bot("SkillCfg") as bot:
        wait_for_ready(bot)

        # ── 1. 合法配置写入：穷举全部合法 meridian × backfire_kind 组合 ──
        # central-review 2012 #5：只测 (Pericardium, tainted_yuan) 会让「仅硬编码接受
        # 这两个值、生产拒掉其余合法 enum 成员/经脉」的错误校验也通过。schema 允许
        # MeridianId::ALL(20) × backfire_kind[4]，全部 80 组合必须逐个写入成功且
        # 回读精确一致（json_config 是 BTreeMap 序列化，dict 等值不受键序影响）。
        last_config: dict = {}
        for meridian in MERIDIANS:
            for kind in BACKFIRE_KINDS:
                config = {"meridian_id": meridian, "backfire_kind": kind}
                last_config = config
                written = _write_config(bot, config)
                configs = _configs_of(written)
                assert SKILL in configs, (
                    f"合法组合 ({meridian}, {kind}) 应写入，实际 {sorted(configs)}"
                )
                parsed = json.loads(configs[SKILL])
                assert parsed == config, (
                    f"写入后 json_config 应精确等于 {config}，实际 {parsed!r}"
                )

        # ── 2. 未知 skill → 拒绝：回推快照不含该 skill + 既有配置守恒 ──
        anchor = last_event_time(bot)
        bot.intent(
            {
                "type": "skill_config_intent",
                "v": 1,
                "skill_id": "no_such_skill_xyz",
                "config": {"whatever": "value"},
            }
        )
        rejected = _expect_config_snapshot(bot, anchor)
        configs = _configs_of(rejected)
        assert "no_such_skill_xyz" not in configs, (
            f"未知 skill 拒绝后 snapshot 不应含 no_such_skill_xyz，实际 {sorted(configs)}"
        )
        # review finding [3]：旧断言只查「未知 skill 不在」，拒绝路径若把整个配置仓
        # 清掉（或抹掉既有 zhenmai.sever_chain），第 2 步也照过——而第 3 步立刻重写
        # 该条目，把清仓错误掩盖掉。必须守恒：拒绝后既有配置保持第 1 步最后一次
        # 写入的精确值（未知 skill 拒绝回推的是权威当前 store，server
        # client_request_handler.rs handle_config_intent Err 分支 snapshot_for_player）。
        assert SKILL in configs, (
            f"未知 skill 拒绝后既有 {SKILL} 配置应保持，实际 {sorted(configs)}"
        )
        parsed = json.loads(configs[SKILL])
        assert parsed == last_config, (
            f"未知 skill 拒绝后 {SKILL} 应保持第 1 步最后写入 {last_config}，"
            f"实际 {parsed!r}"
        )

        # ── 3. 非法 backfire_kind（enum 白名单外，meridian_id 有效）→ 拒绝：完整配置保持 ──
        # central-review 2012 #8：拒绝后不得只盯「被非法化的那个字段」——必须断言
        # 拒绝前后**完整配置**逐字段一致，否则「拒非法值的同时悄悄改了另一字段」
        # 的错误实现也通过。
        baseline = {"meridian_id": "Pericardium", "backfire_kind": "tainted_yuan"}
        written = _write_config(bot, baseline)
        assert json.loads(_configs_of(written)[SKILL]) == baseline, (
            f"基线写入后 json_config 应等于 {baseline}"
        )
        anchor = last_event_time(bot)
        bot.intent(
            {
                "type": "skill_config_intent",
                "v": 1,
                "skill_id": SKILL,
                "config": {"meridian_id": "Pericardium", "backfire_kind": "bogus_kind"},
            }
        )
        kept = _expect_config_snapshot(bot, anchor)
        configs = _configs_of(kept)
        assert SKILL in configs, (
            f"非法字段拒绝后 {SKILL} 配置应保持，实际 {sorted(configs)}"
        )
        parsed = json.loads(configs[SKILL])
        assert parsed == baseline, (
            f"非法 backfire_kind 拒绝后完整配置应保持 {baseline}，实际 {parsed!r}"
        )

        # ── 4. 非法 meridian_id（MeridianId::ALL 之外，backfire_kind 有效）→ 拒绝：完整配置保持 ──
        anchor = last_event_time(bot)
        bot.intent(
            {
                "type": "skill_config_intent",
                "v": 1,
                "skill_id": SKILL,
                "config": {"meridian_id": "bogus_meridian", "backfire_kind": "tainted_yuan"},
            }
        )
        kept = _expect_config_snapshot(bot, anchor)
        configs = _configs_of(kept)
        assert SKILL in configs, (
            f"非法 meridian_id 拒绝后 {SKILL} 配置应保持，实际 {sorted(configs)}"
        )
        parsed = json.loads(configs[SKILL])
        assert parsed == baseline, (
            f"非法 meridian_id 拒绝后完整配置应保持 {baseline}，实际 {parsed!r}"
        )

        # ── 5. 空 config → 清配置：回推快照不含该 skill ──
        anchor = last_event_time(bot)
        bot.intent(
            {
                "type": "skill_config_intent",
                "v": 1,
                "skill_id": SKILL,
                "config": {},
            }
        )
        cleared = _expect_config_snapshot(bot, anchor)
        configs = _configs_of(cleared)
        assert SKILL not in configs, (
            f"空 config 清空后 snapshot 不应含 {SKILL}，实际 {sorted(configs)}"
        )

        bot.assert_alive("技能配置穷举 + 4 步正负路径后")
