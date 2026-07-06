# plan-bughunt-workbench-cross-dimension-break-v1（skeleton）

> **Skeleton（BugHunt B4 / server-gameplay r04）**。一句话主题：制作台 `WorkbenchBlock` 放置时进入玩家所在维度 layer，但后续拆除、开屏、制作配方 station 门禁都只按裸 `Position` 查全局制作台，主路径会让玩家在异维同坐标挖普通方块时误拆主世界制作台并回收 `workbench_item`。

## Bug 摘要

`WorkbenchBlock` 是纯实体化放置物。放置路径在 `server/src/world/block_place.rs:142` 读取玩家 `CurrentDimension`，并在 `server/src/world/block_place.rs:160` 选择对应 `DimensionLayers` layer；但 `PlaceableBlockKind::Workbench` 分支在 `server/src/world/block_place.rs:470` 调用 `handle_workbench_place` 时没有把 `placement.dimension` 传入，`server/src/craft/workbench.rs:38` 的 `WorkbenchBlock` 也只保存 `placed_by` / `placed_at_tick`。

后续 `handle_workbench_break` 在 `server/src/craft/workbench.rs:203` 用 `DiggingEvent.position` 裸坐标扫描全局 `Query<(Entity, &Position, &WorkbenchBlock)>`，没有比较玩家 `CurrentDimension`、制作台 `CurrentDimension` 或 marker `EntityLayerId`。因此不同维度相同坐标的普通挖掘事件可以命中另一个维度的制作台实体。

## 实际游玩体验影响

玩家在天隙渊或未来其他维度挖掉与主世界制作台同坐标的普通方块时，会收到一个 `workbench_item` 返还，同时主世界那个制作台 marker 被 `Despawned`。对玩家表现为：

- 主世界制作台被远程拆掉，回到主世界后工作台凭空消失。
- 异维玩家得到一个制作台物品，形成跨维复制式回收。
- 同根问题还会影响 `station=Workbench` 配方的附近制作台判定：只要另一维同坐标附近存在制作台，当前维度也可能被判定为“附近有制作台”。
- `WorkbenchOpen` 右键开屏也缺 server 维度校验；正常客户端通常只会发当前可见实体，但 server 仍缺权威门禁。

## 证据定位

- `server/src/world/block_place.rs:142` / `:160`：放置时确实按玩家维度选择 layer。
- `server/src/world/block_place.rs:470`：Workbench 分支调用 `handle_workbench_place` 时丢弃 `placement.dimension`。
- `server/src/craft/workbench.rs:38`：`WorkbenchBlock` 不存维度。
- `server/src/craft/workbench.rs:96` / `:108`：制作台 marker spawn 到指定 layer 后只插入 `WorkbenchBlock`。
- `server/src/craft/workbench.rs:183` / `:203`：拆除系统的制作台 query 不含 `CurrentDimension` / `EntityLayerId`，匹配条件仅为裸坐标。
- `server/src/world/block_break.rs:34` / `:47`：默认方块破坏用玩家 `VisibleChunkLayer` 修改当前层方块，但这不会为其他 `DiggingEvent` consumer 自动提供维度门禁。
- `server/src/network/craft_emit.rs:153` / `:154` / `:187`：制作配方 station 门禁只查玩家 `Position` 和全局 `WorkbenchBlock` 的 `Position`。
- `server/src/craft/workbench.rs:118` / `:119` / `:130`：制作台开屏只做距离，不做维度。
- 正例对照：`server/src/world/container_block.rs:137` / `:143` 为实体化容器插入 `CurrentDimension(placement.dimension)`；`server/src/world/container_open.rs:85` / `:87` 打开时比较玩家/容器维度。

## 触发路径

1. 玩家 A 在主世界坐标 `[x,y,z]` 放置制作台，server spawn 一个带 `WorkbenchBlock` 的 marker。
2. 玩家 B 进入天隙渊或其他维度，在同样坐标 `[x,y,z]` 挖任意可挖方块。
3. `DiggingEvent` 不带维度，只带 `client` 与 `position`；默认破坏系统按玩家当前 `VisibleChunkLayer` 正常处理当前维度方块。
4. 同一帧 `handle_workbench_break` 也读到该事件，并用裸坐标在全局制作台实体中找到主世界制作台。
5. 该系统给玩家 B 返还 `workbench_item`，并对主世界制作台 marker 调 `break_placeable(... Workbench ...)` 插入 `Despawned`。

## 反方审查记录

### Round 1

反方结论：接受，但建议收窄触发面。开屏路径普通客户端通常只会命中当前可见实体，跨维开屏更像恶意包或陈旧包；制作配方门禁也可能依赖陈旧 UI 或伪造 `CraftStart`。但拆除路径更强，因为 `DiggingEvent` 是普通挖掘事件，server 侧 workbench consumer 没有维度过滤。

关键反方证据：

- `EntityLayerId` 是 marker 上的普通 component；这些 query 没读它，Bevy 不会自动按 layer 过滤。
- `WorkbenchBlock` 与容器不同，没有 `CurrentDimension`。
- 已知开放 PR #973 / #981 / #990 分别覆盖灵龛、炼丹炉、普通容器断线锁，不覆盖制作台。

### Round 2

反方结论：可立 skeleton。`DiggingEvent` 本身不携带 layer/dimension；默认 break 系统只保护默认方块状态，不会给 workbench consumer 自动加 gate。系统顺序也挡不住，因为 `handle_workbench_break` 不查当前 layer block state，只用同一份 event 的裸坐标扫描全局制作台。

最终收窄：以“制作台异维同坐标误拆/复制式回收”为主 bug；开屏和 `station=Workbench` 判定作为同根次级风险纳入同一修复计划。

## Skeleton Fix Plan

1. 让制作台实体记录维度。
   - `handle_workbench_place` 新增 `dimension: DimensionKind` 参数。
   - `PlaceableBlockKind::Workbench` 分支把 `placement.dimension` 传入。
   - spawn marker 后插入 `CurrentDimension(dimension)`，对齐容器放置路径。

2. 修复拆除门禁。
   - `handle_workbench_break` 的 workbench query 加 `Option<&CurrentDimension>` 或 `&CurrentDimension`。
   - 匹配条件改为 `workbench_block_pos(position) == event.position && workbench_dimension == player_dimension`。
   - 返还背包满时的掉落维度继续使用玩家当前维度。

3. 修复制作配方 station 门禁。
   - `apply_craft_intents` 读取玩家 `CurrentDimension`。
   - `workbenches` query 同时读取 `Position` 与制作台维度。
   - `has_nearby_workbench` 仅在同维度且 3 格内时为 true。

4. 修复制作台开屏门禁。
   - `handle_workbench_interact` 读取玩家与制作台维度。
   - 不同维度直接拒绝，不发送 `WorkbenchOpen` payload / 音效。

## 验收测试计划

- server：跨维拆除拒绝。构造主世界 `WorkbenchBlock + CurrentDimension(Overworld)`，玩家在 `CurrentDimension(Tsy)` 同坐标发 `DiggingEvent::Stop`；断言玩家未新增 `workbench_item`，制作台未插入 `Despawned`。
- server：同维拆除仍成功。同维玩家挖制作台坐标；断言返还 `workbench_item` 且制作台插入 `Despawned`。
- server：`station=Workbench` 配方门禁跨维拒绝。同坐标异维制作台存在时，当前维度玩家 `CraftStart` 应得到 `StationOutOfRange`，材料与真元不变。
- server：同维 `station=Workbench` 配方仍允许，保持现有 happy path。
- server：`WorkbenchOpenRequest` 跨维拒绝，同维开屏仍发送 `WorkbenchOpen` payload。
- 回归命令：在 `server/` 运行 `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`。

## 风险

- 需要为现有旧制作台实体补维度语义。运行时新放置制作台可直接插 `CurrentDimension`；如果存在旧存档恢复路径，缺失维度应保守视为 `Overworld`，但测试必须覆盖缺失维度 fallback 不再误放大跨维权限。
- `apply_craft_intents` 当前只拿 `player_positions: Query<&Position>`，加维度 query 后要避免 Bevy borrow 冲突。
- `handle_workbench_interact` 的正常客户端路径已经有可见实体限制，但 server 修复仍要保留权威校验，防陈旧包和恶意包。
- 不要复用普通容器 `opened_by` 逻辑；制作台是无会话 UI，修复范围只做维度/距离门禁。
