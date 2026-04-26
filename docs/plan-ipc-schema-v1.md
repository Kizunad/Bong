# Bong · plan-ipc-schema-v1 · 模板

**横切基础专项**。集中管理所有 Redis channel + TypeBox schema + 双端对齐规范。

**落位**：`agent/packages/schema/` source of truth → JSON Schema export → Rust serde。

**交叉引用**：CLAUDE.md「Server ↔ Agent IPC」· 各 plan 自述 channel。

---

## §0 设计轴心

- [x] 单一真相源（TypeBox in TS）
- [x] 双端强校验（samples/*.json）
- [x] channel 命名规范（`bong:<domain>` / `bong:<domain>/<event>`，双端 `CHANNELS` / `CH_*` 常量 + frozen 测试）
- [ ] 版本化策略

## §1 Channel 总表

| Channel | 方向 | 节流 | Payload | 来源 plan |
|---|---|---|---|---|
| `bong:world_state` | server → agent | | | server |
| `bong:agent_cmd` | agent → server | | | agent |
| `bong:player_chat` | client ↔ server | | | client |
| `bong:combat/*` | | | | combat / HUD |
| `bong:cultivation/*` | | | | cultivation |
| ... | | | | |

## §2 Schema 版本化

- [ ] 向后兼容策略（新增可选字段 vs 破坏性变更）
- [ ] schema hash / version 字段
- [ ] migration 流程

## §3 命名规范

- [x] channel 前缀约定（`bong:<domain>/<event>`，已落于 botany/skill；agent_* / *_event / world_state 走单段命名）
- [ ] Intent / State / Event 后缀
- [ ] Store 命名

## §4 测试与校验

- [x] samples/*.json 双端 round-trip 测试
- [ ] 新增 channel checklist

## §5 实施节点

- [x] P1 · 盘点现有 channel → 补档
- [ ] P2 · 版本字段加入
- [x] P3 · CI schema 校验

## §6 开放问题

- [ ]

---

## 进度日志

- 2026-04-25：现状盘点 — 已有 18 个 channel 双端落地（`agent/packages/schema/src/channels.ts` ↔ `server/src/schema/channels.rs`，含 `REDIS_V1_CHANNELS` 与 frozen 测试），TypeBox source-of-truth + samples/*.json round-trip + CI 校验闭环；剩 P2 版本化（schemaVersion / migration）未启动。
