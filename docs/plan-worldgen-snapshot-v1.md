# Bong · plan-worldgen-snapshot-v1 · 骨架

**Worldgen 视觉快照 CI**：PR 修改 `worldgen/**` / `server/zones*.json` 时（或手动 `@preview-worldgen`），CI 启动 server + 真 Fabric client（headless），按预设 5 角度截图，配合 JSON 配置的"装饰方块"层（地名木牌 / zone 界碑 / 灵脉柱），将 5 张图打包为 artifact 并贴 PR comment；可与 base ref 自动对比。目标是把"读 raster 数字判断地形"换成"看真游戏画面"。

**世界观锚点**：`worldview.md §九 距离衰减章 / §十 地理章`（地名 / zone 划分）· `worldview.md §六.2 缜密色 / §六.3 凝实色`（装饰方块的色调取材）

**library 锚点**：装饰木牌文案取自 `docs/library/places/*`（zone 描述 / POI 命名）；现阶段先用 `zones.worldview.example.json` 的 `display_name` / `pois.name` 兜底

**交叉引用**：
- `plan-worldgen-v3.x`（前置；本 plan 不动 LAYER_REGISTRY 或 raster_export，只在其上加观测层）
- `plan-client`（Fabric 微端；新 mod 模块 `com.bong.client.preview`）
- `e2e.yml`（已有 server+agent 端到端 smoke；本 plan 引入 client 进 CI 是首次）
- `plan-network-v1`（client↔server connection；预览 mod 用同套连接路径）

**触发模型**：
- `pull_request` 自动：`paths: worldgen/**` / `server/zones*.json` / `worldgen-preview.yml` 自身
- `workflow_dispatch` 手动重跑
- `issue_comment` 含 `@preview-worldgen` 在 PR 评论触发（权限：collaborator+，§10 待最终确认）

**阶段总览**：
- P0 ⬜ 截图 harness 单角度通链（俯视一张，CI 链路骨架）
- P1 ⬜ 多角度（5 张：俯视 + 等角 NE/NW/SE/SW）+ 装饰方块层（JSON 驱动）
- P2 ⬜ PR 投递（artifact + comment）+ 与 base ref 视觉 diff

---

## §0 设计轴心

- [ ] **真 client 渲染，非 raster 投影**：截图来自 `./gradlew runClient` 启动的 Fabric client 飞过区域、`ScreenshotRecorder` 拍下的真游戏画面，不走 matplotlib / Chunky 路径追踪
- [ ] **复用项目现成 pattern**：client 端走 `com.bong.client.weapon.WeaponScreenshotHarness` 范式（env var 激活 + ClientTickEvents 状态机 + `ScreenshotRecorder.takeScreenshot` + `client.scheduleStop()`）；server 端走 `world::tsy_container_spawn::load_tsy_container_spawn_registry`（`server/src/world/tsy_container_spawn.rs:74`）范式 + `ChunkLayer::set_block(pos, Block { state, nbt })` spawn 装饰
- [ ] **配置驱动**：相机预设、装饰方块布局都走 JSON（`worldgen/preview/*.json`），不在 mod 内 hardcode；预设可用 generator 从 zones blueprint 派生
- [ ] **装饰是世界观的一部分**：地名木牌 / 灵脉柱 / 境界界碑既是 PR 视觉标注，又是未来 worldgen 可选的"地标层"——P1 仅在 preview 模式 spawn，但数据结构允许未来 promote 到正式产物
- [ ] **CI 资源不紧**：3000 mins 配额下，单 PR 5 张图 + base 对比 ≈ 8 min 可接受；不为了省时间牺牲画质
- [ ] **每 P 独立可发**：P0 通链就有价值（哪怕只有一张俯视图也比没有强）；P1/P2 可分批落地
- [ ] **与现有 e2e.yml 解耦**：新 workflow `worldgen-preview.yml`，不挂在 e2e 主链路上（避免 client headless 偶发失败拖累已稳定的 e2e）

---

## §1 P0 — 截图 harness 单角度通链

> 目标：CI 跑通 `server (headless) + client (headless) → 单张俯视 PNG → upload-artifact`，不要求装饰、不要求多角度、不要求 PR comment。先把 client headless 这条链路从死变活。

### 1.1 Workflow 文件

- [ ] **新增** `.github/workflows/worldgen-preview.yml`
  - 触发：`pull_request` (paths: `worldgen/**` / `server/zones*.json` / 自身) + `workflow_dispatch` + `issue_comment` 含 `@preview-worldgen`
  - jobs.snapshot：runs-on `ubuntu-latest`，timeout 30min
  - 步骤序：checkout → setup-java(17) / setup-rust → cache → install mesa + xvfb → start server (background) → wait server ready (TCP probe :25565) → run client headless → upload-artifact
  - artifact name: `worldgen-snapshot-${{ github.event.pull_request.number || github.run_id }}`，path `client/run/screenshots/preview-*.png`

### 1.2 Server headless 启动脚本

- [ ] **新增** `scripts/preview/run-server-headless.sh`
  - 内部：`cd server && cargo run --release` 后台 + 写 PID 到 `/tmp/bong-preview-server.pid`
  - server 已是 offline mode + 不需 Mojang 认证（CLAUDE.md `cargo run` 注释）
  - 等待 ready 信号：TCP probe `127.0.0.1:25565` accept 即视为 ready（Valence 监听后即可）
  - 不需 Redis（preview 不挂 agent 链路；agent 关掉以省时间）

### 1.3 Client headless 启动 + Fabric 截图 harness mod

- [ ] **新增包** `client/src/main/java/com/bong/client/preview/`
  - `PreviewHarnessClient` — `ClientModInitializer`，仅在 system property `bong.preview.harness=true` 时激活，避免污染普通 `runClient`
  - `PreviewConfig` — record，从 `client/preview-harness.json` 读取（路径可被 `-Dbong.preview.config=...` 覆盖）
  - `PreviewSession` — 状态机：`WAIT_CONNECT → WAIT_CHUNKS_LOADED → SCREENSHOT_LOOP → EXIT`
- [ ] **screenshot 实现**：调 `net.minecraft.client.util.ScreenshotRecorder.saveScreenshot(File runDir, String fileName, Framebuffer fb, Consumer<Text> messageReceiver)`；输出到 `client/run/screenshots/preview-<camera_name>.png`
- [ ] **camera 控制**：mod 通过 `MinecraftClient.getInstance().player.setYaw/Pitch` + `/tp` 命令（OP 权限 via `connect_dev_op` 或 server 测试模式）
- [ ] **退出**：截图全部成功 → `MinecraftClient.scheduleStop()`；任一步骤超时（默认 60s/角度）→ exit code 非 0 让 CI 红
- [ ] **gradlew 调用**：`./gradlew runClientHeadless` 自定义 task（继承 `runClient` + 加 `-Dbong.preview.harness=true` + LWJGL 软渲染开关）

### 1.4 Headless 渲染依赖（2026-04-28 调研后修订）

- [ ] **runs-on `ubuntu-24.04`**：xvfb 21.1.12 预装，**无需 apt install** 任何额外包；mesa llvmpipe 支持 OpenGL 4.5 core，远超 MC 1.20.1 要求的 3.2 core
- [ ] **xvfb 包装**：当前 fabric-loom `1.6-SNAPSHOT` **未包含**自动 `useXVFB`（main 分支才有），手动包：`xvfb-run -a --server-args='-screen 0 1280x720x24' ./gradlew runClient ...`。未来升 loom > 1.7 后可去掉手动包装
- [ ] **官方参照已实跑**：`FabricMC/fabric-api/.github/workflows/build.yml` 的 `client_test` job 同模式（ubuntu-24.04 + 裸 `./gradlew` + upload `run/screenshots`），无须特殊 JVM args；问题只在 1.20.1 没 backport `runClientGametest` 框架，必须自写 mod 钩子（项目已有先例 `WeaponScreenshotHarness`）
- [ ] **不再需要的措施**：之前担心的 `-Dorg.lwjgl.opengl.libname=...` / `-Dfabric.client.gl.context=mesa` —— 均不需要，xvfb + 默认 mesa 即足

### 1.5 P0 单角度配置

- [ ] `client/preview-harness.json`（最小版本）：
  ```json
  {
    "server": "127.0.0.1:25565",
    "username": "PreviewBot",
    "wait_chunks_radius": 8,
    "screenshots": [
      { "name": "top", "tp": [0, 320, 0], "yaw": 0, "pitch": -90 }
    ],
    "exit_on_complete": true
  }
  ```
- [ ] 命中位置 `(0, 320, 0)` = spawn zone 中心高空俯视

### 1.6 P0 测试与验收

- [ ] **本地通跑**：`bash scripts/preview/run-server-headless.sh & ./gradlew runClientHeadless` → 30s 内产出 `client/run/screenshots/preview-top.png`
- [ ] **CI 通跑**：PR 改 `worldgen/scripts/terrain_gen/fields.py` → workflow 触发 → artifact 内有 `preview-top.png` 且非空（>10KB）
- [ ] **失败信息**：server 没起来 / client 连不上 / xvfb 不工作 / 截图为空 → log 里有清晰 marker（不是 `Process exited 1` 一行带过）
- [ ] **不在乎**：图好不好看、有没有装饰、多角度——P0 只验链路

---

## §2 P1 — 多角度（5 张）+ 装饰方块层

> 目标：把 P0 的单角度扩到 5 角度（俯视 + 等角 NE/NW/SE/SW），并在截图前由 server 端按 JSON 配置 spawn 装饰方块（地名木牌 + zone 边界标记 + 灵脉柱）。

### 2.1 相机预设

- [ ] **新增** `worldgen/preview/cameras.json`（配置文件，CI 读取）：
  ```json
  {
    "presets": [
      { "name": "top",     "tp": [0, 320, 0],   "yaw": 0,    "pitch": -90 },
      { "name": "iso_ne",  "tp": [-400, 200, -400], "yaw": 135,  "pitch": -30 },
      { "name": "iso_nw",  "tp": [400, 200, -400],  "yaw": -135, "pitch": -30 },
      { "name": "iso_se",  "tp": [-400, 200, 400],  "yaw": 45,   "pitch": -30 },
      { "name": "iso_sw",  "tp": [400, 200, 400],   "yaw": -45,  "pitch": -30 }
    ]
  }
  ```
- [ ] camera tp 坐标随 blueprint world bounds 自适应（`worldgen/scripts/preview/gen_cameras.py` 从 `world.bounds_xz` 派生 4 个等角点）
- [ ] mod 端：`PreviewSession` 按 list 顺序拍，每张之间等 chunks 加载稳定（`wait_chunks_radius` ticks）

### 2.2 装饰方块 JSON

- [ ] **新增** `worldgen/preview/decorations.json`（配置文件，gen 出来 + 可手编）：
  ```json
  {
    "version": 1,
    "items": [
      {
        "kind": "sign",
        "pos": [0, 80, 0],
        "block": "minecraft:oak_sign",
        "lines": ["初醒原", "spirit_qi 0.3", "danger 1", ""]
      },
      {
        "kind": "pillar",
        "pos": [0, 75, 0],
        "block": "minecraft:end_rod",
        "height": 12
      },
      {
        "kind": "boundary_marker",
        "aabb_min": [-750, 70, -750],
        "aabb_max": [750, 70, 750],
        "block": "minecraft:soul_lantern",
        "stride": 64
      }
    ]
  }
  ```
- [ ] **支持的 kind**：
  - `sign`：4 行木牌（MC 限制 4×15 字符），用于 zone 显示名 + 关键属性
  - `pillar`：竖直方块柱，用于地标可见性增强（远距离也能看到 zone 中心）
  - `boundary_marker`：AABB 边界点阵 spawn，沿矩形边按 stride 步长摆方块（zone 边界一目了然）

### 2.3 装饰生成器

- [ ] **新增** `worldgen/scripts/preview/gen_decorations.py`
  - 输入：`server/zones.worldview.example.json`
  - 自动生成：
    - 每个 zone 的中心 sign（取 `display_name` + `spirit_qi` + `danger_level`）
    - 每个 zone 中心 pillar（高度按 `danger_level` 1-5 调）
    - 每个 zone aabb 的 boundary_marker
    - 每个 POI 的 sign（取 `pois[].name` + `kind`，木牌位置 = `pos_xyz` 上方）
  - 输出：`worldgen/preview/decorations.json`
  - 可选：`--manual extra-decorations.json` 合并手编装饰（地标补强）

### 2.4 Server 端装饰加载

- [ ] **新增** `server/src/preview/mod.rs` + `server/src/preview/decorations.rs`
  - 仅在启动参数 `--preview-mode` 或 env `BONG_PREVIEW=1` 时启用（普通 `cargo run` 不加载装饰）
  - 启动期 system：读 `worldgen/preview/decorations.json` → 按 kind 派发到 `spawn_sign` / `spawn_pillar` / `spawn_boundary` ECS commands
  - sign 文本写入：用 valence 的 sign block entity NBT；中文支持需确认 valence 当前版本（pinned `2b705351`）的 sign API
- [ ] **新增** `scripts/preview/run-server-headless.sh` 加 `--preview-mode` flag

### 2.5 P1 测试与验收

- [ ] **本地**：`BONG_PREVIEW=1 cargo run --release` + 手动 client 进服 → 看到 spawn zone 中心有"初醒原"木牌 + end_rod 灯柱 + soul_lantern 边界点阵
- [ ] **gen_decorations.py 单测**：feed minimal zones JSON（2 zones + 1 POI）→ 输出 decorations 含 6 个 sign + 2 pillar + 2 boundary_marker
- [ ] **CI 通跑**：artifact 内有 `preview-top.png` / `preview-iso-{ne,nw,se,sw}.png` 共 5 张，每张大小合理（>50KB）；俯视图能肉眼看到至少 3 个 zone 的中心标记
- [ ] **失败信息**：装饰 JSON 解析失败 / sign 文本 NBT 写错 → 启动期 panic 而非默默跳过

---

## §3 P2 — PR 投递 + base ref 对比

> 目标：5 张图打包成一张总览，贴 PR comment；可选与 base ref 跑同样 5 张做视觉 diff（SSIM 高亮）。

### 3.1 总览拼图

- [ ] **新增** `scripts/preview/compose_grid.py`
  - 输入 5 张 PNG → PIL 拼成 1 张总览（建议 1 大俯视 + 4 小等角的 2×3 mosaic）
  - 装饰：每张子图加 caption（"top" / "iso_ne" 等）+ 时间戳 + commit short sha
  - 输出：`client/run/screenshots/preview-grid.png`

### 3.2 PR comment

- [ ] **新增** `scripts/preview/post_comment.py`
  - 用 `gh api` 或 actions/github-script 上传 grid.png 到 PR + 评论 markdown
  - 评论模板包含：commit sha、5 张图的 thumbnail（`<img src="...artifact url..." width=200>`）、artifact 下载链接
  - 防刷：同 PR 已存在 `[bong-snapshot]` 评论时编辑而非新发（marker tag 在评论首行）

### 3.3 与 base ref 的视觉 diff

- [ ] **workflow job 拆分**：snapshot-head（PR HEAD）+ snapshot-base（merge-base）+ diff（聚合）
- [ ] **diff 工具**：`scripts/preview/ssim_diff.py` — `scikit-image.metrics.structural_similarity`，对每张图做对比，输出
  - SSIM 分数（每张一个）+ diff heatmap（红高绿低）
  - 总览拼图：head | base | heatmap 三列对照
- [ ] **PR comment 升级**：含 SSIM 分数表 + diff overlay 链接；若所有 SSIM > 0.95 直接标记 "no visual change detected"，避免吵

### 3.4 触发与权限

- [ ] **`@preview-worldgen` comment trigger**：`issue_comment` event + `if: contains(github.event.comment.body, '@preview-worldgen')` + 限制 `github.event.comment.user.login` 在 collaborator 列表
- [ ] **超时与并发**：单 PR 同时只跑一个 snapshot（`concurrency.group: snapshot-${{ github.event.pull_request.number }}`，cancel-in-progress: true）

### 3.5 P2 测试与验收

- [ ] PR 改 `worldgen/scripts/terrain_gen/fields.py` 中某个 zone 的高度模型 → comment 内 SSIM 在该 zone 对应的角度图上明显下降 + heatmap 红斑命中 zone 区域
- [ ] PR 改 `docs/`（不该触发） → workflow 不跑
- [ ] 评论 `@preview-worldgen` → workflow 触发；非 collaborator 评论同样字符串 → 不触发

---

## §4 数据契约（按 P 汇总，下游 grep 抓手）

| P | 契约 | 位置 |
|---|------|------|
| P0 | `.github/workflows/worldgen-preview.yml` workflow | 仓库根 `.github/workflows/` |
| P0 | `PreviewHarnessClient` mod entry + `PreviewConfig` + `PreviewSession` | `client/src/main/java/com/bong/client/preview/` |
| P0 | `client/preview-harness.json` schema（server / username / screenshots[]） | `client/` |
| P0 | `scripts/preview/run-server-headless.sh` | `scripts/preview/` |
| P0 | `./gradlew runClientHeadless` task | `client/build.gradle` |
| P1 | `worldgen/preview/cameras.json` 5 角度预设 | `worldgen/preview/` |
| P1 | `worldgen/preview/decorations.json` schema（kind = sign/pillar/boundary_marker） | `worldgen/preview/` |
| P1 | `worldgen/scripts/preview/gen_decorations.py` 生成器 | `worldgen/scripts/preview/` |
| P1 | `server/src/preview/mod.rs` + `--preview-mode` flag | `server/src/preview/` |
| P2 | `scripts/preview/compose_grid.py` / `ssim_diff.py` / `post_comment.py` | `scripts/preview/` |
| P2 | PR comment marker tag `[bong-snapshot]` | comment 首行 |
| P2 | `concurrency.group: snapshot-<PR#>` | workflow |

---

## §5 实施节点

- [ ] **P0** 单角度通链 — workflow 文件 + headless server 脚本 + Fabric 截图 mod + `runClientHeadless` task + 一张俯视 artifact
- [ ] **P1** 多角度 + 装饰 — 5 角度相机预设 + decorations JSON + generator + server `--preview-mode` 加载装饰 + 5 张 artifact
- [ ] **P2** PR 投递 + diff — grid 拼图 + comment + base ref SSIM diff + `@preview-worldgen` 触发器

---

## §6 开放问题

### 已敲定（2026-04-28 调研后）

- [x] ~~**GLFW headless 可行性**~~ ✅ 可行：ubuntu-24.04 预装 xvfb + mesa llvmpipe（OpenGL 4.5 core），FabricMC/fabric-api 官方 CI 同模式实跑；当前 loom `1.6-SNAPSHOT` 未自带 `useXVFB`，手动 `xvfb-run -a ./gradlew runClient` 即可
- [x] ~~**装饰中文支持**~~ ✅ valence `2b705351` 完全支持 UTF-8 中文 sign：`compound!` 宏 + `into_text()`，参考 `~/.cargo/git/.../valence/examples/block_entities.rs:54`
- [x] ~~**`@preview-worldgen` 权限**~~ ✅ collaborator+（外部贡献者 PR 已自动触发，手动 trigger 不开放）
- [x] ~~**`--quickPlayMultiplayer` 1.20.1**~~ ✅ 支持（23w14a 引入，1.20.1 携带，vanilla `Main.main` 解析无需 Fabric 拦截）

### 仍待决

- [ ] **runClient 自然退出**：1.20.1 没有 fabric-api `runClientGametest` 框架（≥ 1.21.4 才有），`./gradlew runClient` 不会自然退出。必须 mod 内 `MinecraftClient.scheduleStop()`（项目已用，见 `WeaponScreenshotHarness.java:200`）；外加 workflow timeout 兜底
- [ ] **装饰是否进 worldgen 正式产物**：现在装饰只在 `--preview-mode` spawn；未来若想让玩家也能在普通游戏里看到地名木牌，需要把 decorations 升级成 raster export 的一部分（增加 `landmark` 通道）—— 本 plan 暂不做，作 v2 候选
- [ ] **base ref 对比的语义**：用 `merge-base(HEAD, main)` 还是 `pull_request.base.sha`？前者更准（避免 rebase 后 base 漂移），后者更省事
- [ ] **SSIM 阈值**：> 0.95 标记 "no change" 的阈值合不合理，需 P2 上线后调
- [ ] **多 zone scaling**：现在 5 角度是"全图视角"，如果 zone 超过 ~10 个，等角图会缩太小看不清细节。是否需要 per-zone 的 5 角度集（5 × N 张）？资源够，但 PR comment 会爆——倾向 P1 先全图 5 张，P3 再加按 zone 抽样
- [ ] **agent 是否参与**：preview 模式下 agent 关掉省时间。但有些 worldgen 测试可能需要 agent narrative 一起看效果（如灾劫地形改造）—— 现阶段不做，未来 plan-snapshot-v2 候选
- [ ] **cache 策略**：client jar 编译产物 + cargo target + apt mesa 包 都该缓存；首次 PR 跑慢一点接受，后续 < 8min 是目标
- [ ] **手编装饰怎么管**：`worldgen/preview/manual-decorations.json` 与 `decorations.json` 的合并语义（manual override gen，还是 append？）

---

## §7 进度日志

- **2026-04-28**：骨架立项 — 承接"GitHub Actions 还能玩什么"讨论；用户确认 B 方案（真 Fabric client 渲染）+ JSON 装饰 + paths 自动 / `@preview-worldgen` 手动触发 + 5 角度（俯视 + 等角四方位）。
- **2026-04-28（同日，调研后修订）**：技术可行性核实通过，go signal：
  - GLFW headless：ubuntu-24.04 + xvfb + mesa llvmpipe (OpenGL 4.5) 实跑无障碍，FabricMC/fabric-api 官方 CI 已用同套（[build.yml `client_test` job](https://github.com/FabricMC/fabric-api/blob/main/.github/workflows/build.yml)）
  - loom 1.6-SNAPSHOT 不带 `useXVFB`，手动 `xvfb-run -a` 包；ubuntu-24.04 预装 xvfb 21.1.12 无需 apt
  - valence `2b705351` UTF-8 sign 中文 OK（`compound!` + `into_text()`，example `block_entities.rs`）
  - `--quickPlayMultiplayer` 1.20.1 支持
  - Bong server `main.rs:68 ConnectionMode::Offline` + 默认 25565，vanilla client 直连可
  - **现成可复用**：client `WeaponScreenshotHarness`（env var + ClientTickEvents 状态机 + `ScreenshotRecorder` + `scheduleStop`）、server `world::tsy_container_spawn`（JSON 加载模板）+ `ChunkLayer::set_block`（spawn block 自动同步给 client）
  - 风险残留：`runClient` 不自然退出 → mod 内 `scheduleStop` + workflow timeout 兜底（已在 `WeaponScreenshotHarness:200` 验证）
  - 等 `/consume-plan worldgen-snapshot-v1` 升 active 进入实施。
