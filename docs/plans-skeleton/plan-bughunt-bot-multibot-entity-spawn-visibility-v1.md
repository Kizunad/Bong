# plan-bughunt-bot-multibot-entity-spawn-visibility-v1（骨架）

> **骨架（草案）**。一句话主题：Bot e2e 在同一 server 上连接两个真实协议 Bot 时，当前 `entity_spawn` 互见观察不稳定：Alice 在 12 秒内未收到 Bob 坐标附近的 `entity_spawn`。本题只记录黑盒可观测缺口，不在 bot 覆盖 PR 中修 server/client/agent 业务代码。

## 阶段总览

| 阶段 | 主题 | 路由 | 状态 |
|------|------|------|------|
| P0 | 多 bot 玩家实体互见缺口复现与定界 | bughunt | ⬜ |
| P1 | 协议包/可见性广播契约修复方案 | fix_pr | ⬜ |
| P2 | Bot e2e entity visibility 回归恢复 | coverage | ⬜ |

## P0 — 多 bot 玩家实体互见缺口复现与定界

- 复现场景：`scripts/bot/scenarios/multibot_chat_visibility.py` 曾尝试两个真实协议 Bot 同服互断 `entity_spawn`。
- 失败现象：Alice 等待 Bob `pos_look` 坐标附近 `entity_spawn` 12 秒超时；chat 广播链路仍可观测。
- 黑盒边界：Bot 只读真实 S2C 包和 chat/payload，不读 server 内部 ECS 状态。
- 初步判断：当前协议 bot / server 可见性广播链路下，玩家实体互见不能作为稳定 CI 断言。

## P1 — 协议包/可见性广播契约修复方案

- 核对 Valence 1.20.1 玩家实体 spawn 包、player info 包、tracking 半径、join 顺序与 bot 是否缺少必要 client settings。
- 定界这是 server 产品缺口、bot 协议解码缺口，还是需要先等待 chunk/player-info 的时序问题。
- 若需修业务代码，应另开 fix PR；本 bot 覆盖 PR 不修改 server/client/agent。

## P2 — Bot e2e entity visibility 回归恢复

- 修复后把 `multibot_chat_visibility.py` 或新增 `multibot_entity_visibility.py` 恢复为稳定实体可见性断言。
- 断言仍保持黑盒：只接受 `entity_spawn` 或等价真实协议可见性事件，不读取 server 内部状态。
- 保留 chat 广播场景作为多连接基础回归，entity visibility 作为独立强断言。

## 当前临时处置

- PR #982 中多 bot 场景退回稳定 chat 广播覆盖，避免 CI 因不稳定 `entity_spawn` 断言必红。
- 本骨架记录缺口，供后续 bughunt/fix 切片展开。
