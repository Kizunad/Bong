# plan-bughunt-r10-findings-v1（已归档）

> 一句话主题：round10 六条 finding 已按 `origin/main @ c625d5a5` 拆散：三条 merged 修复，一条 focused shield lifecycle owner，两条 shutdown flush 统一由 R3 P3 吸收。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| T0 | 六条 finding current-code + ancestor commit/PR 复核 | ✅ 2026-07-28 |
| T1 | shield/R3 canonical owner 与 absorb list 去重 | ✅ 2026-07-28 |
| T2 | mapping table、Finish Evidence、归档 | ✅ 2026-07-28 |

## Finding Mapping

| Finding | 当前裁决 / current `file:line` | 分类 | Canonical owner / merged evidence | Absorb list / 文档动作 |
|---|---|---|---|---|
| #5 shield-break state leak | `server/src/combat/resolve.rs:1262` 空 offhand fallback wooden shield；`:1348` 破盾只删物品/emit；`combat/lifecycle.rs:295` stale state 仍 drain | `independent-domain-fix` | `docs/plans-skeleton/plan-bughunt-shield-break-state-cleanup-v1.md` | 新建唯一 owner；defense-hardening 去重表已改指它 |
| #6 Guangbo practice producer | `server/src/network/cast_emit.rs:349-353` 当前发送 `GuangboTicaoPracticeEvent`，`combat/body_conditioning.rs:119-153` 消费并按守恒门增长熟练度 | `already-fixed/invalid`（already-fixed） | `67c647346` / PR #652 | 不归 R9/R10 重构 |
| #1 mineral exhausted shutdown flush | `server/src/mineral/mod.rs:93-102` 仍只在 Update 注册；节流 persistence 无 Last/AppExit 强刷 | `absorbed-by-track` | `docs/plans-skeleton/plan-refactor-persistence-slices-v1.md` R3 P3 | 原 absorb list 漏列；本 PR 已精确补录 finding/symbol |
| #2 zone influence shutdown flush | `server/src/persistence/mod.rs:698-710` 只把 influence persist 注册在 Update、Last 仅刷 zone runtime；`:953-971` 的 influence snapshot 仍按 300 秒节流 | `absorbed-by-track` | 同 R3 P3 | 原 absorb list已列 `zone-influence-shutdown-flush`；active focused plan 标 DELEGATED，禁止双线 |
| #3 `DyingElderQi` TypeBox | `agent/packages/schema/src/spiritual-sense.ts:17` 当前含 `DyingElderQi`；`samples/server-data.spiritual-sense-targets.sample.json:12` pin wire literal | `already-fixed/invalid`（already-fixed） | `a64b9f7e1` / PR #704 | 仅归档 |
| #4 tribulation kind inline union | `agent/packages/schema/src/server-data.ts:105,991-1005` 当前复用 `tribulation.ts:7-14` 的 canonical `TribulationKindV1`；`samples/server-data.tribulation-state.jue-bi.sample.json:7` pin `jue_bi` | `already-fixed/invalid`（already-fixed） | `c09de7228` / PR #705 | 仅归档 |

## Finish Evidence

- **落地清单**：shield 新建 focused successor，并更新 `plan-defense-hardening-v1` 去重引用；R3 P3/absorb list 精确加入 mineral，确认 zone influence 已列并把 standalone active 标为 DELEGATED；三条 merged 修复结案；bundle 迁入本路径。
- **关键 commit / PR**：`67c647346`/#652、`a64b9f7e1`/#704、`c09de7228`/#705 均为目标 HEAD 祖先且当前修复存在。
- **测试结果**：docs-only triage；最终执行 docs static gate + exact-HEAD validator。
- **跨仓库核验**：shield 为 server 状态+既有 client feedback；shutdown 为 server R3；schema 两项 TypeBox/Rust/client literal 当前对齐。
- **遗留 / 后续**：#5 由 shield successor；#1/#2 由 R3 P3。graceful `Last + AppExit` 只保证 SIGINT/SIGTERM 等正常停服，不宣称覆盖 SIGKILL、进程崩溃或断电。
