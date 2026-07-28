# plan-bughunt-r2-findings-v1（已归档）

> **归档说明（2026-07-28）**：除本说明与文末 Round bundle triage 外，下列正文完整保留本 plan 在冻结基线 `origin/main @ c625d5a5` 上的原始阶段、决议、测试与审计记录；正文里的 “Active / 骨架 / ⬜ / 开放问题” 是历史状态。当前唯一实施归属以文末 `Finding Mapping` 为准，移交 successor 的条目仍未实施，不因本 bundle 归档而视为完成。


> **Active（已从 skeleton 升级，待逐项消费）**。一句话主题：代码库自检 bug-hunt **round2**（换角度:emit 孤岛/client+agent/境界门控/schema 双端）确认的一批真 bug——含 **1 critical（dying-elder 给丹永久失效）** + 多处 emit 孤岛/schema 双端不齐/守恒/状态机缺陷。**已对 origin/main(63996fbf1) 复核**，剔除 stale/dup。

> 立项动机：worldgen-v4 收官后 bug-hunt round2，24 候选→裁决。**复核说明**：finder 误扫了主仓 stale 工作目录(cb310d093)，已逐项对 origin/main 复核——`NarrationKind 双端不齐` 在 main 已被 29f0c4a62 修复(剔除)；`ServerDataRouter tsy_collapse_started_ipc 重复` 是 round1 已收(plan-bughunt-r1-mechanical-fixes-v1，不重复)。以下均 **real-on-main**。

## 阶段总览（按主题分组，逐项独立可修）

| 阶段 | 主题 | 状态 |
|------|------|------|
| P0 | 🔴 dying-elder 给丹链路修复（ID 命名空间 + sentinel） | ⬜ |
| P1 | emit 孤岛接线（结果/音效事件无 bridge 消费） | ⬜ |
| P2 | proto/schema 双端不齐（HUD/战斗 payload 不可达） | ⬜ |
| P3 | 境界门控 / ID 失配旁路 | ⬜ |
| P4 | 守恒 + 状态机缺陷 | ⬜ |

## P0 — 🔴 dying-elder 给丹链路（critical）

- **#2 critical**：`client/.../dying_elder/DyingElderInteractionKeybindings.java:113` client 把 **ECS entity index** 当 `elder_entity_id` 回发，server `handle_give_dan_to_elder`（`client_request_handler.rs:10855`）用 `entity_manager.get_by_id` 按 **Valence MC 协议 EntityId**（另一命名空间，`valence_entity/manager.rs:42`）查 → 恒 None → **给丹每次静默丢弃，dying-elder 给丹循环永久坏**。S2C `elder_encounter_emit.rs:129/188/240` emit 的是 `entity.index()`（ECS index）。修：双端统一用 MC 协议 EntityId。
- **#3 major**：`DyingElderInteractionKeybindings.java:101 if (elderEntityIdx <= 0) return` 把 0 当未设，但 0 既是 init/clear sentinel（28/179）又是合法 wire 值（120）+ server 测试 `event_v1_entity_idx_zero_is_valid` 证 0 合法 → index 0 的大能被永久挡。修：用 -1 sentinel / nullable 区分未设 vs 合法 0（#2 修后仍需）。

## P1 — emit 孤岛接线

- **#0 major**：`cultivation/dugu.rs:451` emit `AntidoteResultEvent`（schema + payload fn 俱在=有意），但 `dugu_event_bridge.rs` 只读 `DuguPoisonProgressEvent`，无 `RedisOutbound::AntidoteResult` → 解毒成败结果（唯一 client/agent 反馈）被 Bevy 每 tick 清理丢弃。修：加 bridge 消费镜像兄弟 DuguPoisonProgress 路径（机械）。
- **#1 minor**：`lingtian/systems.rs:649` emit `RenewCompleted`，`audio_trigger.rs:496` 读 Till/Planting/Harvest/Replenish/DrainQi 但**漏 RenewCompleted** → renew 完成无音效（视听不全，5 兄弟皆有音效独此无）。修：加 RenewCompleted reader + emit_play_at_block。

## P2 — proto/schema 双端不齐

- **#5 major**：`npc/war/settle.rs:155 broadcast_faction_war_hud` 生产系统发 `FactionWarState`（生产走 to_proto_bytes），但 `client/.../ProtoServerDataBridge.java` 的 `CASE_TO_TYPE` + `extractInner` **无 FACTION_WAR_STATE case** → bridge 返回 error，JSON fallback 解 proto bytes 失败 → `FactionWarHudHandler`（已注册）永不运行，**战争 HUD 永不更新**。修：补 CASE_TO_TYPE + extractInner（机械）。
- **#11 major**：`agent/packages/schema/src/combat-event.ts` `CombatBodyPartV1` **缺 "back" variant**（TS 0 命中），agent 侧产/消该值时双端不齐。修：TS 补 back（连 sample 对拍）。

## P3 — 境界门控 / ID 失配旁路

- **#9 major**：`combat/known_techniques.rs` `bao_mai.*` vs `baomai.*` ID 失配 → 绕过 qi cost + 经脉校验（与 plan-skill-cast-meridian-gate-v1 同源经脉门主题，可并入）。
- **#10 minor**：`combat/.../skills.rs` `pill_rush` cast 的 `< Realm::Awaken` 死守卫（u8 < 0 恒 false，醒灵是最低境）→ 误导性死代码。修：删或改对门槛。

## P4 — 守恒 + 状态机

- **#12 major（守恒，plan_skeleton）**：`combat/dugu_v2_event_bridge.rs` `Dugu returned_zone_qi` 永不归还区域灵气（守恒孤岛）。与 round1 `plan-qi-conservation-leaks-v1` 同主题，可并入或独立。
- **#13 major**：`cultivation/breakthrough.rs` 突破失败 `qi_max_frozen` 无上界封顶 + 系数偏差 2× → 可致永久真元冻结（状态机漏上界 + plan-code 偏差）。修：补上界 + 校系数。

## §N 开放问题

1. dying-elder ID 命名空间统一（emit 端改发 MC EntityId vs client 端改查 ECS index——哪端为准）。
2. emit 孤岛批是否一并接（antidote/renew）还是逐个 fix PR。
3. #9 并入 plan-skill-cast-meridian-gate-v1 还是独立。
4. #12 并入 plan-qi-conservation-leaks-v1 还是独立。

## 审计来源

bug-hunt round2（workflow，7 角度 finder + 对抗裁决，24 候选）。已对 origin/main 复核剔除 stale(#7 NarrationKind 已修)/dup(#6 round1 已收)。**report-only**：critical dying-elder 优先；其余机械/守恒/状态机待 consume 决议。**方法论修正**：后续 bug-hunt finder 须以 fresh origin/main worktree 为 ROOT，不扫主仓 stale 工作目录。

---

## 2026-07-28 Round bundle finding triage

本节是 master §6.16 / §7 一次性 docs-only 归档移交记录；上文未实施 finding 只有在下表登记唯一 owner 后才退出原聚合队列。

## Finding Mapping

| Finding | 当前裁决 / current `file:line` | 分类 | Canonical owner / merged evidence | 文档动作 |
|---|---|---|---|---|
| #2 dying-elder entity ID 命名空间 | `client/src/main/java/com/bong/client/dying_elder/DyingElderEncounterHandler.java:53-78` 与 `client/src/main/java/com/bong/client/dying_elder/DyingElderInteractionKeybindings.java:99-119` 使用 MC protocol entity ID | `already-fixed/invalid`（already-fixed） | `3b1e13b8f` / PR #605 | 仅归档 |
| #3 dying-elder 0 sentinel | `client/src/main/java/com/bong/client/dying_elder/DyingElderEncounterStore.java:27-28,118-124,176-184` 固定 Valence ID 从 1 起、0 只作 sentinel；`client/src/main/java/com/bong/client/dying_elder/DyingElderInteractionKeybindings.java:99-105` 拒绝 ≤0 | `already-fixed/invalid`（already-fixed） | `3b1e13b8f` / PR #605 | 与 #2 同修 |
| #0 AntidoteResult emit 孤岛 | `server/src/network/dugu_event_bridge.rs:28-39` 消费并发布 `RedisOutbound::AntidoteResult`；`agent/packages/tiandao/src/dugu-narration.ts:158-180,191-203,240-270` 当前订阅/路由 `DUGU_ANTIDOTE_RESULT`、校验契约并把最终 narration 发布到 `AGENT_NARRATE`，`agent/packages/tiandao/tests/dugu-narration.test.ts:80-95,147-267` 锁定订阅、成功/失败、fallback 与非法契约分支 | `already-fixed/invalid`（already-fixed） | `d2539eb89`/#611；agent follow-up `7e85f7a3c`/#617 | 仅归档 |
| #1 RenewCompleted 无音效 | `server/src/lingtian/systems.rs:699` 生产，`server/src/network/audio_trigger.rs:679-725` 已有 `RenewCompleted` reader/播放分支 | `already-fixed/invalid`（already-fixed） | `d2539eb89` / PR #611 | 仅归档 |
| #5 FactionWar HUD bridge 缺失 | `client/src/main/java/com/bong/client/network/ProtoServerDataBridge.java:169-173,585` 明确 intentionally omitted；server 已无 `FactionWarState` runtime producer/HUD consumer，功能由 PR #667 主动退役 | `already-fixed/invalid`（invalid） | 退役 PR #667；current bridge 清理 `550dd7555` / PR #826 | 不复活 retired payload |
| #11 CombatBodyPart `back` | `agent/packages/schema/src/combat-event.ts:26-30` 当前 schema 允许 server body-part 字符串，修复 commit 已补 sample/variant | `already-fixed/invalid`（already-fixed） | `dc8328f1d` / PR #593 | 仅归档 |
| #9 `bao_mai.*` / `baomai.*` 失配 | `server/src/cultivation/known_techniques.rs:78-79,351-367` 与 `server/src/combat/baomai_v3/events.rs:6-11` 当前统一 `baomai.*` | `already-fixed/invalid`（already-fixed） | `6145d3d8a` / PR #602 | 仅归档 |
| #10 `pill_rush < Realm::Awaken` 死守卫 | `server/src/dandao/skills.rs:221-224` 仍把合法 realm 与最低变体 `Realm::Awaken` 比较，分支恒 false | `independent-domain-fix` | successor 短名 `plan-bughunt-dandao-pill-rush-dead-realm-guard-v1` | 后续单独 docs PR 立 skeleton；本 PR 不创建 |
| #12 Dugu returned zone qi 丢失 | `server/src/combat/dugu_v2/mod.rs:31-34` 注册四组 zone-credit system；`server/src/combat/dugu_v2/tick.rs:151-256,265-362,372-464,472-572` 当前经 `qi_release_to_zone` 入 zone/overflow 并写 audit | `already-fixed/invalid`（already-fixed） | `8e85f423a`/#604；follow-up `c65c8de7f`/#698 | 仅归档 |
| #13 breakthrough `qi_max_frozen` | `server/src/cultivation/breakthrough.rs:586-591` 已有 0.5×cap 但仍 `severity*10`；`server/src/cultivation/overload.rs:14-16,44-77` 的 canonical factor 为 5 | `independent-domain-fix`（partial fixed） | cap `6db5b7d51`/#597；successor 短名 `plan-bughunt-breakthrough-freeze-factor-align-v1` | 后续单独 docs PR 只承接 10↔5 规格漂移，不重修 cap |

## Finish Evidence

- **落地清单**：完成十条唯一分类；记录 pill-rush 与 freeze-factor 两个后续 successor 短名（均尚未立 skeleton）；bundle 迁入本路径。
- **关键 commit / PR**：#605、#611/#617、#593、#602、#604/#698、#597 均在 `origin/main` 祖先链且当前修复存在；FactionWar 由 PR #667 退役并由 `550dd7555`/#826 保持 unmapped。
- **测试结果**：docs-only triage；最终执行 docs static gates 与 exact-HEAD validator，不运行 server/client/agent build。
- **跨仓库核验**：dying-elder client↔server ID、Dugu server↔agent outbound、CombatBodyPart TypeBox、retired FactionWar bridge 均核对。
- **遗留 / 后续**：#10 与 #13 剩余规格漂移等待各自独立 docs PR 建立 successor skeleton 后实施；本 bundle 禁止再消费。
