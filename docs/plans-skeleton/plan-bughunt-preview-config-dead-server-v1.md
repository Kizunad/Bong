# plan-bughunt-preview-config-dead-server-v1（骨架）

> **骨架（草案）**。一句话主题：`preview-harness.json` 宣称可配置 `server` / `username`，但 preview client 实际完全不消费这两个字段；真正决定 quick-play 目标的是 `client/build.gradle` 里的 `BONG_PREVIEW_SERVER` 环境变量默认值 `127.0.0.1:25565`。结果是 **preview/world snapshot 流程会“看起来读了配置，实际上连到另一台服/默认服”**，在非默认端口、多套并行 preview、手动 rerun 场景下会静默拍错世界或直接超时 0 图。

## 阶段总览

| 阶段 | 主题 | 路由 | 状态 |
|------|------|------|------|
| P0 | `preview-harness.json` 的 `server/username` 为死字段，preview quick-play 目标与配置脱钩 | fix_pr | ⬜ |

## P0 — preview 配置死字段，world snapshot 可能静默拍错服

- **#1 major（fix_pr）**：`client/preview-harness.json`、`docs/finished_plans/plan-worldgen-snapshot-v1.md §1.5`、`client/src/main/java/com/bong/client/preview/PreviewConfig.java:41-99` 共同把 `server` / `username` 描述成 preview harness 的正式配置；但运行链路里：
  - `PreviewConfig.load()` 只把这两个字段读进 record（`PreviewConfig.java:64-65`），后续**零消费**
  - `PreviewHarnessClient.install()` 仅 `load(config)` → `new PreviewSession(config)` → 注册 tick（`PreviewHarnessClient.java:46-65`），既不发起连接，也不把 `config.server()` / `config.username()` 传给任何连接层
  - `PreviewSession` 只负责世界 ready/传送/截图状态机，不碰连接参数（`PreviewSession.java:24-217`）
  - 真正决定 client 启动后连哪台服的是 `client/build.gradle:205-219`：`runClientPreview` 走 `args '--quickPlayMultiplayer', previewServer`，而 `previewServer = System.getenv('BONG_PREVIEW_SERVER') ?: '127.0.0.1:25565'`
  - `.github/workflows/worldgen-preview.yml:150-162` 只传 `BONG_PREVIEW_HARNESS=1` 和 `BONG_PREVIEW_CONFIG=.../preview-harness.json`，**没有**把 `preview-harness.json.server` 回灌成 `BONG_PREVIEW_SERVER`
  - 全仓对 `config.server()` / `config.username()` 的唯一“覆盖”只有 `PreviewConfigChunkReadyTest` 断言默认值（`client/src/test/java/com/bong/client/preview/PreviewConfigChunkReadyTest.java:97`），没有任何消费侧测试；`rg` 全仓也无其它读取点
- **结论**：当前 preview/world snapshot 链路是“配置文件读取成功，但连接目标由另一条独立 env 路径决定”。`server` 是死字段；`username` 更是连替代路径都没有，属于纯装饰字段。

## 复现路径

1. 保持 server 正常监听默认 `127.0.0.1:25565`。
2. 把 `client/preview-harness.json` 的 `server` 改成一个**不同**地址，例如 `127.0.0.1:25566`；可顺手把 `username` 改成 `PreviewBotAlt`。
3. **不要**设置 `BONG_PREVIEW_SERVER`，直接运行 `cd client && xvfb-run -a ./gradlew runClientPreview --no-daemon`（或沿用 `.github/workflows/worldgen-preview.yml` 当前做法）。
4. 观察：
   - client 仍会按 `client/build.gradle:217-218` 的默认值连 `127.0.0.1:25565`
   - `preview-harness.json.server=127.0.0.1:25566` 完全不生效
   - `username` 无任何落点，既不参与 quick-play，也不进入 `PreviewHarnessClient` / `PreviewSession`
5. 若默认端口上恰好有另一台可用 server，preview **会成功出图但拍的是错世界**；若默认端口无服务，则 `PreviewSession.WAIT_WORLD` 走到 `timeout`，结果是 0 图/假红，而不是按配置去连 `25566`

## 根因链路

- **设计口径分叉**：
  - 文档/配置口径：`PreviewConfig` 和 `preview-harness.json` 把 `server` / `username` 定义成 harness 输入
  - 实际执行口径：连接发生在 Gradle `runClient` 启动参数层，不在 Java harness 层；而 Gradle 只认 `BONG_PREVIEW_SERVER`
- **断链点**：
  - `PreviewConfig.server` / `PreviewConfig.username` 从 JSON 读出后没有任何 consumer
  - workflow 只注入 `BONG_PREVIEW_CONFIG`，未做“读取 JSON → 导出 `BONG_PREVIEW_SERVER`”桥接
- **为什么它不是“预留字段”而是 bug**：
  - 现有注释和样例配置都把这两个字段写成可工作的正式入口，而不是 future placeholder
  - `runClientPreview` 已经依赖 quick-play 自动连服；此时 `server` 若不生效，就不是 harmless dead code，而是**行为与配置表面语义不一致**

## 影响面

- `client/build.gradle` 的 `runClientPreview` 本地手动使用者
- `.github/workflows/worldgen-preview.yml` 及其未来任何“换端口/换目标服”的变体
- 任何希望并行跑多套 preview server、或把 preview server 挂在非 `25565` 端口的开发/CI 场景
- `username` 相关的基线分离诉求（不同快照 bot 名称、离线档区分）当前**完全无法通过配置实现**

## 这个 bug 对实际游玩体验的影响

- 对普通玩家的日常游玩**直接影响较小**，因为它主要卡在 preview/world snapshot 工具链，不是在线玩法主循环。
- 但对实际内容验收体验影响很实在：开发者以为自己在看“这次 worldgen 改动的快照”，实际可能拍的是默认端口上的旧世界/另一台服，导致：
  - PR 视觉验收被错误 artifact 误导，错把“没改到”当“改对了”
  - 非默认端口/并行环境下 preview 无法按配置连服，直接超时 0 图
  - 本地复核与 CI 结果不一致，排查时会误以为是 chunk、渲染、pause menu 或 world bootstrap 问题，实际根因只是连错服
- 这类“静默拍错世界”比硬失败更糟，因为它会产出**看似正常但语义错误**的 world snapshot。

## 修复建议

- 二选一，必须统一口径：
  - **方案 A（更直）**：让 `runClientPreview` 或 `PreviewHarnessClient` 真正消费 `PreviewConfig.server`，把 quick-play 目标与 JSON 配置对齐；若能设用户名，也一并接通 `username`
  - **方案 B（更保守）**：明确删掉 `PreviewConfig.server` / `username`、样例 JSON、plan 文档中的对应字段，只保留 `BONG_PREVIEW_SERVER` 这一条权威入口，避免假配置
- 无论选哪条，都要补 pin 测试：
  - `config.server` 改掉后，preview 连接目标必须跟着变
  - 若保留 `username`，必须有至少一条真实消费断言；否则删字段和默认值测试

## 反方裁决

> 当前会话无可用 subagent / delegate 工具，无法再开额外子代理做独立对抗审阅；以下按要求记录**退化处理**：由本轮 bughunt 手工执行两轮反方裁决，并明确列出反方论点与驳回理由。

### 第一轮反方

- **反方论点**：`server` 字段只是给人看的元数据；真正连接入口本来就设计成 `BONG_PREVIEW_SERVER`，所以不算 bug。
- **驳回理由**：
  - 若只是元数据，`PreviewConfig` 不应把它做成正式 record 字段并提供默认值（`PreviewConfig.java:41-65`）
  - `plan-worldgen-snapshot-v1 §1.5` 与 `client/preview-harness.json` 都把它放进“配置样例”，没有任何“仅注释/不生效”说明
  - 当前 workflow 明确依赖 `BONG_PREVIEW_CONFIG` 激活 harness，却未设置 `BONG_PREVIEW_SERVER`；这说明仓库内主流程本身就在制造“配置文件应当足够”的错觉

### 第二轮反方

- **反方论点**：CI 现在固定 server 就是 `127.0.0.1:25565`，所以即便 `config.server` 不生效，也没有现实后果。
- **驳回理由**：
  - 这只覆盖“今天这份 workflow 的默认 happy path”，不覆盖手动 rerun、非默认端口、本地多实例、未来矩阵化 preview
  - 更关键的是，这条 bug 在默认端口存在另一台服时会**静默拍错世界并产出正常 PNG**，不是单纯的“某场景不可配置”
  - 从 bug 风险排序看，静默错连导致错误 world snapshot 被拿去做评审，比直接 timeout 更危险，因此应判定为 real bug，不是单纯的 tidy-up

## 审计来源

- bug-hunt round（本轮聚焦 preview / world state / world snapshot，显式避开“preview 暂停菜单卡屏”已知题）
- 证据来源：
  - `client/src/main/java/com/bong/client/preview/PreviewConfig.java:41-99`
  - `client/src/main/java/com/bong/client/preview/PreviewHarnessClient.java:33-65`
  - `client/src/main/java/com/bong/client/preview/PreviewSession.java:24-217`
  - `client/build.gradle:179-219`
  - `.github/workflows/worldgen-preview.yml:150-162`
  - `client/preview-harness.json:2-12`
  - `docs/finished_plans/plan-worldgen-snapshot-v1.md §1.5`
