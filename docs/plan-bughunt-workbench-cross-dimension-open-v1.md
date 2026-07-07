# plan-bughunt-workbench-cross-dimension-open-v1

一句话：制作台 Workbench 的服务端打开与制作门禁只按坐标距离判断，未绑定 `CurrentDimension`，伪造/复用另一维制作台 `entity_id` 时可跨维打开制作台 UI，并可能让工作台配方误判为“附近有制作台”。

## 实际游玩体验影响

- 玩家在坍缩渊/TSY 与主世界同坐标附近时，只要客户端发出另一维制作台的 protocol `entity_id`，服务端会通过 `WorkbenchOpen` 并下发制作台界面。
- 工作台配方的“附近 3 格内有制作台”检查同样只扫全局 `WorkbenchBlock` 坐标，未过滤维度；玩家可能在另一维同坐标处启动本应要求当前维制作台的手搓配方。
- 正常 Fabric 客户端通常只会看到当前 layer 的实体，因此该 bug 主要是服务端不信任 C2S 输入时的权限/维度隔离缺口；不能依赖客户端准星过滤作为安全边界。

## 复现路径

1. 在主世界放置一个 `workbench_item`，记录该制作台 marker 的 protocol `entity_id`。
2. 将玩家传送/进入 TSY 或其它非主世界维度，并移动到与主世界制作台相同或 3 格内的坐标。
3. 通过 `bong:client_request` 发送 `WorkbenchOpen { v: 1, entity_id: <主世界制作台 id> }`。
4. 观察服务端没有维度拒绝，而是发送 `ServerDataPayloadV1::WorkbenchOpen`。
5. 进一步发送需要制作台的 `CraftStart`，在 TSY 同坐标附近没有真实制作台时，`apply_craft_intents` 仍可能因主世界 `WorkbenchBlock` 坐标命中而放行 `has_nearby_workbench`。

## 根因证据

- `server/src/network/client_request_handler.rs:2333` 的 `WorkbenchOpen` 分支只用 `EntityManager::get_by_id(entity_id)` 把客户端输入解析为全局 `Entity`，随后发送 `WorkbenchOpenRequest { client, workbench }`，没有读取玩家/制作台维度。
- `server/src/craft/workbench.rs:115` 的 `handle_workbench_interact` 查询形状是 `players: Query<&Position, With<Client>>` 与 `workbenches: Query<(&Position, &WorkbenchBlock)>`，只在 `server/src/craft/workbench.rs:130` 调 `is_within_workbench_range`，没有查询或比较 `CurrentDimension`。
- `server/src/world/block_place.rs:455` 的 `PlaceablePlacement` 已有 `dimension`，但 `server/src/world/block_place.rs:470` 的 `Workbench` 分支调用 `handle_workbench_place(...)` 时丢掉该字段。
- `server/src/craft/workbench.rs:89` 的 `handle_workbench_place` 只给 marker 插入 `WorkbenchBlock`，没有插入 `CurrentDimension(placement.dimension)`。
- 对照：`server/src/world/container_block.rs:137` 的容器方块放置会插入 `CurrentDimension(placement.dimension)`；`server/src/world/container_open.rs` 的通用容器打开路径会比较玩家与容器维度。
- 同类延伸：`server/src/network/craft_emit.rs:187` 的 `has_nearby_workbench` 只用 `player_positions: Query<&Position>` 与 `workbenches: Query<&Position, With<WorkbenchBlock>>` 做距离扫描，未过滤维度。

## 修复计划骨架

### P0：制作台实体维度持久化

- 将 `server/src/craft/workbench.rs::handle_workbench_place` 参数扩展为接收 `DimensionKind`，并在 `WorkbenchBlock` marker 上插入 `CurrentDimension(dimension)`。
- 调整 `server/src/world/block_place.rs::place_placeable` 的 Workbench 分支，把 `PlaceablePlacement.dimension` 传入制作台放置函数。
- 补单测：放置 Workbench 后实体必须携带与玩家当前维一致的 `CurrentDimension`。

### P1：WorkbenchOpen 维度门禁

- 将 `handle_workbench_interact` 的玩家查询扩展为 `(&Position, Option<&CurrentDimension>)`，制作台查询扩展为 `(&Position, &WorkbenchBlock, Option<&CurrentDimension>)`。
- 在距离检查前比较维度；缺失维度按 `Overworld` 兜底以兼容旧实体，但新放置实体必须有显式维度。
- 跨维拒绝时只发明确聊天反馈，不下发 `WorkbenchOpen` payload，不播放打开音效。

### P2：制作台配方门禁同维过滤

- 将 `server/src/network/craft_emit.rs::apply_craft_intents` 的 `player_positions` / `workbenches` 查询补上 `CurrentDimension`。
- `has_nearby_workbench` 只允许同维且 3 格内的 `WorkbenchBlock` 命中。
- 补回归：TSY 玩家与主世界制作台同坐标时，要求制作台的 recipe 必须失败；同维 3 格内仍成功。

### P3：协议级 bot/e2e 覆盖

- 增加黑盒场景：用 dev 命令放置/记录制作台，切换玩家维度后通过 `bong:client_request` 发送伪造 `WorkbenchOpen`，断言不会收到 `workbench_open` payload。
- 增加 `CraftStart` 场景：跨维同坐标制作台不满足 station gate，同维制作台满足。

## 验证计划

- `cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`
- `BONG_SKIP_SKIN_PREFETCH=1 bash scripts/smoke-test-e2e.sh`
- 若补 bot 场景：`bash scripts/bot-e2e.sh`

## 对抗结论

- 第一轮质疑：`spawn_visual_marker` 会把制作台 marker 放在对应 layer，正常客户端只看当前 layer；但服务端 `WorkbenchOpen` 使用全局 `EntityManager::get_by_id`，layer 可见性不是 C2S 鉴权。
- 第一轮修正：复现不写成“官方客户端必然残留 id 触发”，而写成“伪造/复用 protocol entity_id 时服务端缺少维度门禁”。
- 第二轮质疑：目标必须真是 `WorkbenchBlock`，距离也必须在 3 格内；因此不是任意远程打开。
- 第二轮裁决：候选成立。制作台实体未携带/未校验 `CurrentDimension`，且制作门禁也未按维度过滤；应作为 server-gameplay 维度隔离 bug 修复。
