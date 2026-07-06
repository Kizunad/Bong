# plan-social-renown-identity-bridge-v1（骨架）

> **骨架（草案）**。一句话主题：`SocialRenownDeltaEvent` 现在只写 live `Renown` + SQLite `social_renown`，**不回写 `PlayerIdentities.active.renown` 与 `player_identities`**；结果是 `player_state.social` / `social_renown_delta` / CultivationScreen 看到的是**新名声**，但 `identity_panel_state` / `IdentityReactionChangedEvent` / `bong:wanted_player` / `npc_metadata` / 交易门禁继续读**旧身份名声**。这是 server→schema→client-state/social-hud 主路径上的一条高置信断链，且重连后不会自愈。

> **这个 bug 对实际游玩体验的影响**：玩家在背盟、宗门背叛、越级全力击杀、战争胜利后，修炼面板里的 fame/notoriety 会立刻变化，但 NPC 仍按旧身份分数交易/追杀，Wanted 不触发，身份面板继续停留在旧 reputation；下线重连后依然分裂，因为 `social_renown` 与 `player_identities` 是两张独立持久化表。

## 阶段总览

| 阶段 | 主题 | 路由 | 状态 |
|------|------|------|------|
| P0 | SocialRenown → ActiveIdentity 写侧桥接补齐 | fix_pr | ⬜ |
| P1 | 身份反应 / Wanted / client state 一致性回归 | fix_pr | ⬜ |

## P0 — SocialRenown → ActiveIdentity 写侧桥接补齐

- **高置信 bug（fix_pr）**：`server/src/social/mod.rs:1373-1418` `apply_social_renown_deltas` 读取 `SocialRenownDeltaEvent` 后，只会：
  - 更新 live `Renown` component；
  - 持久化 SQLite `social_renown`；
  - **不会**更新 live `PlayerIdentities.active().renown`；
  - **不会**持久化 SQLite `player_identities`。
- 断链后果已形成双轨：
  - `server/src/network/mod.rs:1676-1725` `build_player_social_snapshot` 读的是 `Renown`，所以 `player_state.social.renown` 永远跟着最新 delta 走。
  - `server/src/network/identity_panel_emit.rs:64-83` `build_identity_panel_state` 读的是 `IdentityProfile.reputation_score()`；`server/src/identity/reaction.rs:119-147` Wanted/Low/High 分级也读 `PlayerIdentities.active()`；`server/src/identity/wanted_player_emit.rs:30-50` 再把这份旧分数发给 agent。
  - `server/src/network/npc_metadata.rs:394-408` 与 `server/src/network/client_request_handler.rs:9922-9956` 的 NPC 好感/交易门禁回退路径同样读 `PlayerIdentities.active().reputation_score()`。
- 持久化不会自愈：
  - `server/src/social/mod.rs:1378-1396` 只写 `social_renown`；
  - `server/src/persistence/identity.rs:56-140` 的 `player_identities` 只在 `identity/command.rs`、`identity/revealed.rs`、`social/pvp_encounter.rs` 被显式保存；
  - `server/src/social/mod.rs:263-307` 与 `server/src/identity/mod.rs:292-329` 登录时分别从两张表独立 hydrate，故重连后仍保持分裂。
- 现有生产者里，除 `server/src/social/pvp_encounter.rs:243-245` 会手动补写 active identity 外，其余 `SocialRenownDeltaEvent` 生产者都走断链：
  - `server/src/social/mod.rs:476-482` `pk_death_higher_fame_victim`
  - `server/src/social/mod.rs:751-757` `pact_broken`
  - `server/src/social/mod.rs:1566-1572` `faction_betrayal`
  - `server/src/npc/war/settle.rs:131-143` `war_winner_*`
  - `server/src/cultivation/full_power_strike.rs:556-568` `full_power_strike_high_realm_kill`
- 修复目标：
  - 在 `apply_social_renown_deltas` 内把 delta 同步写入 live `PlayerIdentities.active_mut().renown`；
  - 对在线玩家同步落盘 `player_identities`；
  - 对离线角色决定如何补写 `player_identities`（见开放问题 #1），避免继续制造 `social_renown` / `player_identities` 双写分裂。

## P1 — 身份反应 / Wanted / client state 一致性回归

- 回归锚点必须同时覆盖 server、schema、client-state 三条读链，证明“同一笔 renown delta 不再分叉”：
  - `server/src/social/mod.rs`：新增 regression，命中至少一条**未手动补写身份**的生产者（建议 `pact_broken` 或 `faction_betrayal`），断言 `Renown` 与 `PlayerIdentities.active().renown` 同步增长。
  - `server/src/identity/reaction.rs` / `server/src/identity/wanted_player_emit.rs`：同一笔 delta 让 active identity 跨到 Wanted 时，`IdentityReactionChangedEvent` 与 `bong:wanted_player` 应真正触发，而不是继续停在旧 tier。
  - `server/src/network/identity_panel_emit.rs` + `server/src/network/mod.rs`：同 tick 下 `player_state.social.renown` 与 `identity_panel_state.identities[*].reputation_score` 不再相互矛盾。
  - 必要时补一条 reconnect persistence regression：证明写入后 `load_social_components` 与 `load_player_identities` 看到的是同一套 active identity 名声。
- client 侧无需新功能，但需要以现有消费路径验证影响面：
  - `client/src/main/java/com/bong/client/network/PlayerStateHandler.java:109-131` 继续读取 `social.renown`；
  - `client/src/main/java/com/bong/client/network/IdentityPanelStateHandler.java:63-80` 继续读取 `reputation_score`；
  - `client/src/main/java/com/bong/client/ui/CultivationScreen.java:88-90` 与 `client/src/main/java/com/bong/client/identity/IdentityPanelStateStore.java` 不应再承载两份互相冲突的“当前名声”。

## §N 开放问题

1. **离线角色 delta 如何补 `player_identities`**：若 `SocialRenownDeltaEvent` 命中离线角色且缺少 `player_identities` 行，是跳过并记日志，还是创建默认 identity 后补写？建议优先“仅在已有 identity 档案时补写；缺档案只写 `social_renown` + 明确日志”，避免后台事件替玩家隐式造身份。
2. **桥接 helper 放置点**：是把“active identity 应用 renown delta + save_player_identities”抽到 `identity` 模块复用，还是先在 `apply_social_renown_deltas` 局部补齐？建议先抽 helper，防未来新增 renown 生产者再次只改一半。
3. **回归样例选择**：`pact_broken` / `faction_betrayal` / `war_winner_*` / `full_power_strike_high_realm_kill` 至少覆盖 1 条 notoriety 与 1 条 fame 来源，避免只修负名声路径。

## 两轮反方裁决摘要

1. **反方一**：这也许是“global social renown”与“identity reputation”故意分离，只有 `player_state.social` 该更新，identity/Wanted 不该跟。  
   **裁决**：驳回。`IdentityProfile` 文档与 `reputation_score()` 明确把 identity 名声定义为 NPC 反应 / Wanted 的权威输入；`IdentityPanelStateV1` 还专门携带 `reputation_score` 给 client。现在同一玩家同一时刻出现 `player_state.social` 新值、`identity_panel_state` 旧值，不是产品设计而是双写断链。
2. **反方二**：也许别处有延迟同步，或重连后会从 `social_renown` 回灌到 `player_identities`，所以只是单帧不一致。  
   **裁决**：驳回。全仓 grep 只发现 `save_player_identities` 调用位于 identity 命令、revealed tag 和 `pvp_encounter` 手工补写；登录路径也分别从 `social_renown` 与 `player_identities` 两表独立 hydrate，没有任何回灌逻辑。故这不是瞬时抖动，而是会跨 tick、跨重连长期存在的持久分裂。

## 审计来源

bug-hunt AG（`network/schema/client-state/social-hud` 主路径，限定 worktree：`.worktree/bughunt-loop-20260705-ag`）。主证据链：

- 写侧只改 `Renown`：`server/src/social/mod.rs:1373-1418`
- `player_state.social` 读 `Renown`：`server/src/network/mod.rs:1676-1725`
- `identity_panel_state` / reaction / wanted / NPC metadata 读 `PlayerIdentities.active()`：`server/src/network/identity_panel_emit.rs:64-83`、`server/src/identity/reaction.rs:119-147`、`server/src/identity/wanted_player_emit.rs:30-50`、`server/src/network/npc_metadata.rs:394-408`
- client 两路分别消费：`client/src/main/java/com/bong/client/network/PlayerStateHandler.java:109-131`、`client/src/main/java/com/bong/client/network/IdentityPanelStateHandler.java:63-80`
- 重连不自愈：`server/src/social/mod.rs:263-307`、`server/src/identity/mod.rs:292-329`、`server/src/persistence/identity.rs:56-140`
