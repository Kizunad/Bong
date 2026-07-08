# plan-bughunt-bot-multibot-chat-visibility-v1（骨架）

> **骨架（草案）**。一句话主题：Bot e2e 在同一 server 上连接两个真实协议 Bot 时，Alice 发出的 chat 未稳定广播到 Bob 的协议观察流。PR #982 只做 bot 覆盖，不修 server/client/agent 业务代码，因此先移除会红断言并记录缺口。

## 阶段总览

| 阶段 | 主题 | 路由 | 状态 |
|------|------|------|------|
| P0 | 多 bot chat 广播缺口复现与协议定界 | bughunt | ⬜ |
| P1 | chat 广播产品/协议修复方案 | fix_pr | ⬜ |
| P2 | Bot e2e chat visibility 回归恢复 | coverage | ⬜ |

## P0 — 多 bot chat 广播缺口复现与协议定界

- 失败现象：`multibot_chat_visibility` 中 Alice 发送 `bot-e2e-chat-ci` 后，Bob 10 秒内没有收到同文本 chat；Bob 已收到大量其他协议事件，说明连接本身存活。
- 黑盒边界：Bot 只走真实 C2S chat 包和 S2C chat/game message 解码，不读 server 内部状态。
- 当前处置：PR #982 中多 bot 场景降级为两个 Bot 均完成 `game_join` / `pos_look` 且连接保持。

## P1 — chat 广播产品/协议修复方案

- 核对 server chat collector / broadcast 只写 agent/Redis 还是也应回流玩家客户端。
- 核对协议 Bot 的 `C2S_CHAT_MESSAGE` 与 server 期望字段是否完全匹配；若只是 bot packet 形状问题，修 bot helper。
- 若是产品语义缺口，另开 fix PR 修 server/client，不在 coverage PR 中混入业务修复。

## P2 — Bot e2e chat visibility 回归恢复

- 修复后恢复 `A.chat(marker)` → `B.expect_chat(marker)` 的黑盒断言。
- 保留基础双连接场景，chat visibility 可独立成强断言场景。
