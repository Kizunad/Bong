# BugHunt: tribulation_scorch 雷磁矿露头未物化为可挖矿脉

## Bug 摘要

`tribulation_scorch` 已归档 plan 明确承诺焦土地表 `mineral_density` hot-spot 可挖到 lodestone / copper / iron，是新手能够入手低阶矿的稀有地表入口。但当前 worldgen 只把这些露头写进 raster 的 `mineral_density` / `mineral_kind` 层，server 侧只把它们读进 `ColumnSample`，没有物化为 `MineralOreNode` / `MineralOreIndex`。

结果是：焦土地表看起来有雷磁矿、铜渣、铁渣语义，但 Bong 的矿物 runtime 不认为这些位置是矿脉。玩家无法通过感矿脉命中，也无法挖出带 `mineral_id` 的有限矿物资源。

这不是 `docs/plans-skeleton/plan-bughunt-mineral-anchor-position-drift-v1.md`：该 skeleton 处理固定 `mineral_anchors.json` 旧坐标漂移；本 bug 不改固定 anchor 坐标，而是 `tribulation_scorch` profile 的 raster 程序露头没有进入 runtime 矿物索引。

这也不主张补完 `plan-mineral-v1` 留待后续的全局“程序生成脉 / 品阶反比曲线”。本 bug 只针对 `plan-terrain-tribulation-scorch-v1` 已归档承诺的 lodestone / copper / iron 地表露头不可挖。

## 实际游玩体验影响

玩家到达烬焰焦土后，按 finished plan 预期应能在地表矿露头直接获得 lodestone / copper / iron，形成高风险但低门槛的早期资源入口。当前实际体验会断裂：地表可见焦土、雷磁柱、铜渣等地貌语义，但探矿请求只查 `MineralOreIndex`，这些 raster hot-spot 不在 index 中，返回 `NotMineralOre`。

即使玩家直接挖掉对应地表方块，矿物掉落路径也不会按 Bong mineral runtime 处理，不能产出带 `mineral_id` 的矿物、不会进入有限储量 / 耗竭 / 重生日志链路。焦土区域的资源奖励、风险回报和“新手稀有地表入口”语义因此落空。

## 证据定位

- `docs/finished_plans/plan-terrain-tribulation-scorch-v1.md:82`：明确写 `雷磁矿露头` 触发于地表 `mineral_density` hot-spot，效果是“表层即可挖到 lodestone / copper / iron（不需要凝脉感知）”，并称其为新手低阶矿入口。
- `docs/finished_plans/plan-terrain-tribulation-scorch-v1.md:231-239`：数值表给 `mineral_density` 配置渡劫坑核心、焦土主体、雷磁柱周围等富集区。
- `worldgen/scripts/terrain_gen/fields.py:269-279`：`mineral_density` 注释声明为矿物 ore-block 占据概率，`mineral_kind` 用于让 server 区分同 vanilla block 下的不同矿物。
- `worldgen/scripts/terrain_gen/profiles/tribulation_scorch.py:92-108`：profile 声明 extra layer 含 `mineral_density` / `mineral_kind`，notes 写明暴露 lodestone / copper / iron lightning deposits。
- `worldgen/scripts/terrain_gen/profiles/tribulation_scorch.py:263-276`：实际按 crater/static/lodestone/copper/iron mask 计算 `mineral_density` 与 `mineral_kind`。
- `worldgen/scripts/terrain_gen/profiles/tribulation_scorch.py:323-324`：把 `mineral_density` / `mineral_kind` 写进 raster buffer。
- `server/src/world/terrain/raster.rs:262-265`：`ColumnSample` 有 `mineral_density` / `mineral_kind` 字段。
- `server/src/world/terrain/raster.rs:1005-1006`：TerrainProvider 采样时读入这两个 raster layer。
- `server/src/world/terrain/mod.rs:798-805`、`server/src/world/terrain/mod.rs:833-855`：chunk 生成阶段只调用 `overlay_mineral_ores` 遍历 `MineralOreIndex`，不读取 `sample.mineral_density` / `sample.mineral_kind`。
- `server/src/mineral/anchors.rs:104-123`：`MineralOreIndex` 启动期物化来源是固定 anchor。
- `server/src/mineral/anchors.rs:123-165`：另一个来源是 whalefall fossil bbox。
- `server/src/mineral/mod.rs:11-12`：模块注释承认 raster `mineral_density/mineral_kind` 只是“后续可继续扩展同一 index”。
- `server/src/mineral/probe.rs:58-60`：感矿脉 miss `MineralOreIndex` 就返回 `NotMineralOre`。
- `server/src/world/block_drop.rs:191` 起的普通掉落路径只有在 `MineralOreIndex` 命中时才会交给 mineral handler。

## 触发路径

1. 通过 worldgen 生成包含 `tribulation_scorch` zone 的 raster manifest。
2. `fill_tribulation_scorch_tile` 在雷磁柱、铜渣、铁渣等区域写出 `mineral_density > 0` 与 `mineral_kind = 1/2/3`。
3. server 以 `BONG_TERRAIN_RASTER_PATH` 启动，`TerrainProvider` 成功加载这些 layer。
4. `mineral::spawn_mineral_anchor_nodes` 只根据 `mineral_anchors.json` 与 fossil bbox 写 `MineralOreIndex`，不会扫描 `tribulation_scorch` raster mineral layer。
5. 玩家走到焦土地表矿露头，对露头位置使用感矿脉或直接挖掘。
6. 探矿因 index miss 返回 `NotMineralOre`；挖掘也不会走有限矿脉掉落链路。

## 反方审查记录

### 第一轮质疑

反方尝试证明该层只是视觉层或未来扩展，不算 bug。审查结果：反驳失败。`fields.py` 的层注释直接写 Rust consumer 应采样并 roll；`tribulation_scorch.py` notes 写明这些 layer 暴露 lodestone / copper / iron deposits；`plan-terrain-tribulation-scorch-v1.md` 更明确承诺“表层即可挖到”。

反方同时确认 server 只把 layer 读入 `ColumnSample`，而矿块 overlay、探矿、掉落都只看 `MineralOreIndex`。`MineralOreIndex` 目前只有固定 anchor 与 fossil bbox 两个物化入口。

### 第二轮质疑

反方指出两个薄弱点：

1. `plan-mineral-v1` 曾把“密度曲线 vs 品阶反比的程序生成脉”留待后续，因此不能把本 bug 写成全局程序矿脉未实现。
2. `mineral_kind -> MineralId` 的 server palette 契约尚未落地，修复不能硬猜 u8 含义。

采纳上述收窄：本 plan 不主张补全全局程序矿脉，只针对后来的 finished `tribulation_scorch` gameplay 承诺。`mineral_kind` palette 缺失本身是修复前必须补的契约，不是否定 bug 的理由；`fields.py` 已说明该 layer 设计目标就是给 server 区分矿物。

最终裁决：候选成立，适合开单一 report-only plan PR。标题和范围应收窄为“`tribulation_scorch` 地表雷磁矿露头只导出 raster，未物化为可挖 `MineralOreNode`”。

## Skeleton Fix Plan

- [ ] 明确 `tribulation_scorch` 的 `mineral_kind` palette 契约：`1/2/3` 分别映射到 Bong `MineralId` 中的 lodestone / copper / iron 对应项，或把 profile 改为导出可被 server 明确解析的矿物 id metadata。
- [ ] 在 server 启动期新增只针对 `tribulation_scorch` / raster mineral layer 的物化路径，按 `mineral_density`、`mineral_kind`、地表高度和稳定 hash 生成有限数量 `MineralOreNode`，写入 `MineralOreIndex`。
- [ ] 物化逻辑必须接入现有 `ExhaustedMineralsLog`，避免挖空后重启复活。
- [ ] 物化逻辑必须避免覆盖固定 anchor 与 fossil bbox 已占用位置；`MineralOreIndex` 命中时跳过。
- [ ] 生成的矿脉 block 仍由 `overlay_mineral_ores` 统一写入 chunk，保持现有有限矿物可视化 / 掉落 / 探矿链路。
- [ ] 增加回归：构造带 `tribulation_scorch` mineral raster 的 `TerrainProvider` fixture，启动矿物物化后断言至少一个 lodestone/copper/iron 地表露头进入 `MineralOreIndex`。
- [ ] 增加负向回归：无 `mineral_density` 或 `mineral_kind = 0` 的列不得生成矿脉；已耗竭位置不得重生。
- [ ] 增加契约测试：`tribulation_scorch` exported raster 中 `mineral_density > 0` 的区域不会只有视觉方块，至少有可被探矿/采集命中的 runtime node。

## 验收测试计划

- [ ] `cd worldgen && python3 -m pytest tests/test_tribulation_scorch.py`，确保 profile 仍导出 `mineral_density` / `mineral_kind`。
- [ ] `cd worldgen && python3 -m scripts.terrain_gen --backend raster --zone-filter north_waste_east_scorch,blood_valley_east_scorch,drift_scorch_001 --output-dir /tmp/bong-scorch-mineral-check`，随后用 `worldgen/scripts/terrain_gen/harness/raster_check.py` 校验输出。
- [ ] `cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test mineral`。
- [ ] `cd server && cargo test tribulation_scorch`，确认焦土相关 runtime 回归仍通过。
- [ ] 集成验证：以包含焦土的 `BONG_TERRAIN_RASTER_PATH` 启服，定位一个 `mineral_density` hot-spot，感矿脉命中对应矿物，挖掘后产出带 `mineral_id` 的 Bong 矿物，并记录耗竭。

## 风险

- `lodestone/copper/iron` 需要映射到 Bong 现有 `MineralId`；不能直接把 vanilla block 名当作矿物 id。
- 如果按所有 `mineral_density` cell 直接物化，节点数量可能过大；需要稳定采样、上限和 chunk/zone 级预算。
- 如果只在 chunk 生成时临时 overlay，而不写 `MineralOreIndex` / `ExhaustedMineralsLog`，探矿与耗竭仍会断链。
- 不能顺手修 `mineral_anchors.json` 坐标漂移；那是相邻 skeleton 的范围。
