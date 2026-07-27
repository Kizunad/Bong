# plan-bughunt-r2-findings-v1（已归档）

> 一句话主题：round2 十条 distinct finding 已按 `origin/main @ c625d5a5` 拆散：七条已修、一条主动退役、一条仍 live、一条部分修复后将剩余规格漂移转 successor。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| T0 | 十条 finding current-code + ancestor commit/PR 复核 | ✅ 2026-07-28 |
| T1 | live/partial finding 登记唯一 focused successor | ✅ 2026-07-28 |
| T2 | mapping table、Finish Evidence、归档 | ✅ 2026-07-28 |

## Finding Mapping

| Finding | 当前裁决 / current `file:line` | 分类 | Canonical owner / merged evidence | 文档动作 |
|---|---|---|---|---|
| #2 dying-elder entity ID 命名空间 | `client/.../DyingElderEncounterHandler.java:53-78` 与 `DyingElderInteractionKeybindings.java:102-118` 使用 MC protocol entity ID | `already-fixed/invalid`（already-fixed） | `3b1e13b8f` / PR #605 | 仅归档 |
| #3 dying-elder 0 sentinel | `DyingElderEncounterStore.java:28,120,179` 固定 Valence ID 从 1 起、0 只作 sentinel；输入端 `:102-104` 拒绝 ≤0 | `already-fixed/invalid`（already-fixed） | `3b1e13b8f` / PR #605 | 与 #2 同修 |
| #0 AntidoteResult emit 孤岛 | `server/src/network/dugu_event_bridge.rs:28-39` 消费并发布 `RedisOutbound::AntidoteResult` | `already-fixed/invalid`（already-fixed） | `d2539eb89`/#611；agent follow-up `7e85f7a3c`/#617 | 仅归档 |
| #1 RenewCompleted 无音效 | `server/src/lingtian/systems.rs:699` 生产，`server/src/network/audio_trigger.rs` 已有 `RenewCompleted` reader/播放分支 | `already-fixed/invalid`（already-fixed） | `d2539eb89` / PR #611 | 仅归档 |
| #5 FactionWar HUD bridge 缺失 | `client/.../ProtoServerDataBridge.java:169-173,585` 明确 intentionally omitted；server 已无 `FactionWarState` runtime producer/HUD consumer，功能由 PR #667 主动退役 | `already-fixed/invalid`（invalid） | 退役 PR #667；current bridge 清理 `550dd7555` / PR #826 | 不复活 retired payload |
| #11 CombatBodyPart `back` | `agent/packages/schema/src/combat-event.ts:26-30` 当前 schema 允许 server body-part 字符串，修复 commit 已补 sample/variant | `already-fixed/invalid`（already-fixed） | `dc8328f1d` / PR #593 | 仅归档 |
| #9 `bao_mai.*` / `baomai.*` 失配 | `server/src/cultivation/known_techniques.rs:78-79,351-367` 与 `server/src/combat/baomai_v3/events.rs:6-11` 当前统一 `baomai.*` | `already-fixed/invalid`（already-fixed） | `6145d3d8a` / PR #602 | 仅归档 |
| #10 `pill_rush < Realm::Awaken` 死守卫 | `server/src/dandao/skills.rs:221-224` 仍把合法 realm 与最低变体 `Realm::Awaken` 比较，分支恒 false | `independent-domain-fix` | `docs/plans-skeleton/plan-bughunt-dandao-pill-rush-dead-realm-guard-v1.md` | 新建唯一 focused owner |
| #12 Dugu returned zone qi 丢失 | `server/src/combat/dugu_v2/mod.rs:31-34` 注册四组 zone-credit system；`tick.rs:151-237,265-564` 当前经 `qi_release_to_zone` 入 zone/overflow 并写 audit | `already-fixed/invalid`（already-fixed） | `8e85f423a`/#604；follow-up `c65c8de7f`/#698 | 仅归档 |
| #13 breakthrough `qi_max_frozen` | `server/src/cultivation/breakthrough.rs:586-591` 已有 0.5×cap 但仍 `severity*10`；`overload.rs:16,64` canonical factor=5 | `independent-domain-fix`（partial fixed） | cap `6db5b7d51`/#597；剩余 owner `plan-bughunt-breakthrough-freeze-factor-align-v1.md` | 只迁移 10↔5 规格漂移，不重修 cap |

## Finish Evidence

- **落地清单**：完成十条唯一分类；新建 pill-rush 与 freeze-factor 两份 focused skeleton；bundle 迁入本路径。
- **关键 commit / PR**：#605、#611/#617、#593、#602、#604/#698、#597 均在 `origin/main` 祖先链且当前修复存在；FactionWar 由 PR #667 退役并由 `550dd7555`/#826 保持 unmapped。
- **测试结果**：docs-only triage；最终执行 docs static gates 与 exact-HEAD validator，不运行 server/client/agent build。
- **跨仓库核验**：dying-elder client↔server ID、Dugu server↔agent outbound、CombatBodyPart TypeBox、retired FactionWar bridge 均核对。
- **遗留 / 后续**：#10 与 #13 剩余规格漂移分别由两份 successor 实施；本 bundle 禁止再消费。
