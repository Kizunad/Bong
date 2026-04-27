# Bong

AI-Native Xianxia (修仙) sandbox on Minecraft. Three-layer architecture:

- **server/** — Rust 无头 MC 服务器（Valence on Bevy 0.14 ECS，MC 1.20.1 协议 763）
- **client/** — Fabric 1.20.1 微端（Java 17，owo-lib UI）
- **agent/** — LLM "天道" agent 层（TypeScript，三 Agent 并发推演）
- **worldgen/** — Python 地形生成流水线（blueprint 驱动，terrain_gen 模块，LAYER_REGISTRY 统一 16 层地形）
- **library-web/** — 末法残土图书馆前端（Astro，静态站点）

## Quick commands

```bash
# Server
cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
cd server && cargo run              # 监听 :25565，offline mode

# Client
cd client && ./gradlew test build   # jar 在 build/libs/
cd client && ./gradlew runClient    # 通过 WSLg 启动 MC

# Agent（天道）
cd agent && npm run build                          # 编译 TS
cd agent/packages/tiandao && npm start             # 启动天道 Agent
cd agent/packages/tiandao && npm run start:mock    # mock 模式（无需真实 LLM）
cd agent/packages/tiandao && npm test              # 类型检查 + vitest

# Schema
cd agent/packages/schema && npm test

# Worldgen
cd worldgen && python -m scripts.terrain_gen       # 地形生成主流程
bash worldgen/pipeline.sh                          # 默认导出 raster + 预览

# Dev reload (regen + validate + rebuild + restart)
bash scripts/dev-reload.sh
bash scripts/dev-reload.sh --skip-regen            # rebuild only
bash scripts/dev-reload.sh --skip-validate         # 跳过 raster 校验

# Full smoke test
bash scripts/smoke-test.sh
```

## Key dependencies & versions

- Valence: git rev `2b705351`（pinned in Cargo.toml）
- big-brain `0.21`，bevy_transform `0.14.2`，pathfinding `4`
- Fabric: MC 1.20.1，Loader 0.16.10，owo-lib 0.11.2+1.20
- Schema: @sinclair/typebox 0.34
- Agent: openai ^4，ioredis ^5，tsx ^4，vitest ^3

## Architecture notes

- **Server ↔ Agent IPC**：Redis（`bong:world_state` 发布，`bong:agent_cmd` 订阅，`bong:player_chat` 队列）
- **IPC schema**：TypeBox（TS source of truth）→ JSON Schema export → Rust serde structs；共享 `agent/packages/schema/samples/*.json` 双端校验
- **天道 Agent**：三 Agent 并发推演（灾劫/变化/演绎时代），Arbiter 仲裁层负责合并与冲突消解
- **NPC AI**：big-brain Utility AI（Scorer → Action 模式），Position ↔ Transform 同步桥
- **Worldgen 流水线**：blueprint 定义固定坐标大地图 → terrain_gen 生成区域 field → stitcher 负责 zone→wilderness 过渡（按 LAYER_REGISTRY blend_mode）→ raster_export 导出 little-endian float32/uint8 二进制（mmap-friendly）→ Rust server 运行时按需生成 chunk
- **LAYER_REGISTRY**（`worldgen/scripts/terrain_gen/fields.py`）：16 层地形统一注册表，每层定义 `LayerSpec(safe_default, blend_mode, export_type)`；stitcher 和 raster_export 均从此派生配置
- **Dev harness**：`scripts/dev-reload.sh` 一键 regen+validate+rebuild+restart；`worldgen/scripts/terrain_gen/harness/raster_check.py` 做 raster 后验（rift_axis_sdf 默认值、height range、water depth）
- **Terrain profiles**：qingyun_peaks、spring_marsh、rift_valley/blood_valley、spawn、north_wastes、lingquan_marsh 均已完成
- `#[allow(dead_code)]` on `mod schema` in main.rs — schema 模块用于 IPC 对齐，尚未接入运行时

## Current milestone

**M1 — 天道闭环** ✅（2026-04-13 验收通过：server + agent + client 联跑，聊天栏出现 narration，server 消费 agent_cmd）

| 层 | 状态 |
|----|------|
| Server | MVP 0.1 ✅（草地平台、玩家连接、僵尸 NPC、Redis IPC） |
| Agent | ✅（三 Agent 并发、Context Assembler、Arbiter、WorldModel Redis 持久化、137 单测、端到端联调通过） |
| Client | MVP 0.1 ✅（Fabric 微端、CustomPayload、HUD 渲染） |
| Schema | ✅ 双端对齐 |
| Worldgen | Phase A ✅，LAYER_REGISTRY refactor ✅，Phase B ✅（巨树/洞穴/水体/子表面/平滑/结构物/群系细化） |

## Conventions

- 使用中文沟通
- 云端开发，拉到本地 WSL 测试
- `cargo run` 使用 offline mode（无需 Mojang 认证）
- Client 测试通过 `./gradlew runClient`（WSLg，无需单独启动器）
- Java 17 用于 Fabric，系统默认 Java 21（sdkman）
- docs/ 目录存放架构设计文档和路线图，修改前可参考
- Python 文件保存后自动 ruff 格式化（PostToolUse hook，见 `.claude/settings.local.json`）
- 跑会开 worktree 的外部 orchestrator（Codex / Sisyphus 等）之前，先 `git commit -m "WIP"` 把 worktree 改动落盘；跑完 `git stash list` 检查孤儿 `WIP before inspecting ...` / `WIP: stash before inspecting ...`，有就 `git stash pop` 回来（那类 agent 会 auto-stash + `reset --hard` 但不 auto-pop）

## Plan 工作流

修仙系统功能落地由 plan 文档驱动。**三态流转**：

- **骨架** `docs/plans-skeleton/plan-<name>-vN.md` — 草案，目标 + P0/P1/... 大致划分
- **Active** `docs/plan-<name>-vN.md` — 实施中，被 `/consume-plan` 消费的对象
- **归档** `docs/finished_plans/plan-<name>-vN.md` — 全部阶段 ✅ 且填好 `## Finish Evidence` 后迁入

### Plan 文件结构（写 plan 时必须遵守）

每份 plan 必须包含：

1. **头部**：一句话主题 + 阶段总览（P0/P1/.../P5 各自 ✅⏳⬜ + 验收日期 `YYYY-MM-DD`）
2. **各阶段块**（P0/P1/...）：每段写出**可核验**的交付物——下游核验工具（`/plans-status` / `/audit-plans-progress` / `/consume-plan`）会按这些抓手 grep 代码
   - 模块名 / 文件路径（如 `server/src/cultivation/`）
   - 类型 / 函数名（如 `struct Tribulation` / `fn breakthrough`）
   - 测试声明（如 "cultivation::* 94 单测"）
   - schema 名 / Redis key / 配置字段（如 `bong:insight_request`）
   - 跨仓库契约 symbol（server↔agent↔client，例 `CultivationDeathTrigger`）
3. **`## Finish Evidence`**（迁入 `finished_plans/` 前必填，章节标题严格如此）：
   - **落地清单**：每阶段对应真实模块/文件路径
   - **关键 commit**：hash + 日期 + 一句话
   - **测试结果**：跑过的命令 + 数量
   - **跨仓库核验**：server / agent / client 各自命中的 symbol
   - **遗留 / 后续**：未在本 plan 范围、依赖其他 plan 的待办

### 状态标记

- `✅ YYYY-MM-DD` — 已完成 + 验收日期
- `⏳` — 进行中
- `⬜` — 未开始
- `🔄` — 代码超前于文档（`/plans-status` 等核验工具标出，提示需补文档）
- `⚠️` — 文档自报已完成但代码未找到（红旗）

### 流转规则

- **骨架 → Active**：人工 `git mv docs/plans-skeleton/plan-x-vN.md docs/plan-x-vN.md`，或基于骨架写新版本号 vN+1。skeleton 不会被 `/consume-plan` 消费
- **Active → Finished**：全部 P ✅ + Finish Evidence 写完后，由 `/consume-plan` 在 PR 末尾 commit 内 `git mv` 入 `finished_plans/`，或人工 mv + commit
- **一个 PR 只动一个 plan**：`/consume-plan` 不允许顺手归档/修改其他 plan

### `/consume-plan` 对 docs/ 的写权限

**仅允许**：在 `docs/plan-$PLAN.md` 末尾追加 `## Finish Evidence`、最终 `git mv` 入 `docs/finished_plans/`。

其他 `docs/` 文件 / `CLAUDE.md` / `worldview.md` 严禁自动改——遇到必须改的情况停下交人工。

## Testing — 饱和化测试

**核心原则**：测试要把"目标行为"完全锁住，让任何回归都立刻撞红。我不接受"smoke 过了就行"或"happy path 跑通"的节流——目标没被测试稳稳锁住，就等于没写。

- **饱和覆盖**：每个新加的函数 / 组件 / 协议都要测 ① happy path ② 所有边界（empty / max / boundary off-by-one）③ 所有错误分支（invalid input、权限、状态前置）④ 所有状态转换（enum 变体、生命周期阶段）。覆盖到"想不出还能加什么 case"为止
- **测契约不测实现**：断言外部可观察的行为（IO、协议、副作用、payload 结构），不要绑死内部调用次数 / 私有字段 / 中间步骤。重构内部不应让测试红
- **mock 顶位时接口必须完整**：当下游模块未实装（plan A 依赖 plan B 的 P0），mock 暴露的接口要和真实最终形态一致；测试要覆盖 mock 的全部行为分支，让真实 impl 接入时"只换 impl 不改测试"。**接口先于实现锁定，测试同时锁定接口**
- **schema / enum / 状态机有专属 pin 测试**：每个 TypeBox / serde variant 都要有正反 sample 对拍；每个 enum 变体至少一条专属 case；每个 state transition (A→B、A→C、A→A) 都有命中用例。schema 改动连同 sample 一起改
- **集成测试走完整链路**：单元测试不能替代集成测试。client 发请求 → server 处理 → emit payload → client 收到 这种端到端路径要有专门的 e2e 用例，不要假设单元拼起来就是对的
- **失败信息带修复线索**：assert 写清"期望是 X 因为 Y，实际是 Z"，而不是 `assertEq(a, b)` 一行带过。撞红时不需要 git blame 才能理解为什么
