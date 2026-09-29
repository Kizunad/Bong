# plan-bughunt-world-terrain-minor-cleanups-v1（骨架）

> **来源 issue**：#1796、#1615。两条都位于 terrain 几何/缓存边界：chunk 边界采样和缓存键缺少世界身份。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | biome override 覆盖完整 chunk；giant-sword crater cache 按 terrain/dimension 隔离 | ⬜ |

## §0 摘要

剑海 biome 当前先看 chunk 中心，再决定整块是否启用 override，边界列会漏掉应该覆盖的地形。giant-sword crater 的 cache key 只有 `(min_y, world_height)`，不同 terrain 或 dimension 会复用错误中心数据。修复只涉及确定性 terrain 计算与缓存隔离。

## §1 游玩影响

玩家沿 chunk 边界移动会看到剑海覆盖断裂；切换维度或 terrain 配置后可能看到另一世界的陨坑中心。两者都是服务端生成/查询错误，可能造成区块视觉和碰撞不一致。

## §2 复现路径

1. 让一个 chunk 的中心落在剑海 biome 外、边缘落在 override 区域内，生成该 chunk，边缘列保持原 biome。
2. 在两个 terrain/dimension 上以相同 `min_y/world_height` 请求 giant-sword crater 中心，观察第二次命中第一次缓存。

## §3 今天 `origin/main` 的证据

- `server/src/world/terrain/biome.rs:8-35` 在 `:10-15` 用 chunk 中心决定是否启用 override。
- `server/src/world/terrain/giant_sword.rs:935-963` 的 cache key 只有 `(min_y, world_height)`，没有 terrain/dimension 身份。
- 相关调用没有通过 client/agent payload；结果直接进入 server terrain 查询。

## §4 非重复比对

已查 `docs/finished_plans/`、active plan 和 skeleton 中的 `biome override`、`giant_sword`、`crater cache`；没有已有计划同时覆盖这两个边界。两条共享“terrain 结果必须由完整上下文决定”的根因族，分开验收以免互相掩盖。

## §5 立项检查记录

- `docs/worldview.md`：查区域表、维度和地形关键词；本计划不新增区域 ID。
- `docs/finished_plans/`：查 `plan-worldgen*`、terrain cache、剑海，确认只复用现有地形接口。
- active plan：查 `BiomeOverride`、`TerrainProfile`、`DimensionKind`，未发现同一代码入口的 active 改动。
- skeleton：查 `server/src/world/terrain/biome.rs`、`giant_sword.rs`、#1796、#1615，无同主题骨架。
- `docs/plans-skeleton/reminder.md`：已检查，未找到相关条目；不改该文件。

## §6 接入面与跨仓契约

- **Inputs**：chunk 坐标、完整地形 profile、`DimensionKind`、世界高度参数。
- **Outputs**：server 的 biome/陨坑几何结果和内部 cache；没有新增 IPC 或 S2C 字段。
- **共享类型/事件**：复用 `TerrainProfile`、`DimensionKind`、现有 crater cache 类型。
- **三端契约符号**：server `biome` override 与 `giant_sword` cache；agent **无变更**、client **无变更**（terrain 结果仍由 server 生成，不改变协议）。
- **worldview 锚点**：`docs/worldview.md` 区域/维度地貌表；无真元流动，不调用 `qi_physics`。

## P0 验收

- 用同一规则检查 chunk 四角/边界，覆盖判断不再由中心点错误短路。
- 两个 terrain 或 dimension 即使高度相同也不能共享 crater cache；同一完整 key 才命中。
