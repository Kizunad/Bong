# plan-bughunt-social-runtime-bridge-gap-v1（骨架）

> **骨架（草案）**。一句话主题：`server/src/social/mod.rs` 的 social runtime bridge 只把 `SocialExposure` / 灵龛 / 切磋 / 交易下发给在线 client，却把**活跃 gameplay 真会产出的** `SocialRelationshipEvent(Feud)` 与 `SocialRenownDeltaEvent` 仅写本地状态 + Redis，导致多人 PvP 背叛、派系背盟、越级全力击杀等社交后果**在 server 已生效、agent 已可叙事、client 却完全收不到 `social_feud` / `social_renown_delta`**。

## 结论

- **类型**：server runtime / gameplay bridge 缺口
- **优先级**：major
- **是否与已知题重复**：已对照用户列出的禁区与现有 `docs/plans-skeleton` / `docs/finished_plans`，**非**“玩家交易误套 NPC 门禁 / witness-zone 维度失明 / 第 4 个 EmergentGroupId 不发 FactionReputationDeltaEvent / combat_event 最小字段桥 / SilentSignalSystem 无 runtime/HUD 桥接”同题
- **退化处理说明**：本会话未开 subagent；两轮反方裁决由当前会话独立完成，论点与驳回理由见文末

## 复现路径

### 路径 A：多人 PvP 背叛 / 死斗

1. 两名在线玩家发生 `PvpEncounterEvent`，结果为 `ProbeFight` / `DeathFight` / `Betrayal`。
2. `server/src/social/pvp_encounter.rs:185-210` 的 `handle_pvp_encounter_events` 进入 `emit_social_edges`。
3. `emit_social_edges` 在 `:284-319`：
   - 对上述战斗结果恒发 `SocialRelationshipEvent { left_kind/right_kind = Feud }`。
   - 若 `npc_witnessed=true`，再发 `SocialRenownDeltaEvent { reason = "pvp_betrayal" }`。
4. `server/src/social/mod.rs:225-242` 里这些事件会被：
   - `apply_social_relationships` / `apply_social_renown_deltas` 消费，写入 server 状态与持久化。
   - `publish_social_events` 消费，桥到 Redis。
5. 但在线 client **永远收不到** `social_feud` / `social_renown_delta`：
   - `apply_social_exposures` 在 `server/src/social/mod.rs:508-600` 明确序列化并 `send_server_data_payload` 给相关玩家。
   - `emit_niche_defense_server_data` 在 `:603-686` 明确给灵龛相关玩家发 payload。
   - `dispatch_sparring_invites` / `dispatch_trade_offers` 在 `:779-843`、`:883-1014` 也各自发 payload。
   - **唯独没有任何 `ServerDataPayloadV1::SocialFeud` / `SocialRenownDelta` 的 runtime 发送点。**

### 路径 B：单人但活跃 gameplay 的声名变化

1. 玩家发生派系背盟，`server/src/social/mod.rs:1550-1573` `apply_faction_membership_decisions` 会 emit `SocialRenownDeltaEvent { reason = "faction_betrayal" }`。
2. 或玩家用全力击杀高境目标，`server/src/cultivation/full_power_strike.rs:528-570` 会 emit `SocialRenownDeltaEvent { reason = "full_power_high_realm_kill" }`。
3. server 内部 renown/persistence 会更新，Redis 也会广播。
4. 但本地玩家 client 依旧收不到 `social_renown_delta`，因此不会进 social event stream / SocialStateStore。

## 根因链路

### 1. server 已注册并消费关系/声名事件，但下发分支缺失

- `server/src/social/mod.rs:176-187` 注册了：
  - `SocialRenownDeltaEvent`
  - `SocialRelationshipEvent`
- 同文件 `:225-242` 把它们接进正式 Update 链：
  - `apply_social_relationships`
  - `apply_social_renown_deltas`
  - `publish_social_events`
- 这说明它们**不是 dead code，不是未来占位**，而是活跃 runtime 事件。

### 2. client 端完整准备好了 social payload 入口

- `client/src/main/java/com/bong/client/network/ServerDataRouter.java:213-222` 明确把以下类型交给 `SocialServerDataHandler`：
  - `social_pact`
  - `social_feud`
  - `social_renown_delta`
- `client/src/main/java/com/bong/client/network/SocialServerDataHandler.java:25-35` 注册了对应分支。
- 同文件：
  - `:89-116` `handlePact` 会写 `SocialStateStore.relationships` + `UnifiedEventStore`
  - `:119-145` `handleFeud` 会写 `SocialStateStore.relationships` + `UnifiedEventStore`
  - `:148-178` `handleRenownDelta` 会写 `SocialStateStore.renownDeltas` + `UnifiedEventStore`

### 3. 真正的缺口在 server runtime bridge，只发 Redis 不发在线 client

- `server/src/social/mod.rs:3102-3177` `publish_social_events`：
  - `SocialRelationshipEvent(Feud)` → `RedisOutbound::SocialFeud`
  - `SocialRelationshipEvent(Pact)` → `RedisOutbound::SocialPact`
  - `SocialRenownDeltaEvent` → `RedisOutbound::SocialRenownDelta`
- 但全仓对 `ServerDataPayloadV1::SocialFeud` / `SocialPact` / `SocialRenownDelta` 的非 schema 命中为 **0**；实际没有任何 runtime `send_server_data_payload` 分支。
- 结果是：**agent 能看到，client handler 也准备好了，但在线玩家看不到。**

## 为什么这是 bug，不是设计

1. **同模块内部已形成对照组**：`SocialExposure`、灵龛入侵、切磋邀请、交易邀请都同时有 server→client runtime 下发；只有关系/声名断在 Redis 侧，属于明显分支遗漏。
2. **client 不是没有消费方**：`SocialServerDataHandler` 与 `SocialStateStore`、`UnifiedEventStore` 都已落地，不是“未来规划中的 schema 占位”。
3. **没有 triage / defer 记录**：`server/src/test_coverage_guards.rs` 只 triage 了“无 reader 的事件”；本题不是“事件无人消费”，而是**活跃事件的 client bridge 缺口**。仓内未见“social_feud/social_renown_delta 只给 agent、不下发 client”的 defer 注记。

## 影响面

- **多人遭遇链路割裂**：PvP 背叛或死斗后，server 已经把双方记成死仇、把背叛者 notoriety 写入持久化，但当事玩家/目击玩家客户端**没有任何 social event stream 提示**。
- **派系与高光战斗反馈缺失**：派系背盟、越级全力击杀等会改变声名标签与 notoriety/fame；这些变化不会进入 client `SocialStateStore.renownDeltas`，玩家看不到即时反馈。
- **agent / 玩家感知不一致**：同一事件 agent 侧可基于 Redis 做 political narration，client 侧自己却没有对应的 social runtime 记录，造成“世界在传，你本人 HUD 没反应”的错位。

## 修复建议

1. 在 `server/src/social/mod.rs` 增补一个与 `apply_social_exposures` / `emit_niche_defense_server_data` 对称的 runtime emit 系统。
2. 最小可行做法：
   - `EventReader<SocialRelationshipEvent>`：对 `Feud` 与 `Pact` 分别构造 `ServerDataPayloadV1::SocialFeud` / `SocialPact`，下发给相关玩家。
   - `EventReader<SocialRenownDeltaEvent>`：构造 `ServerDataPayloadV1::SocialRenownDelta`，至少下发给 `char_id` 对应在线玩家；若设计需要，也可下发给相关 witness / 同 zone 观察者。
3. 发送范围建议先按最保守闭环：
   - `social_feud`：双方当事人。
   - `social_renown_delta`：声名变化所属玩家本人。
   - `social_pact`：缔约/解约双方。
4. 回归时顺手加 pin，防止以后再出现“Redis 有、client 没有”的双桥分叉。

## 验收抓手

1. **PvP 背叛链**：构造 `PvpEncounterEvent { outcome=Betrayal, npc_witnessed=true }`，断言：
   - server 仍写入 `Relationships` / `Renown`
   - 在线 client 收到 `social_feud`
   - 背叛者 client 收到 `social_renown_delta`
2. **派系背盟链**：构造 `FactionMembershipDecisionEvent::Betray`，断言在线玩家 client 收到 `social_renown_delta`。
3. **非回归**：`social_exposure` / `trade_offer` / `sparring_invite` 既有下发不受影响。
4. **桥接一致性审计**：对 social 模块做一组“事件既发 Redis 又发 client”的表驱动测试，至少覆盖 `exposure / feud / pact / renown / niche_intrusion`。

## 两轮反方裁决（退化：本会话独立完成）

### Round 1 反方论点

- **论点**：`social_feud` / `social_renown_delta` 的 client handler 可能只是预留；真正设计上只想给 agent，不想给玩家。

**驳回理由**：

- `ServerDataRouter.java:213-222` 的注释直接写“匿名、暴露、关系、声名、切磋邀请”；
- `SocialServerDataHandler` 不只是空壳，而是确实写 `SocialStateStore` 并发 `UnifiedEventStore`；
- 同模块其他 social 事件都已 server→client 下发，没有任何文档声明这三类只走 agent。

### Round 2 反方论点

- **论点**：即便 client 没有 `social_feud` / `social_renown_delta`，玩家也能从持久化状态、identity 面板或 political narration 间接感知，不构成真实 bug。

**驳回理由**：

- `SocialStateStore.renownDeltas()` / `relationships()` 只靠对应 payload 更新，当前 runtime 永远不写；
- political narration 是 agent 侧节流/去重后的二次产物，不保证每次触发、也不保证只对当事人可见；
- “即时 social 结果”与“后续叙事转述”不是一回事。server 已把事件定义为独立 payload 并在 client 做了即时展示处理，runtime 不下发就是闭环断裂。

## 建议 PR 路由

- `plan_skeleton`
- 题名建议：`social runtime bridge 漏发 feud/renown client payload`
