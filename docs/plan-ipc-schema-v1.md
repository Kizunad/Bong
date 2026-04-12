# Bong · plan-ipc-schema-v1 · 模板

**横切基础专项**。集中管理所有 Redis channel + TypeBox schema + 双端对齐规范。

**落位**：`agent/packages/schema/` source of truth → JSON Schema export → Rust serde。

**交叉引用**：CLAUDE.md「Server ↔ Agent IPC」· 各 plan 自述 channel。

---

## §0 设计轴心

- [ ] 单一真相源（TypeBox in TS）
- [ ] 双端强校验（samples/*.json）
- [ ] channel 命名规范
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

- [ ] channel 前缀约定（`bong:<domain>/<event>`）
- [ ] Intent / State / Event 后缀
- [ ] Store 命名

## §4 测试与校验

- [ ] samples/*.json 双端 round-trip 测试
- [ ] 新增 channel checklist

## §5 实施节点

- [ ] P1 · 盘点现有 channel → 补档
- [ ] P2 · 版本字段加入
- [ ] P3 · CI schema 校验

## §6 开放问题

- [ ]
