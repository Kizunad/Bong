# BugHunt: spawn_distribution 固定 safe_y 与真实地表漂移

## Bug 摘要

`server/zones.json` 的 `spawn` 区域把三个 `spawn_distribution` anchor 都固定为 `safe_y=72`，但按当前 `spawn_plain` profile / ColumnSpans 生成逻辑采样，三个出生圆盘中分别约有 11.4% / 22.7% / 43.1% 的列会得到 `surface_y == 72`。`spawn_selector` 在确定 X/Z 后没有查询 raster surface，而是直接把 `selected.safe_y` 作为 Valence 玩家脚点 `Position.y` 返回。

因此，落到这些列的初登或转世玩家会在首 tick 以 `Position.y=72` 出现在 y=72 地表完整碰撞块内部。现有 `recover_fall_through` 通常会在后续 tick 把玩家弹到 `surface+2`，所以这不是“必然永久卡死”，但它仍然是主路径出生坐标安全合同漂移。

## 实际游玩体验影响

- 新玩家初次进入世界或角色转世时，部分种子会先出生在地表方块内部，随后被恢复系统弹出，表现为首屏穿模、抖动、一次额外 chunk resend。
- 在 chunk / 视野同步竞态下，玩家可能短暂看到黑视野、空洞感或被突然回弹，第一印象像是出生点不稳定。
- fall recovery 自身也复用 `spawn_position_for_seed(..., FallRecovery)`；若兜底回 spawn 时再次落到 `surface_y == 72` 的列，会先进入同类错误姿态，再依赖下一轮恢复。
- 不能把影响夸大成稳定卡死：`recover_fall_through` 已有 `ResendAndBounce` 分支，会在检测到脚点陷入固体时弹到 `surface+2`。

## 证据定位

- `server/zones.json:704-733`：`spawn_distribution` 三个 anchor 的 `safe_y` 都是 `72.0`。
- `server/zones.worldview.example.json:18-39`：`spawn` blueprint 为 `spawn_plain`，范围 1500x1500，边界 width 96，height model `[66,78]`。
- `server/src/player/spawn_selector.rs:132-147`：selector 用 `selected.safe_y` 构造候选 `DVec3`，再返回 `[clamped.x, clamped.y, clamped.z]`，没有按最终 X/Z 查询 `SurfaceProvider`。
- `server/src/player/state.rs:450-464`：无 slow slice 或读取失败的新玩家路径调用 `spawn_position_for_seed(..., InitialLogin)`。
- `server/src/cultivation/character_select.rs:81-86`：转世新角色调用 `spawn_position_for_seed(..., NewLifeBirth)`。
- `worldgen/scripts/terrain_gen/profiles/spawn_plain.py:291-327`：`spawn_plain` 高度按真实 zone `center_xz` / `size_xz` 合成；profile 采样的三个出生圆盘 `surface_y == 72` 列比例约为 11.4% / 22.7% / 43.1%。这是 profile / ColumnSpans 采样证据，不是已部署 raster artifact 的精确统计。
- `worldgen/scripts/terrain_gen/bakers/raster_export.py:276-284` 与 `:358-363`：raster manifest 约定 `surface = span[0].ceiling_y`。
- `server/src/world/terrain/raster.rs:285-294`：runtime `ColumnSample::surface_y()` 读取 spans[0] 的 ceiling。
- `server/src/world/terrain/column.rs:263-272`：`world_y == column.top_y` 写入 surface block；完整方块上安全脚点至少应为 `surface_y + 1`，保守落点可用现有恢复口径 `surface_y + 2`。
- `server/src/world/terrain/mod.rs:332-391`：`recover_fall_through` 把 `Position.y` 当脚点，`floor(p.y)` 查脚所在体素；陷入固体时重发 chunk 并弹到 `surface + 2`。
- `server/src/player/spawn_selector.rs:257-308`：现有 spawn selector 测试只断言不同 seed 分布在 spawn zone 内，以及 `spawn_distribution` 可解析，没有断言 `spawn_pos.y > query_surface(x,z).y`。

## 触发路径

1. 玩家初次登录，或转世生成新角色。
2. `spawn_position_for_seed(seed, InitialLogin/NewLifeBirth)` 进入 `fallback_spawn`。
3. `load_default_spawn_distribution()` 从 `server/zones.json` 读取 `safe_y=72`。
4. selector 根据 seed 选中某个 anchor，并在圆盘内确定 X/Z。
5. 若该 X/Z 所在列按 `spawn_plain` / spans runtime 合同得到 `surface_y == 72`，selector 仍返回 `Position.y=72`。
6. 该列 y=72 是地表完整碰撞块，玩家脚点 `floor(72.0)=72`、`fract_y=0`，位于碰撞体内部。
7. 后续 `recover_fall_through` 可能检测到穿地并弹到 `surface+2`，玩家看到首屏抖动或回弹。

## 反方审查记录

第一轮对抗：

- 反方质疑是否重复 #1036。裁决：不重复。#1036 是 spawn 教学 POI 高度漂移；本候选是玩家出生 `spawn_distribution.safe_y` 与 runtime surface 合同漂移。
- 反方质疑 `Position.y` 是否为脚点。裁决：成立。Bong 的 `recover_fall_through` 明确用 `Position.y` 的 floor 查脚部体素，并按脚点碰撞判定。
- 反方质疑恢复系统是否洗掉 bug。裁决：只能降级，不能洗掉。恢复系统降低永久卡死风险，但首 tick 错误出生和额外弹出仍是玩家可见问题。
- 反方确认 `spawn_position_for_seed` 进入初登、存档 fallback、转世和 fall recovery 主路径。
- 反方确认 profile 采样与 runtime surface 语义一致，但要求把比例写成 profile / ColumnSpans 采样证据。

第二轮对抗：

- 反方要求避免把 11.4% / 22.7% / 43.1% 写成已烘焙 raster artifact 精确统计；最终文案改为“按当前 profile / ColumnSpans 逻辑采样”。
- 反方要求避免把列比例等同于种子概率；最终文案改为“落到这些列的种子”。
- 反方确认当 `surface_y == safe_y` 且 surface block 为完整碰撞块时，脚点位于该块碰撞体内部；安全脚点至少为 `surface_y + 1`。
- 反方要求把“黑视野”降级为竞态下可能出现的短暂现象，不写成必现。

最终裁决：

- 候选成立，高置信真实 bug。
- 影响等级按“首帧出生安全合同错误 + 恢复系统兜底回弹”描述，不按永久软锁描述。

## Skeleton Fix Plan

TODO:

- [ ] 把 spawn selector 的最终 Y 从静态 `safe_y` 改为“确定 X/Z 后查询 terrain surface”，返回 `surface_y + 1` 或项目统一保守口径 `surface_y + 2`。
- [ ] 保留 `safe_y` 作为 `TerrainProvider` 不可用时的 fallback，而不是正常 runtime 的权威落点。
- [ ] 给 `SpawnPurpose::InitialLogin`、`NewLifeBirth`、`FallRecovery` 复用同一套 surface-aware 落点逻辑，避免兜底救援再次落入同一个错误姿态。
- [ ] 明确非完整方块边界策略：完整碰撞块至少 `surface_y + 1`；若沿用恢复系统的保守口径，则统一为 `surface_y + 2`，避免半砖/路径/雪层等特殊碰撞体出现误判。
- [ ] 不在 `zones.json` 里手工抬高一组固定 `safe_y` 作为最终方案；固定数值会再次随 profile/raster 漂移。

## 验收测试计划

- [ ] server 单测：用 fake surface provider 构造 `surface_y=72`、anchor radius=0 的确定性分布，断言 spawn selector 不返回 `y=72`，而是返回 `>= surface_y + 1`。
- [ ] server 回归：覆盖 `InitialLogin`、`NewLifeBirth`、`FallRecovery` 三个 purpose，确认都走 surface-aware 落点。
- [ ] worldgen / server 契约测试：对当前 `server/zones.json` 的 spawn_distribution 采样一批 seeds，按 `floor(x/z)` 查询 surface，断言每个返回点 `spawn_pos.y > surface_y`。
- [ ] 测试不得只断言 `spawn_zone.contains(pos)`；必须显式断言脚点高于 runtime walkable surface。
- [ ] 若使用已烘焙 raster artifact 做集成测试，标明其来源，并把 profile 采样比例与 artifact 统计分开记录。
- [ ] 手工验证：选用命中 `surface_y == 72` 的 witness seed，例如 `offline:Azure` 或 `Bob`，初登/转世后不出现首屏穿地、回弹或额外恢复日志。

## 风险

- selector 当前不持有 `TerrainProvider`，修复可能需要调整 spawn API 或在调用点注入 provider；要避免把世界生成依赖反向塞进纯配置加载路径。
- `surface_y + 1` 与 `surface_y + 2` 的选择会影响出生手感：`+1` 更贴地，`+2` 更保守但可能出现轻微落地感。需要和现有恢复系统口径统一。
- 若 provider 不可用时继续 fallback 到 `safe_y`，仍要保留日志或测试，避免静默回到旧问题。
- 已有持久化玩家坐标不应被无条件重写；修复应只影响新出生、转世和 fall recovery 这类明确需要 spawn selector 的路径。
