# plan-bughunt-spiritual-sense-cross-dimension-v1

> 来源 Issue：#1339、#1803。今天 `origin/main` 验真后仍成立；两条扫描报告指向同一遗漏：神识推送收集了全局目标，却没有把 `CurrentDimension` 作为目标集合过滤条件。

## §0 摘要

`push_spiritual_sense_targets` 为每个观察者收集所有带 `Client` 的玩家、拟态蛛、道伥和垂死大能。观察者虽然读取了自己的 `CurrentDimension`，但 `PlayerSenseSnapshot` 没保存目标维度，`build_player_sense_targets` 只排除观察者实体；三个 NPC 目标集合也只按距离过滤。坐标重叠时，主世界神识会收到 TSY 中玩家的境界、位置和伪装目标，违反跨维隔离。

本 plan 只写修复骨架，不修改代码、协议或客户端。

## §1 实际游玩体验影响

- 玩家在主世界施放神识时可能看到坍缩渊/TSY 内同坐标目标，泄露境界和精确位置。
- 道伥、拟态蛛或垂死大能也可能跨维出现在雷达条目中，造成错误追踪和跨玩家情报泄漏。
- 客户端无法从现有 `SpiritualSenseTargetsV1` payload 判断这是合法同维目标还是服务端误发。

## §2 复现路径

1. 在主世界与 TSY 各放置一个玩家，令两者坐标落在神识半径内；或在两维度放置一个带 `NpcMarker` 的目标。
2. 让主世界观察者满足 `should_scan`，运行 `push_spiritual_sense_targets`。
3. `player_sets.p0()` 收集两维度所有客户端；`build_player_sense_targets` 仅执行 `target.entity != observer`，随后按裸坐标计算距离。
4. 观察者收到另一维度的 `Cultivator(Realm)` / 目标位置条目。

## §3 根因证据

- `server/src/cultivation/spiritual_sense/push.rs:46-53` 的 `PlayerSenseSnapshot` 没有 `DimensionKind`；`:213-234` 因而把全局玩家快照直接建成目标。
- `server/src/cultivation/spiritual_sense/push.rs:262-295` 只把 `observer_dimension` 用于 `SpiritEyeRegistry::private_marker_entries`；玩家、蛛、道伥和垂死大能分支没有同维过滤。
- `server/src/cultivation/spiritual_sense/push.rs:353-375` 的 `build_player_sense_targets` 过滤条件只有实体不相等，距离函数接收跨维裸坐标。
- `server/src/cultivation/spiritual_sense/push.rs:75-91` 的蛛/道伥查询虽然已经读取 `Option<&CurrentDimension>`，但映射目标前没有用它；`:185-210` 的垂死大能同样如此。
- `server/src/cultivation/realm_vision/push.rs` 的 `private_marker_entries` 已按 `eye.dimension == observer_dimension` 过滤，证明同维是既有感知契约，而不是新设计。

## §4 非重复比对

- 未发现 `docs/plans-skeleton/`、active plan 或 finished plan 覆盖 `push_spiritual_sense_targets` 的玩家/NPC跨维过滤；灵眼私有标记的同维实现只是可复用先例。
- 这是服务端目标集合错误，不是 client HUD、Redis schema 或 agent narration 问题；不并入 `realm_vision` 的断线清理类 plan。
- #1339 和 #1803 的触发对象不同（NPC/玩家），但共享“目标快照没有维度、距离计算跨维”的单一根因，合并为一个修复 PR。

## 接入面与跨仓契约

- **Inputs**：`CultivationClock`、观察者 `CurrentDimension`、目标 `CurrentDimension`、`Position`、`Cultivation`、`SpiritualSensePushState`。
- **Outputs**：现有 `SpiritualSenseTargetsV1` 条目，只保留与观察者同一 `DimensionKind` 的目标。
- **共享类型 / 事件**：复用 `CurrentDimension`、`DimensionKind`、`SpiritualSenseTarget`、`SenseEntryV1`；不新建事件或 payload 字段。
- **server 契约符号**：`push_spiritual_sense_targets`、`PlayerSenseSnapshot`、`build_player_sense_targets`、`SpiritualSenseNpcSpiderReadItem`、`SpiritualSenseNpcDaoZhangReadItem`、`SpiritualSenseNpcDyingElderReadItem`。
- **agent**：无变更。神识 payload 不经 agent，server 仍发送同一 `ServerDataPayloadV1::SpiritualSenseTargets`。
- **client**：无变更。Fabric 端继续消费 `SpiritualSenseTargetsV1`，服务端过滤后 payload 形状不变。
- **worldview 锚点**：`docs/worldview.md §六 L517-L519`（神识是主动感知信息差）；`§十六 L1566-L1569`（坍缩渊负压环境使神识/感知失灵）。跨维目标不应绕过该空间边界。
- **qi_physics**：本问题不修改真元/灵气，不调用 ledger；现有目标 `Cultivation` 只用于境界展示，守恒断言不适用。

## §5 修复骨架

### P0 同维目标过滤

- 为 `PlayerSenseSnapshot` 保存目标 `CurrentDimension`（缺失时按既有实体可见性策略明确拒绝，不能默认为跨维可见）。
- 在 `build_player_sense_targets`、`build_niche_intrusion_trace_targets` 和三个 NPC 目标映射/过滤点传入 `observer_dimension`，先做同维判断，再做距离判断。
- 统一证明所有进入 `scan_targets_inner_ring` / `scan_targets_mid_ring_void` 的 `SpiritualSenseTarget` 都与观察者同维；`SpiritEyeRegistry` 的既有维度过滤保持不变。

### P1 契约回归

- 玩家同维/跨维、NPC 同维/跨维各保留一个行为测试；跨维条目不得出现在 inner/mid ring 缓存和最终 payload。
- 断言 payload 仍为 `SpiritualSenseTargetsV1`，不新增 agent/client 适配。
