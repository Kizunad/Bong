# Bong · plan-npc-fixups-v1 · 骨架

NPC 系统**三个独立正确性 bug 集中修复 fastlane plan**。源自 plan-npc-perf-v1 五路 sonnet agent 探查 + 主链路源码读取。每个 bug 独立 PR 不打包，配饱和化回归测试 pin 行为。**无 worldview / qi_physics 锚点**（纯 fix plan）。

**前置依赖**：`plan-npc-ai-v1` ✅（NPC ECS Bundle / Navigator / Archetype / skin spawn 逻辑基础）

**反向被依赖**：

- `plan-npc-perf-v1` ⏳ → baseline 录档前应已修 #1 #2，否则"NPC 不动"污染性能基线
- `plan-npc-virtualize-v1` ⏳ → hydrate spawn ECS entity 后 NPC 必须直接落地，#1 是 hydrate 路径的隐式前提

---

## 接入面 Checklist

- **进料**：`server/src/npc/navigator.rs:275`（#1 重力 idle）/ POI `pos_xyz` 坐标（#2 A* 不对齐）/ `server/src/npc/spawn.rs:963`（#3 MineSkin fallback）
- **出料**：三个 bug 各自独立 PR + 回归测试；无新 component / event / schema
- **共享类型**：复用已有 `Navigator`、`NpcKind`（不新建）
- **跨仓库契约**：纯 server 端 fix，无 agent / client 影响
- **worldview 锚点**：无（纯 fix）
- **qi_physics 锚点**：无

---

## §0 Bug 清单

### #1 重力 idle 失效（高优先）

**位置**：`server/src/npc/navigator.rs:275`

**现象**：所有 idle NPC 永远悬空在 spawn Y，从不落地。

**根因**：`nav.is_idle()` 分支直接 `continue`，跳过了 `snap_to_ground` 调用。

```rust
// 当前（错误）
if nav.is_idle() {
    continue;  // 跳过了 snap_to_ground
}
// snap_to_ground(...);
```

**修法**：idle 分支内先做 `snap_to_ground`，再 `continue`（或重排顺序，把 `snap_to_ground` 移到 `continue` 前）。

**回归测试**：spawn NPC → assert Y 坐标在 1 tick 后等于 heightmap Y（不允许浮空 > 0.5 格）；修前 assert fail，修后 pass。

---

### #2 A* 路径返回空时永远 idle（高优先）

**现象**：NPC 永远 idle 不移动（无路径警告，静默卡死）。

**根因**（三层叠加）：
1. POI `pos_xyz` 的 Y 与 worldgen heightmap 不对齐 → A* 起点无 walkable block / chunk 未 load → `ChunkLayer` block 查询返回 `None` → 路径为空 `Vec`
2. `goal` 超过 `MAX_PATH_ITERS=400` 节点上限 → A* 截断返回空
3. `compute_path` 返回空 `Vec` 时 navigator 不报错、不标 fail，只是永远 idle

**修法**：
- POI Y 对齐：从 heightmap 重新查 Y，而不是直接用存储坐标
- chunk 未 load：增加 `ChunkLayer::is_loaded` 检查，未 load 时 delay 而不是空路径
- 空路径处理：`compute_path` 返回空 → navigator 置为 `WanderFailed` 状态 + warn log（不是静默 idle）

**回归测试**：给 NPC 设置 heightmap 之外的 Y 坐标 → assert navigator 最终进入 `WanderFailed` 而非永远 idle。

---

### #3 MineSkin fallback 退化女巫 entity（中优先）

**位置**：`server/src/npc/spawn.rs:963`

**现象**：`MINESKIN_API_KEY` 未配置时，fallback rogue commoner 100% spawn 为女巫（WITCH entity），破坏修仙观感。

**根因**：`fallback_rogue_commoner_kind` 在 skin fallback 时硬返回 `WITCH`。

**修法**：fallback 时从 commoner-appropriate entity list（如 `VILLAGER` 或随机凡人外观）中选取，不使用 `WITCH`。保留 `WITCH` match 分支供其他路径用（不删，避免 exhaustive match 报错）。

**回归测试**：mock `MINESKIN_API_KEY` 为空 → spawn 100 rogue commoner → assert 无 WITCH entity；assert entity 类型在白名单 `[VILLAGER, ...]` 内。

---

## §1 阶段规划

| 阶段 | 内容 | 状态 |
|---|---|---|
| P0 | Bug #1 重力 idle 修复 + 回归测试 | ⬜ |
| P1 | Bug #2 A* 路径空处理修复 + 回归测试 | ⬜ |
| P2 | Bug #3 MineSkin fallback 修复 + 回归测试 | ⬜ |
| P3 | sonnet Explore 异步探查剩余 bug → 评估纳入本 plan / 派生 v2 / 入 reminder | ⬜ |

## §2 验收标准

- `cargo test -p server npc` 三条回归测试全绿
- plan-npc-perf-v1 baseline 可以在本 plan P0/P1 完成后录档（NPC 不再静默 idle）
- plan-npc-virtualize-v1 的 hydrate 路径在 P0 修好后不再悬空

## §3 开放问题（P0 决策门收口）

1. **WITCH match 分支保留 vs 删**：其他调用路径是否依赖 WITCH？确认后决定是 fallback 绕过还是彻底替换
2. **修法是否扩展全 archetype**：只修 rogue commoner 还是所有 archetype fallback 都排查
3. **warn log 是否升级为 metric**：A* 空路径频率高的话是否接入 Prometheus counter（可留 v2 / plan-npc-perf-v1 顺手加）
