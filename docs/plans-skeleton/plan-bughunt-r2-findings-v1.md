# plan-bughunt-r2-findings-v1（骨架）

> **骨架（草案）**。一句话主题：代码库自检 bug-hunt **round2**（换角度:emit 孤岛/client+agent/境界门控/schema 双端）确认的一批真 bug——含 **1 critical（dying-elder 给丹永久失效）** + 多处 emit 孤岛/schema 双端不齐/守恒/状态机缺陷。**已对 origin/main(63996fbf1) 复核**，剔除 stale/dup。

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
