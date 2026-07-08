# plan-bughunt-zone-ecology-global-refuge-v1（骨架）

> **骨架（草案）**。一句话主题：`fauna::migration` 的避难区选择在生产态永远拿不到真实邻接图，低灵气 zone 会退化成“从全表挑最高灵气区”，把本应是**邻区大迁徙**的生态链路错误放大成**跨整图/跨生态带导流**，并进一步把兽潮刷到错误 zone。

> 结论先行：这是一个 **real bug**，不属于已排除的 `zone_info stale` / `pseudo vein restart loss` / `weather overlay collapse` / `lingtian default zone shadow`。根因是 `ZoneGraph` 只被 `default()` 注入、从未在生产代码填边；而 `select_migration_target_zone` / `migration_neighbors` 在空图时直接 fallback 到 `zones.iter().collect::<Vec<_>>()`，随后按 `spirit_qi` 最大值选目标。

## 阶段总览

| 阶段 | 主题 | 路由 | 状态 |
|------|------|------|------|
| P0 | zone/ecology/world events：大迁徙错误指向全图最高灵气区，连带把兽潮落到错误 zone | fix_pr | ⬜ |

## P0 — 大迁徙错误指向全图最高灵气区

- **复现路径（基于现有生产代码，无需改代码）**
  1. 启服后进入任一低灵气 zone，例如 `north_wastes`（`server/zones.json:956-970`，`spirit_qi=0.065261`，中心约 `0,128,-7000`）。
  2. 继续把该区灵气压到 `MIGRATION_THRESHOLD=0.05` 以下并维持 600 tick；`fauna_migration_system` 会在 `server/src/fauna/migration.rs:349-380` 触发 `ZoneQiCriticalEvent` 与 `MigrationEvent`。
  3. 生产态 `fauna::register` 只注入 `ZoneGraph::default()`（`server/src/fauna/mod.rs:31-40`），仓内没有任何非测试代码给它加边；因此 `select_migration_target_zone` 会走空图 fallback（`server/src/fauna/migration.rs:828-850`）。
  4. 真实地图里全表最高灵气区是 `celestial_isles`（`server/zones.json:815-829`，`spirit_qi=0.852253`，中心约 `-4400,168,1200`），所以 `north_wastes` 的迁徙目标会被错误选成 `celestial_isles`，方向从 `dz=+8200` 的超远距离拉过去，而不是相邻 refuge。
  5. `migration_trigger_system` 会把该错误目标写进每只 fauna / NPC 的 `MigrationTarget`（`server/src/fauna/migration.rs:609-635`）；Dormant LOD 还会在 `migration_move_system` / `horde_migration_system` 里直接 `position.set(target.target_pos)`（`server/src/fauna/migration.rs:653-656`、`562-565`），造成跨图瞬移。
  6. 当错误目标区累计到 `MIGRATION_BEAST_TIDE_THRESHOLD` 后，`migration_to_beast_tide_system` 会在那个错误 zone 直接 enqueue `beast_tide`（`server/src/fauna/migration.rs:685-719`），把世界事件继续放大成错误落点。

- **与设计预期的明确背离**
  - `docs/finished_plans/plan-world-ecology-events-v1.md:184-187` 写的是“灵泉湿地灵气耗尽后，湿地内兽群向**南邻 zone** 奔逃，随后在邻区触发兽潮”。
  - `ZoneQiCriticalEvent.neighbors` 的命名和注释本身也声明它承载的是“邻近 zone 列表”，不是“全地图灵气排行榜”。
  - `server/src/world/zone.rs:323-359` 已有 `zones_are_adjacent` / `adjacent_zone_names` 工具，但迁徙链路完全没接。

- **根因链路**
  1. `fauna::register` 在生产代码里只 `insert_resource(migration::ZoneGraph::default())`，没有任何启动期 builder（`server/src/fauna/mod.rs:37-40`）。
  2. 仓内对 `ZoneGraph::from_edges` / `add_undirected_edge` 的非测试引用为零；`ambient_scheduler` 的测试注释还直接承认“无 ZoneGraph 时 fallback 到全体 zones”（`server/src/npc/spawn/ambient_scheduler.rs:2016-2017`）。
  3. `fauna_migration_system` 和 `beast_horde_detect_system` 都把 `graph.as_deref()` 传给 `select_migration_target_zone`（`server/src/fauna/migration.rs:349-350`、`411-412`）。
  4. `select_migration_target_zone` 在空图时把 `candidates` 设为 `zones.iter().collect::<Vec<_>>()`，再做 `max_by(spirit_qi)`（`server/src/fauna/migration.rs:833-849`）；`migration_neighbors` 同样把“neighbors”退化成除了自己以外的全表 zone（`server/src/fauna/migration.rs:852-870`）。
  5. `migration_trigger_system` 又从 `event.neighbors` 里二次取最大灵气值（`server/src/fauna/migration.rs:613-632`），所以错误目标不是只影响日志/VFX，而是直接驱动实体迁徙。
  6. `migration_to_beast_tide_system` 以后续到达计数为依据，在错误 target zone 触发 `EVENT_BEAST_TIDE`（`server/src/fauna/migration.rs:706-718`），把 bug 从 ecology 扩散到 world events。

## 这个 bug 对实际游玩体验的影响

- 玩家看到的“万兽奔逃方向”不再表示附近 refuge，而是被全图最高灵气区劫持；顺着兽潮找安全区会被带偏，世界阅读性直接失真。
- Dormant fauna / NPC 会被瞬移到错误 target zone，中远距离观察会出现“某区突然空了、另一块地突然刷满迁徙兽”的割裂感。
- 错误到达计数会把 `beast_tide` 刷在不该刷的 zone，导致玩家在高灵气区遭遇一波与本地生态毫无因果关系的兽潮。
- 这会破坏 `plan-world-ecology-events-v1` 设计的反馈环：本应是“低灵气区 -> 邻区压力上升”，实际变成“低灵气区 -> 全图最高灵气区吃到迁徙和兽潮”，生态因果被拉断。

## 影响面

- `server/src/fauna/migration.rs`：`fauna_migration_system`、`beast_horde_detect_system`、`migration_trigger_system`、`migration_move_system`、`horde_migration_system`、`migration_to_beast_tide_system`
- `server/src/fauna/mod.rs`：`ZoneGraph` 资源初始化
- `server/src/world/zone.rs`：现成 adjacency helper 未接入
- 受影响玩法：低灵气区 fauna/NPC 迁徙、兽潮落点、世界事件方向可读性、AI/玩家对生态反馈环的判断

## 修复建议

1. 启服时基于 `ZoneRegistry::adjacent_zone_names(...)/zones_are_adjacent(...)` 生成真实 `ZoneGraph`，至少覆盖 Overworld 主图；不要把“空图 fallback 全表”留在生产路径。
2. 若 adjacency 图缺失，应 **fail closed**：宁可不触发迁徙，也不要 silently 退化成全图最高灵气区。
3. `select_migration_target_zone` / `migration_neighbors` 至少要加同维度过滤，并优先按 adjacency gate，再比较 `spirit_qi`。
4. 增补 pin 测试：生产默认注册后 `ZoneGraph` 非空；`north_wastes` 一类低灵气 zone 只能选相邻 refuge，不能直跳 `celestial_isles`。

## 两轮反方裁决（当前会话无 subagent/delegate 工具，退化为本地手工双轮）

- **Round 1 反方论点**：空 `ZoneGraph` 可能是有意设计，fallback 全表只是临时近似，不算 bug。
  - **驳回理由**：设计文档写的是“邻近 zone”“万兽南奔”，代码里也已有 adjacency helper；而生产代码从未填边，说明当前行为不是“退一步的近似”，而是“设计已写、接线缺失”的真实断路。

- **Round 2 反方论点**：就算目标选远了，实体大多要慢慢走，实际游玩影响未必显著。
  - **驳回理由**：`migration_trigger_system` 把错误目标写进真实 `MigrationTarget`；Dormant LOD 直接 `position.set(target.target_pos)`，不是慢慢偏航；随后 `migration_to_beast_tide_system` 还会在错误落点刷 `beast_tide`。这已经是可观测的玩法级错误，不是纯叙事瑕疵。

## 审计来源

- 2026-07-05 bughunt，范围限定在 zone / ecology / world events。
- 已显式避开：`zone_info stale`、`pseudo vein restart loss`、`weather overlay collapse`、`lingtian default zone shadow`。
- 本骨架仅记录 bug，不包含源码修改。
