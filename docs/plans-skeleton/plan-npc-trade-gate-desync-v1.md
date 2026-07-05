# plan-npc-trade-gate-desync-v1（骨架）

> **骨架（草案）**。一句话主题：`npc/interaction` 主链里的交易入口存在**客户端展示条件与服务端受理条件分叉**。`bong:npc_metadata`/`NpcDialogueScreen` 只用“非 hostile + trade_offers 非空”决定是否展示“看看你有什么好东西”，但服务端 `resolve_npc_engagement_target -> can_trade()` 还额外要求 **当前 zone 的 `FactionReputationTier != Wanted`**。结果是：**玩家在被该地区势力通缉时，仍能看到并打开 NPC 交易界面，点确认后才被服务端以“[NPC] …不做买卖”打回**。

> 立项动机：这是正常游玩可达的交互/UI 断链，不是测试夹具专属分支。问题落点集中在 `server/src/network/npc_metadata.rs`、`server/src/network/client_request_handler.rs`、`client/src/main/java/com/bong/client/npc/`，修复面局部明确，适合先立 skeleton 收口证据与验收抓手，再单独出 fix PR。

## 阶段总览

| 阶段 | 主题 | 路由 | 状态 |
|------|------|------|------|
| P0 | NPC 交易入口 / 服务端门禁不同步 | fix_pr | ⬜ |

## P0 — NPC 交易入口 / 服务端门禁不同步

- **现象**：client 侧 `NpcMetadata.tradeCandidate()` 仅检查 `!hostile() && !tradeOffers.isEmpty()`（`client/src/main/java/com/bong/client/npc/NpcMetadata.java:105-115`）；`NpcDialogueScreen` 只要该条件成立就渲染“看看你有什么好东西”，并在点击后**直接** `setScreen(new NpcTradeScreen(metadata))`（`client/src/main/java/com/bong/client/npc/NpcDialogueScreen.java:101-106`）。`NpcTradeScreen` 里还能继续选择货物并发 `sendNpcTradeRequest(...)`（`client/src/main/java/com/bong/client/npc/NpcTradeScreen.java:181-205`）。
- **服务端真实门禁**：`resolve_npc_engagement_target()` 会按 NPC 所在 zone 读取 `player_faction_reputation.tier_for_zone(zone)`，得到 `faction_reputation_tier`（`server/src/network/client_request_handler.rs:9916-9937`）；`NpcEngagementTarget::can_trade()` 明确要求 `FactionReputationTier != Wanted` 且 `reputation_to_player >= -30`（`server/src/network/client_request_handler.rs:9880-9885`）。真正的 `NpcTradeRequest` 若没过这层，会被服务端直接拒绝并发聊天反馈“§c[NPC] …不做买卖。”（`server/src/network/client_request_handler.rs:1350-1361`）。
- **断链根因**：`build_npc_metadata()` 只拿到 `membership + player_identities + trade_inventory`，用 `reputation_to_player_score_for_client(...)` 计算一个客户端可见分数，并把 `trade_offers` 原样塞进 payload（`server/src/network/npc_metadata.rs:223-240,304-339`）。它**没有** `FactionReputation` / zone 信息，因此 client 永远不知道“该地区对我是 Wanted，虽然 NPC 有货但不能交易”。
- **为什么这是高置信 bug**：仓库已有单测专门锁定“Wanted 玩家会在服务端交易请求阶段被拒绝”，而且前置断言特意确认 `reputation_to_player_score_for_npc_zone(...) >= -30`，说明这不是“单纯 hostile 时不该显示交易”那种老逻辑，而是**Wanted tier 独立于客户端旧分数门槛**（`server/src/network/client_request_handler.rs:4191-4272`）。同时 `FactionReputationTier::Wanted` 阈值是 `score < -50`（`server/src/social/components.rs:316-326`），和 client 侧 `hostile() == reputation_to_player < -30` 不是一套门。
- **对实际游玩体验的影响**：玩家在青云残峰、血谷等有区域势力声望的地点被通缉后，仍会被 UI 误导为“这个 NPC 能交易”。实际流程是：能看到交易选项、能打开交易屏、能看到货物与价格、甚至能点“确认交易”，但最后只收到服务端拒绝。体感上是**交互界面撒谎**，尤其会让玩家误判“是不是骨币不够/按钮坏了/网络卡了”，而不是明确知道“你在该地被通缉，所以这个 NPC 不跟你做买卖”。
- **建议修复范围 / 模块**：优先收口 `server/src/network/npc_metadata.rs` 与 `client/src/main/java/com/bong/client/npc/`。方向二选一并统一：1) 在 metadata payload 里显式下发 `can_trade` / `trade_block_reason`（推荐，避免 client 手抄服务端门）；2) 给 metadata builder 补足 zone faction rep 语义并让 client 与 server 共用同一套门禁判断。无论选哪条，都应避免继续让 `trade_offers` 是否为空充当“可否交易”的替身。
- **验收抓手**：至少补 4 组 pin。1) Wanted 玩家收到 `trade_offers` 时，client 不显示交易入口，或明确显示不可交易原因。 2) 非 Wanted 且有货时，交易入口和交易屏仍能正常打开。 3) zone 声望从 Normal/Low 跌到 Wanted 后，已有 metadata UI 会及时收敛，不再保留旧入口。 4) client/server 一致性测试覆盖“`reputation_to_player >= -30` 但 `FactionReputationTier::Wanted`”这个当前真实出 bug 的分叉点。

## 反方裁决摘要

1. **Round 1 反方假设**：“也许 client 根本拿不到被通缉但仍非 hostile 的组合，所以入口不会误亮。”裁决：被代码与现有单测直接否掉。服务端测试 `npc_trade_request_rejects_wanted_player_through_engagement_wiring` 明确构造了 `score_gate_value >= -30` 但 `tier_for_zone == Wanted` 的前置条件，证明该组合是被正式支持且真实可达的。
2. **Round 2 反方假设**：“也许只是按钮显示错了，但并不会真的把玩家带进假交易流程，影响不大。”裁决：`NpcDialogueScreen` 点击交易后立即本地 `setScreen(new NpcTradeScreen(metadata))`，`NpcTradeScreen` 还能继续列货、选货并发 `NpcTradeRequest`；也就是说玩家不是只看见一个误亮按钮，而是会被完整带进一个**注定失败**的交易 UI 流程，体验层影响成立。

## 开放问题

1. `trade_block_reason` 是否应只覆盖 Wanted，还是顺手把 Low/hostile/无货也统一做成显式原因枚举，彻底消掉 client 侧猜测逻辑？
2. 如果 metadata 改成显式 `can_trade`，`NpcTradeScreen` 是否也要在已有界面内显示“你在该地被通缉”之类的拒绝文案，而不是单纯隐藏按钮，避免玩家误会成 UI 缺货？

## 审计来源

bughunt 线程 O，范围限定为 `npc/interaction/inspect` 相邻主链与直接 network 邻接文件。候选经主代理人工复核，并按“默认怀疑”思路做两轮证伪后保留。当前结论是 **report-only**：先提交 skeleton-only PR，把玩家影响、可达链路、服务端/客户端分叉点、修复面与验收抓手固定下来，再由后续 fix PR 单独收口实现。
