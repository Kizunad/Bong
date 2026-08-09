"""技能配置组：skill_config_intent。

黑盒契约面（server/src/skill/config.rs + client_request_handler.rs）：
- 合法配置 → 校验通过 → 写入 store → 推 `skill_config_snapshot`（configs 含该 skill，
  json_config 为序列化 JSON 字符串）。zhenmai.sever_chain schema：
  meridian_id（MeridianId::ALL）+ backfire_kind（enum [real_yuan, physical_carrier,
  tainted_yuan, array]）。
- 未知 skill → UnknownSkill 拒绝：仍回推权威快照（当前 store，无该 skill 条目）。
- 字段值非法（不在 enum 白名单）→ 拒绝：回推快照中该 skill 配置保持原值。
- 空 config {} → 清配置：回推快照不含该 skill 条目。

拒绝路径不踢线不 panic，且永远以权威 skill_config_snapshot 收尾。
"""

import json

from bot.scenarios._combat_helpers import last_event_time, wait_for_ready

DESCRIPTION = "技能配置：合法写入/未知skill拒绝/非法字段拒绝/空config清空"
MODULES = ["skill"]

SKILL = "zhenmai.sever_chain"
VALID_CONFIG = {
    "meridian_id": "Pericardium",
    "backfire_kind": "tainted_yuan",
}


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


def run(env) -> None:
    with env.new_bot("SkillCfg") as bot:
        wait_for_ready(bot)

        # ── 1. 合法配置写入 → snapshot 含该 skill，json_config 字段齐全 ──
        anchor = last_event_time(bot)  # 锚必须在 intent 前（见 _expect_config_snapshot）
        bot.intent(
            {
                "type": "skill_config_intent",
                "v": 1,
                "skill_id": SKILL,
                "config": VALID_CONFIG,
            }
        )
        written = _expect_config_snapshot(bot, anchor)
        configs = _configs_of(written)
        assert SKILL in configs, f"写入后 snapshot 应含 {SKILL}，实际 {sorted(configs)}"
        parsed = json.loads(configs[SKILL])
        assert parsed.get("meridian_id") == "Pericardium", (
            f"json_config.meridian_id 应为 Pericardium，实际 {parsed!r}"
        )
        assert parsed.get("backfire_kind") == "tainted_yuan", (
            f"json_config.backfire_kind 应为 tainted_yuan，实际 {parsed!r}"
        )

        # ── 2. 未知 skill → 拒绝：回推快照不含该 skill ──
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

        # ── 3. 非法 backfire_kind（enum 白名单外，meridian_id 有效）→ 拒绝：原配置保持 ──
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
        assert parsed.get("backfire_kind") == "tainted_yuan", (
            f"非法 backfire_kind 拒绝后 backfire_kind 应保持 tainted_yuan，实际 {parsed!r}"
        )

        # ── 4. 非法 meridian_id（MeridianId::ALL 之外，backfire_kind 有效）→ 拒绝：原配置保持 ──
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
        assert parsed.get("meridian_id") == "Pericardium", (
            f"非法 meridian_id 拒绝后 meridian_id 应保持 Pericardium，实际 {parsed!r}"
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

        bot.assert_alive("技能配置 4 步正负路径后")
