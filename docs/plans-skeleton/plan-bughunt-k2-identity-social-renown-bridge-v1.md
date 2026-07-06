# plan-bughunt-k2-identity-social-renown-bridge-v1（骨架）

> **骨架（草案）**。一句话主题：K2 bughunt 确认 1 个高置信真 bug——`SocialRenownDeltaEvent` 路径只更新 `Renown` component / `social_renown` 持久化，不回写 `PlayerIdentities.active.renown` / `player_identities`，导致 identity/social 双账本永久分叉。实际游玩影响：结契背盟、宗门背叛、越级全力击杀、战争胜方 fame 等事件之后，玩家当前身份的 NPC 反应、通缉、identity panel、high-renown 江湖传闻仍继续读旧名声。

> 结论：**候选成立，进入 fix_pr**。这不是单点调用漏发，而是 `SocialRenownDeltaEvent -> active identity` 的统一同步桥缺失；`pvp_encounter` 已经存在手工双写特例，反而证明通用事件消费链本身不会补 identity。

## 阶段总览

| 阶段 | 主题 | 路由 | 状态 |
|------|------|------|------|
| P0 | `SocialRenownDeltaEvent` 双账本分叉（server 根因） | fix_pr | ⬜ |
| P1 | identity/social 回归测试补齐（server + 对应 payload） | fix_pr | ⬜ |

## P0 — `SocialRenownDeltaEvent` 只写 social，不写 active identity

- `server/src/social/mod.rs:1373-1412` `apply_social_renown_deltas` 只 query `(&Lifecycle, &mut Renown)`，只调用 `persist_social_renown(...)`，没有任何 `PlayerIdentities` / `identity_db::save_player_identities(...)` 写回。
- `server/src/identity/mod.rs:309-317` 玩家身份在 join 时独立从 `player_identities` 载入；`server/src/social/mod.rs:2900-2926` social 路径独立写 `social_renown`。两套持久化当前没有 merge 点，所以一旦事件只走 social 表，identity 读链会长期陈旧，不是“下一 tick 自愈”。
- identity/social 主路径明确读 active identity：
  - `server/src/identity/reaction.rs:133-150` `update_identity_reaction_state` 按 `active.reputation_score()` 决定 Normal/Low/Wanted。
  - `server/src/identity/wanted_player_emit.rs:30-50` `build_wanted_player_event` 直接取 `active.display_name` / `active.reputation_score()`。
  - `server/src/network/npc_metadata.rs:401-407` NPC 对玩家声名分数走 `PlayerIdentities::active()`。
  - `server/src/network/identity_panel_emit.rs:64-82` identity panel 的 `reputation_score` 来自各 `IdentityProfile`。
  - `server/src/social/high_renown_tracker.rs:67-93` high-renown milestone 只看 `active.renown.fame`。
- 现有生产者里，除 `pvp_encounter` 外，多数都只 emit `SocialRenownDeltaEvent`：
  - `server/src/social/mod.rs:476-483` 杀高 fame 目标仅加 `notoriety_delta=10`。
  - `server/src/social/mod.rs:751-760` 破 pact 仅加“背盟者”。
  - `server/src/social/mod.rs:1566-1572` faction betrayal 仅加 notoriety/tag。
  - `server/src/cultivation/full_power_strike.rs:559-571` 越级全力击杀仅加 fame/tag。
  - `server/src/npc/war/settle.rs:125-143` 战争胜方仅加 fame。
- `server/src/social/pvp_encounter.rs:239-316` 是关键反证：它先手工写 `active.renown` + `save_player_identities(...)`，再 emit 同一条 `SocialRenownDeltaEvent`。如果通用消费链本来会补写 identity，这个特例就没有存在必要。

## P1 — 修复面与回归面

- server 修复方向：
  - 在 `apply_social_renown_deltas` 内补 `PlayerIdentities` 写回，在线玩家更新 `active_mut().renown`，离线/持久化路径更新 `player_identities` 当前 active identity。
  - 保留 `Renown` component / `social_renown` 现有写入，确保 social HUD/remote identity 与 identity/NPC 反应同帧一致。
  - 若未来真要支持“只影响 char-global、不影响 active identity”的名声事件，应显式拆 scope（例如 event 字段或新事件类型），而不是继续依赖当前隐式半同步。
- 回归测试：
  - 为 event-only 生产者补 pin：`full_power_strike_high_realm_kill`、`war_winner_enlist|mercenary`、`pact_broken`、`faction_betrayal`、`pk_death_higher_fame_victim` 触发后，`Renown` 与 `PlayerIdentities.active.renown` 必须同值。
  - `identity_panel_state` 必须跟随 fame/notoriety 变化刷新。
  - `high_renown_milestone` 必须能被 event-only fame 生产者触发，而不是只对 `pvp_encounter` 这种手工双写路径生效。
  - Wanted/Low 阈值跨边界时，`IdentityReactionChangedEvent` 与 `wanted_player` 必须能由 event-only notoriety 生产者触发。

## §N 开放问题

1. 现有 `social_renown` 是否要继续保留 char-global 语义，还是降级为 active-identity 的投影视图；fix_pr 需要先拍板，避免再造第三套账本。
2. 离线玩家收到 `SocialRenownDeltaEvent` 时，是否始终写“当时 active identity”；当前 persistence 结构可做到，但要确认这正符合“切身份洗白”设计。

## 两轮反方裁决摘要

- **Round 1 反方主张**：`Renown` 是 char-global 社交声名，`PlayerIdentities.active.renown` 是身份声名，二者分离是设计而非 bug。**裁决**：不成立。`pvp_encounter` 已手工双写 identity + social；如果“天然分离”是设计，背叛事件不应特判当前 identity。
- **Round 2 反方主张**：即便 identity 不更新，主链也主要读 `Renown` component，玩家体感不会坏。**裁决**：不成立。NPC 反应、wanted emit、npc metadata、identity panel、high-renown milestone 全都直接读 `PlayerIdentities.active`，而且 `player_identities` 与 `social_renown` 分表持久化，重连也不会自愈。

## 审计来源

bughunt 线程 K2，围绕既有候选做两轮对抗复核后保留。范围只落在 `server/src/identity/`、`server/src/social/`、`server/src/network/` 的交叉读写面；本轮仅新增 skeleton，不改实现。
