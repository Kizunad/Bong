# BugHunt: spawn 教学 POI 高度漂移

## Bug 摘要

spawn_plain 当前地表已经抬到约 Y=71~72，但出生教学 POI 仍以旧的固定 Y=65/69 写入 worldgen manifest。`tutorial_lingquan`、`tutorial_chest`、`tutorial_rat_path` 会在运行时落到地表下约 6~7 格；只有 `tutorial_rogue_anchor` 在 server 消费时额外吸附到地表。

## 对实际游玩体验的影响

新玩家到达教学灵泉附近时，灵泉 marker 与开脉丹宝箱会出现在地下，而不是出现在地表引导路径上。灵泉到达 hook 目前按 XZ 距离判定，状态机可能还能推进；但教程宝箱是普通 `LootContainer`，客户端和服务端都按 3D 距离限制 5 格交互。当地表玩家站在宝箱正上方时，垂直差约 6~7 格，仍会被判定超出范围，导致玩家看不到也搜不到出生教学奖励。

## 证据定位

- `server/zones.worldview.example.json:77`、`:84`、`:91`、`:105`：spawn 教学灵泉、宝箱、鼠群路径仍手写为 Y=65。
- `worldgen/scripts/terrain_gen/profiles/spawn_plain.py:114`：fallback 灵泉坐标固定生成 `(x, 65.0, z)`。
- `worldgen/scripts/terrain_gen/profiles/spawn_plain.py:125`：`dynamic_lingquan_selector` 只有传入 `height` array 时才使用 `height + 1.0`。
- `worldgen/scripts/terrain_gen/profiles/spawn_plain.py:180`：`spawn_tutorial_pois_for_zone` 调用 `dynamic_lingquan_selector((center_x, center_z))`，没有传入 `height/qi_density/wx/wz`。
- `worldgen/scripts/terrain_gen/bakers/raster_export.py:562`：manifest POI 导出先保留 blueprint 手写 POI，再按 `(kind, name)` 去重跳过同名自动 POI，不会重算手写 Y。
- `server/src/world/terrain/raster.rs:764`：server `TerrainProvider` 原样读取 manifest `pois[].pos_xyz`。
- `server/src/world/spawn_tutorial.rs:481`、`:495`：`tutorial_lingquan` 和 `tutorial_chest` 直接使用 POI `pos_xyz` 生成 ECS `Position`。
- `server/src/world/spawn_tutorial.rs:509`：只有 `tutorial_rogue_anchor` 调用 `snap_spawn_y_to_surface`。
- `server/src/world/entity_model.rs:492`：容器可视实体用 `LootContainer` 的 `Position` 同步，无地表吸附。
- `server/src/network/tsy_container_search_emit.rs:181`：`container_state.world_pos` 直接由 `Position` 发给客户端。
- `client/src/main/java/com/bong/client/tsy/TsyContainerSearchIntentHandler.java:16`：客户端容器命中距离为 5 格，并使用 `TsyContainerView.distanceSq` 的 3D 坐标。
- `server/src/world/tsy_container.rs:181`、`server/src/world/tsy_container_search.rs:360`：服务端搜索半径为 5.0，且用 3D `distance` 拒绝超距请求。

轻量复现采样（只读调用 spawn_plain 单柱地形）：

| POI | 坐标 | 当前 surface | delta |
|---|---:|---:|---:|
| 半埋石棺 | `(0,69,0)` | 72 | -3 |
| 教学灵泉 #1 | `(50,65,100)` | 72 | -7 |
| 教学灵泉 #2 | `(-30,65,-80)` | 71 | -6 |
| 灵泉边小匣 | `(55,65,100)` | 72 | -7 |
| 踽行散修 | `(35,70,-45)` | 71 | -1，且 runtime 会 snap |
| 鼠群擦痕 | `(25,65,50)` | 72 | -7 |

## 触发路径

1. 生成 overworld raster manifest，默认 blueprint 来自 `server/zones.worldview.example.json`。
2. `_collect_poi_payload` 把手写 spawn 教学 POI 写入 manifest，并因 `(kind, name)` 已存在而跳过 `spawn_tutorial_pois_for_zone` 自动补同名 POI。
3. server 通过 `BONG_TERRAIN_RASTER_PATH` 加载 manifest，`TerrainProvider.pois()` 保留原始 Y。
4. `spawn_tutorial_poi_markers` 生成 `TutorialLingquan` 与教程 `LootContainer`，Position 仍为 Y=65。
5. 客户端在地下位置收到容器 visual 和 `container_state.world_pos`；玩家站在地表同 XZ 时 3D 距离超过 5 格，无法选中/搜索。

## 反方审查记录

### 第 1 轮

反方尝试证伪“导出时已有高度吸附”“server/client 另有 surface snap”“XZ-only hook 足够消除影响”“开放 PR/已有 plan 重复”。结论：通过。导出链路保留手写 POI；灵泉/箱子没有 snap；XZ-only 只能保住部分 hook，不能修复实体可见性与宝箱交互；开放 PR 未发现同题。

### 第 2 轮

反方继续攻击“教程箱子是否真走通用 LootContainer”“3D 半径是否足以阻断”“灵泉不可见是否只是弱体验”“没有 checked-in generated manifest 是否削弱”。结论：通过。教程箱子确实是 `StoragePouch` LootContainer；客户端和服务端都是 5 格 3D 半径；generated manifest 不入库不影响 worldgen/export/server bootstrap 契约；灵泉问题降级为体验弱化，但宝箱交互失败是硬 bug。

## Skeleton Fix Plan

- [ ] 在 worldgen 侧明确 spawn 教学 POI 的高度来源：要么让 `spawn_tutorial_pois_for_zone` 在导出期拿到对应地形高度并输出 `surface + 1`，要么删除/迁移手写固定 Y POI，避免同名去重锁死旧坐标。
- [ ] 给 manifest/raster harness 增加 spawn 教学 POI 高度契约：`tutorial_lingquan`、`tutorial_chest`、`tutorial_rat_path`、`spawn_tutorial_coffin` 的 Y 必须在当前位置 surface 附近，超出 1~2 格直接报错。
- [ ] 在 server 消费端增加防御性吸附或拒绝：`tutorial_lingquan`、`tutorial_chest`、`spawn_tutorial_coffin` 与鼠群锚点不得直接信任 manifest 旧 Y。
- [ ] 补一条客户端/服务端交互回归：教程宝箱在地表同 XZ 可被选中并开始搜索，且不会因垂直差被 `OutOfRange` 拒绝。

## 验收测试计划

- worldgen：对 spawn zone 的教学 POI 跑单柱 surface 采样，断言 POI Y 与 `spans` surface 的差值在允许范围内。
- raster harness：读取 manifest `pois` 后，对地表相关 POI 执行 POI-vs-surface 校验，覆盖手写 POI 和自动补 POI。
- server：构造带低 Y 教学 POI 的 `TerrainProviders` fixture，验证 `spawn_tutorial_poi_markers` 生成的灵泉/宝箱/棺材 Position 被吸附到地表或被明确拒绝。
- client/server 协议：教程 `LootContainer` 的 `container_state.world_pos` 与 visual 坐标在地表附近；玩家同 XZ 站位能通过 5 格 3D 搜索半径。
- e2e：新玩家出生后可在地表看见并搜索灵泉边小匣，能拿到开脉丹，灵泉/鼠群引导不要求挖地下。

## 风险

- 如果只在 server 端吸附，worldgen manifest 仍会继续输出坏数据，后续离线校验和其它 POI 消费者仍可能踩坑。
- 如果只在 worldgen 端修导出，旧 manifest 或运维未重烘焙时仍会保留坏坐标，需要 server 防御性校验兜底。
- 地表吸附需要处理水面、洞穴、多 span 列和半埋石棺的“半埋”语义，不能简单把所有 POI 都推到完全同一高度。
- 修改 POI Y 后可能改变新手路径节奏，需要回归棺材、灵泉、宝箱、散修、鼠群之间的距离和引导顺序。
