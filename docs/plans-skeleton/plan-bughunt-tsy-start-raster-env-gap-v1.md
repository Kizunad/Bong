# plan-bughunt-tsy-start-raster-env-gap-v1（骨架）

## Bug 摘要

`scripts/start.sh` 是 Windows 客户端与部署提示里的常规整栈启动入口，但它只探测并导出主世界 `BONG_TERRAIN_RASTER_PATH`，没有把已生成的 TSY raster manifest 导出为 `BONG_TSY_RASTER_PATH`。服务端启动后 `TerrainProviders.tsy` 因此保持 `None`，即使 `worldgen/generated/terrain-gen-tsy/rasters/manifest.json` 已经存在。

这不是 #971 的矿脉固定锚点旧坐标漂移问题，也不是 #986 的 `giant_sword_sea` / `wuxing_abyss` AABB 重叠问题；本 bug 位于常规启动脚本与服务端 TSY provider 环境变量契约之间。

## 对实际游玩体验的影响

玩家按常规流程运行 `bash scripts/start.sh` 后，主世界仍会加载真实 raster，主世界 TSY entry portal 也能从 overworld POI 生成。玩家进入传送门时会被传送到 TSY 维度坐标，但 TSY provider 没有加载，chunk 生成系统会跳过整个 TSY 维度，表现为坍缩渊没有地形、路径、出口传送门、容器、NPC 或遗迹 POI。实际体验是玩家能进秘境入口，却落入空洞/不可游玩的 TSY 维度。

## 证据定位

- `scripts/start.sh:23-31` 只检查 `worldgen/generated/terrain-gen/rasters/manifest.json`，并只设置 `BONG_TERRAIN_RASTER_PATH`。
- `scripts/start.sh:60-69` server pane 只导出 `BONG_TERRAIN_RASTER_PATH`，没有导出 `BONG_TSY_RASTER_PATH`。
- `scripts/dev-reload.sh:29-42` 已经支持生成 TSY raster 到 `generated/terrain-gen-tsy`。
- `scripts/dev-reload.sh:101-107` 在 TSY manifest 存在时会追加 `BONG_TSY_RASTER_PATH=$TSY_MANIFEST_ABS`，说明双 manifest runtime 契约已经存在。
- `server/src/world/terrain/mod.rs:572-579` 将 TSY provider 从 `load_tsy_provider_from_env` 加载进 `TerrainProviders.tsy`。
- `server/src/world/terrain/mod.rs:583-619` `load_tsy_provider_from_env` 唯一读取 `BONG_TSY_RASTER_PATH`；未设置或不可读时返回 `None`。
- `server/src/world/terrain/raster.rs:421-442` `TerrainProviders.tsy` 是 `Option<TerrainProvider>`，`for_dimension(DimensionKind::Tsy)` 在缺 provider 时返回 `None`。
- `server/src/world/terrain/mod.rs:656-663` chunk 生成按维度遍历；某维度 provider 为 `None` 时直接 `continue`，因此 TSY 不生成 chunk。
- `server/src/world/tsy_poi_consumer.rs:90-148` entry portal 只依赖 overworld provider，仍会在主世界生成。
- `server/src/world/tsy_poi_consumer.rs:151-157` TSY provider 缺失时明确跳过 exit portals。
- `server/src/world/tsy_portal.rs:116-121` entry portal 会发送 `DimensionTransferRequest` 到 TSY。
- `server/src/world/dimension_transfer.rs:72-76` 传送系统切换 layer、位置和当前维度，不会因为 TSY provider 缺失而拒绝传送。
- `scripts/windows-client.md:5-6` 常规 Windows 客户端流程要求在 WSL 中运行 `bash scripts/start.sh`。
- `scripts/deploy.sh:35` 部署完成后也提示运行 `bash scripts/start.sh`。
- `docs/finished_plans/plan-tsy-worldgen-v1.md:321` 设计契约写明 server 启动应读 `BONG_TERRAIN_RASTER_PATH` 与 `BONG_TSY_RASTER_PATH` 两个环境变量。

## 触发路径

1. 先通过 `bash scripts/dev-reload.sh` 或 worldgen pipeline 生成主世界与 TSY rasters，使 `worldgen/generated/terrain-gen/rasters/manifest.json` 与 `worldgen/generated/terrain-gen-tsy/rasters/manifest.json` 均存在。
2. 按常规流程重新启动整栈：`bash scripts/start.sh`。
3. 服务端只收到 `BONG_TERRAIN_RASTER_PATH`，没有收到 `BONG_TSY_RASTER_PATH`。
4. `TerrainProviders.tsy=None`。
5. 玩家在主世界进入 TSY entry portal。
6. 传送系统把玩家移动到 TSY layer 与目标坐标。
7. TSY chunk 生成循环因 `providers.for_dimension(DimensionKind::Tsy)` 为 `None` 被跳过；TSY exit portals、容器、NPC、遗迹也因缺 provider 未生成。

## 反方审查记录

审查 subagent：`019f3752-0b23-7e53-b3d1-81df24d1a131`。

第一轮质疑：

- 质疑点：`dev-reload.sh` 已经传入 `BONG_TSY_RASTER_PATH`，是否说明实际开发入口不会触发？
- 结论：不成立。`scripts/start.sh` 仍是 Windows 客户端与部署后的常规整栈启动入口；`dev-reload.sh` 的正确接线反而证明 `start.sh` 与 runtime 契约漂移。

第二轮质疑：

- 质疑点：缺少 TSY provider 是否只是降级行为，不影响实际玩家？
- 结论：不成立。entry portal 从 overworld provider 生成，玩家仍能进入 TSY；缺 provider 只阻止 TSY chunk 与 TSY POI 生成，不阻止跨维度传送，所以体验会从“入口可用”断到“秘境空洞”。

反方最终结论：通过。该问题高置信、可由常规启动路径触发，且不重复 #971 / #986。

## Skeleton Fix Plan

1. 在 `scripts/start.sh` 中按 `scripts/dev-reload.sh` 的既有目录约定探测 `worldgen/generated/terrain-gen-tsy/rasters/manifest.json`。
2. 当 TSY manifest 存在时导出 `BONG_TSY_RASTER_PATH` 到 server pane；不存在时打印清晰 warning，但保持主世界启动可用。
3. 启动日志中同时打印主世界与 TSY raster manifest 状态，避免玩家误以为 TSY 已加载。
4. 增加脚本级回归测试或 shellcheck 风格验证，断言 `start.sh` 同时包含主世界与 TSY manifest 探测、导出逻辑。
5. 增加服务端/脚本集成验证：在两个 manifest 均存在的 fixture 下启动 server 环境构造，确认 TSY provider 不为 `None`，且 TSY chunk 生成路径不会被 provider 缺失分支跳过。

## 验收测试计划

- `bash scripts/dev-reload.sh --skip-regen --skip-validate` 或等价 root 级流程：确认不破坏已有 dev-reload 双 manifest env 行为。
- `bash scripts/start.sh` dry-run/脚本测试：两个 manifest 均存在时 server pane 命令包含 `BONG_TERRAIN_RASTER_PATH` 与 `BONG_TSY_RASTER_PATH`。
- 缺少 TSY manifest 时：`start.sh` 应打印 TSY warning，主世界仍按原逻辑启动，不把空路径误传成有效 TSY provider。
- `cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`。
- root e2e：`BONG_SKIP_SKIN_PREFETCH=1 bash scripts/smoke-test-e2e.sh`，覆盖玩家进入 TSY 后可以生成 chunk、看到 exit portal/容器/NPC/遗迹 POI 的链路。

## 风险

- `start.sh` 当前构造 tmux pane 命令，新增 env 需要正确处理路径引用，避免空格或单引号破坏 shell 命令。
- TSY manifest 不存在时不能让 server 因必需 env 缺失而退出；应保持现有 legacy 降级，但日志必须明确“TSY 未加载”。
- 若未来 TSY 输出目录改名，`start.sh` 与 `dev-reload.sh` 可能再次漂移；后续修复应尽量复用同一目录常量或补测试锁定路径契约。
