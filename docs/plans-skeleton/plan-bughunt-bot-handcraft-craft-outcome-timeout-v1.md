# plan-bughunt-bot-handcraft-craft-outcome-timeout-v1（骨架）

> **骨架（草案）**。一句话主题：bot e2e 复用现有 debug server 时，`production_handcraft_stone_knife` 两次稳定复现 `craft_start` 已受理、`craft_session_state` 已回推，但 100 秒内没有 `craft_outcome`，手搓会话疑似 tick/完工结算或回流断链。

> 立项动机：bot 覆盖不应在本轮直接改产品代码。按 bot coverage 收尾规则，live 测试发现产品运行时缺口时先落 skeleton，后续独立 bugfix PR 再处理。

## 阶段总览

| 阶段 | 主题 | 路由 | 状态 |
|------|------|------|------|
| P0 | 复现并定位手搓会话不完工/不回流 | bughunt | ⬜ |
| P1 | 修复 craft session tick / outcome emit / inventory 入包链路 | fix_pr | ⬜ |
| P2 | 恢复 bot e2e 全绿并补回归证据 | coverage | ⬜ |

## P0 — 复现并定位手搓会话不完工/不回流

- 复现命令：在 `origin/main` 临时验证目录运行 `BOT_E2E_REUSE=1 BOT_E2E_RUN_TAG=C55 bash scripts/bot-e2e.sh`，复用已有 `127.0.0.1:25565` debug server。
- 全量结果：22 个 bot 场景中 21 PASS，唯一失败为 `production_handcraft_stone_knife`。
- 单场景复跑：`python3 scripts/bot/run_scenarios.py --scenario production_handcraft_stone_knife --run-tag H2` 仍失败。
- 失败断言：`craft_start` 后收到 `craft_session_state`，但 100 秒内没有 `craft_outcome`。
- server log 证据：
  - `client_request received ... {"type":"craft_start","v":1,"recipe_id":"workbench.weapon.stone_knife"}`
  - `[bong][craft] start ok player=offline:BH2Craft recipe=workbench.weapon.stone_knife ticks=400 quantity=1`
  - 后续 100 秒窗口内持续只有 `inventory_snapshot`，没有 `craft_outcome`。
- 环境限定：本次按用户规则复用既有 server；该 server 来自 `fix-basic-attack-weapon-multiplier` debug worktree，且 worktree 有未提交 combat 改动。P0 需要在干净 `origin/main` server 上确认一次，避免把 debug server TPS/脏状态误判为主线缺陷。

## P1 — 修复 craft session tick / outcome emit / inventory 入包链路

- 若干净复现确认：
  - 检查 `server/src/craft/session.rs` 中会话剩余 tick 是否随 Bevy Update 正常递减。
  - 检查 `server/src/network/craft_emit.rs::emit_craft_outcome_payloads` 是否能看到 Completed/Failed 事件并推送 `ServerDataType::CraftOutcome`。
  - 检查材料扣除后是否因库存 revision、会话 ownership、玩家断连/重连状态导致 outcome 被吞。
  - 确认 `workbench.weapon.stone_knife` station=None 的手搓路径不需要工作台实体，不能卡在 workbench gate。
- 若仅为 debug TPS 过低：
  - 将 bot 场景等待条件改为按 `craft_session_state.remaining_ticks` 进度推进，而不是固定 100 秒 wall-clock。
  - 或给 debug/reuse 模式下的手搓场景单独放宽 timeout，并在日志里打印 tick 进度。

## P2 — 恢复 bot e2e 全绿并补回归证据

- `python3 scripts/bot/test_protocol.py` 继续保持通过。
- `production_handcraft_stone_knife` 必须在干净 server 上通过：`craft_start -> craft_session_state -> craft_outcome(completed) -> inventory_snapshot 包含 stone_knife`。
- 全量 `BOT_E2E_REUSE=1 bash scripts/bot-e2e.sh` 应恢复 22/22 PASS；若环境禁止新起 server，验证报告必须标明复用 server 的 branch/dirty 状态。
- 若修复涉及产品代码，补 server 单测覆盖 handcraft 400 tick 完工、Completed event emit、inventory 入包三段；bot 场景只保留黑盒断言。

## 排重

- 不重复 `plan-bughunt-craft-outcome-network-thread-sound-v1`：该案关注 Fabric network thread 上的客户端完成音效/Store side effect；本案关注 server 侧或 bot live 观察到的 `craft_outcome` 不到达。
- 不重复 `plan-bughunt-client-request-schema-drift.md`：`craft_start` wire 形状已被 server 接收并启动会话，本案发生在受理之后。

## 审计来源

- 2026-07-07 bot coverage 收尾验证。
- bot 脚本来源：`origin/main` 的 `scripts/bot/scenarios/production_handcraft_stone_knife.py`。
- 运行约束：未启动新 server，未杀已有 server，未改产品代码。
