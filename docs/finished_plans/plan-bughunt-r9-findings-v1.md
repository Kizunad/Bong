# plan-bughunt-r9-findings-v1（已归档）

> 一句话主题：round9 六条 finding 已按 `origin/main @ c625d5a5` 复核：五条已由 merged PR 闭环，散灵珠 runtime ledger account cleanup 仍 live 并转交 focused successor。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| T0 | 六条 finding current-code + ancestor commit/PR 复核 | ✅ 2026-07-28 |
| T1 | scatter-bead unique owner 登记 | ✅ 2026-07-28 |
| T2 | mapping table、Finish Evidence、归档 | ✅ 2026-07-28 |

## Finding Mapping

| Finding | 当前裁决 / current `file:line` | 分类 | Canonical owner / merged evidence | 文档动作 |
|---|---|---|---|---|
| #6 `spirit_qi_max` 未下发 | `server/src/player/state.rs:265` 当前 emit `cultivation.qi_max`，`server/src/player/state.rs:5907-5918` 有 payload pin | `already-fixed/invalid`（already-fixed） | `9cfdfe0f7` / PR #700 | 仅归档 |
| #2 vortex particle atlas | `client/src/main/resources/assets/minecraft/atlases/particles.json:209` 已含 `bong:particle/vortex_spiral`，并有 asset/reconciliation tests | `already-fixed/invalid`（already-fixed） | `754b8c3fa`/#838；test hardening `1294cfc0b`/#1079 | 仅归档 |
| #3 ash spider disguise texture | `client/src/main/java/com/bong/client/fauna/FaunaModel.java:31-35` 声明 `ash_spider_disguised.png`，`client/src/test/java/com/bong/client/fauna/FaunaModelDisguiseTest.java:38-64` 验证当前资产路径/存在性 | `already-fixed/invalid`（already-fixed） | `1285f79a0` / PR #674 | 仅归档 |
| #1 scatter-bead ledger zombie | `server/src/zhenfa/mod.rs:215-229,2589-2647` 主动成功链移除 burial 后不删 account；`server/src/zhenfa/mod.rs:2726-2734` 自然耗尽同样只删 burial；canonical API 在 `server/src/qi_physics/ledger.rs:404-405` | `independent-domain-fix` | `docs/plans-skeleton/plan-bughunt-scatter-bead-ledger-account-cleanup-v1.md` | 新建唯一 runtime cleanup owner；`server/src/zhenfa/mod.rs:2528-2537` 仅是 `set_balance` 失败 rollback，不算成功终止 |
| #4 38 个 skill icon 缺失 | 当前 `server/src/cultivation/known_techniques.rs:6-14` 已把多数技能重链到 `gui/items/skill_scroll_*` 单一真源，磁盘资产与 snapshot test 对拍 | `already-fixed/invalid`（already-fixed） | `9d2e29d08`/#1220 + `001bbe7d8`/#1222 | 旧 `gui/skill` 计数不再是 canonical 缺口 |
| #5 tuike 三 icon 缺失 | 当前磁盘有 `skill_scroll_tuike_{don,shed,transfer_taint}.png`，`client/src/test/resources/bong/technique_icon_snapshot.json:32-34` pin 三路径 | `already-fixed/invalid`（already-fixed） | 同 PR #1220/#1222 | 与 #4 同修 |

## Finish Evidence

- **落地清单**：五条 merged 修复结案；scatter bead 新建 focused successor；bundle 迁入本路径。
- **关键 commit / PR**：`9cfdfe0f7`/#700、`754b8c3fa`/#838、`1294cfc0b`/#1079、`1285f79a0`/#674、`9d2e29d08`/#1220、`001bbe7d8`/#1222；均从当前路径反查并验证在目标 HEAD 祖先链。
- **测试结果**：docs-only triage；不复跑 client/server gate，最终执行 docs static gate + exact-HEAD validator。
- **跨仓库核验**：PlayerState server→client 字段、particle atlas、fauna texture、canonical technique icon snapshot 均核对。
- **遗留 / 后续**：仅 scatter-bead runtime account lifecycle，由 focused successor 实施；restart persistence 仍归既有 `plan-bughunt-scatter-bead-burial-restart-loss-v1`，两者不重复。
