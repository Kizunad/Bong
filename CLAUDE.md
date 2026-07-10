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

## Dev test commands

这些命令只用于本地 / dev 测试场景快速搭建，全部挂在 server brigadier 命令树下；client 通过原版命令树自动获得 Tab 补全，agent 不参与。

> **dev-only**：这些入口会显式绕过 worldview 自然修炼规则和 qi_physics ledger 守恒，不允许复用到生产 gameplay 路径。

| 命令 | 用途 |
|------|------|
| `/meridian open <id>` / `/meridian open_all` / `/meridian list` | 强制打通经脉或查看经脉状态 |
| `/realm set <id>` | 直写玩家境界 |
| `/qi set <value>` / `/qi max <value>` | 直写真元当前值或上限 |
| `/technique list` / `/technique add <id>` / `/technique remove <id>` / `/technique proficiency <id> <value>` / `/technique active <id> <bool>` / `/technique reset_all` | 查看、增删、调熟练度或重置功法 |
| `/give <template_id> [count]` | 给予物品 |
| `/clearinv [pack\|all\|naked]` | 清背包 / hotbar / 装备槽 |
| `/zone_qi set <name> <value>` | 直写区域灵气浓度 |
| `/kill self` / `/revive self` | 触发玩家死亡 / 复活事件链路 |
| `/time advance <ticks>` | 快进 `CultivationClock` |

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

> **立新 plan 前先读 `docs/CLAUDE.md`** —— 防孤岛调研流程（必读 finished_plans / active / skeleton + 接入面 checklist + 红旗清单）。本节只讲三态流转和 plan 文件结构。

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

---

# Agent 行为硬约束

> 以下各节自原 `AGENTS.md` 并入（原文件是 oh-my-opencode 注入层，随多 harness 布局废弃）；`AGENTS.md` 现为指向本文件的 symlink，供按惯例读取 AGENTS.md 的 harness（Codex 等外部 orchestrator）共用同一份约束。

## 禁止动作

- 用户明确要求 `commit` / `push` / `开 PR` / `gh pr create` 时，视为已授权普通提交、普通推送和 PR 创建，**无需二次确认**
- 仍需明确确认：`git push --force`、`git reset --hard`、`git commit --amend`、交互式 rebase、批量删除/移动文件、依赖版本或生产配置改动
- 严禁 `--no-verify`、`--no-gpg-sign`、`-c commit.gpgsign=false`
- 不绕过 "Java 17 for Fabric" 约定；不要跨栈调命令（server 里不跑 npm、agent 里不跑 cargo）
- 不改 `.gitignore`、`package.json`、`Cargo.toml` 的依赖版本（除非当前 plan 明确要求）
- 不向 `docs/worldview.md` 回写——世界观锚点只在核心 canon 改动时人工修，且必须单独 PR 人工 review
- 不向 `docs/library/` 主动回写（图书馆域走 `/write-book` / `/review-book` 专门流程，plan 流水线不跨界）
- **`git stash push` 无对等 `git stash pop`**：任何 auto-stash 的流程，完成时必须把自己产生的 WIP stash pop 回来；不得在主仓库留下 `WIP before inspecting ...` 孤儿 stash（历史教训：曾 stash + `reset --hard` 主仓库但不 pop，用户 worktree 改动凭空"消失"直到从 stash 捞出）

## Commit 约定

- commit message **中文**，匹配仓库近 30 提交风格；每个逻辑单元一个 atomic commit，不堆积巨型 commit
- 归档 commit 形如：`归档 plan-<name>：<一句话总结>`

## 世界观正典硬锚（写代码/schema/命名前先对，别凭"修仙常识"）

唯一权威 `docs/worldview.md`。下面是**最常被违反**的几条，违反 = review 直接打回：

- **六境界**（worldview.md §三 L67-L72，顺序固定；worldview.md §三 L63 明禁旧称）：**醒灵 → 引气 → 凝脉 → 固元 → 通灵 → 化虚**。严禁上古称呼：练气 / 筑基 / 金丹 / 元婴。
- **命名禁词**（worldview.md §三 L63 的命名原则落地速查）：末法时代禁用 玄/陨/星/仙/太/古；优选衰败素朴意象 残/碎/锈/杂/粗/髓/朴/枯。例外：已入世俗医药的矿名（丹砂/朱砂/雄黄）OK。
- **经济**：唯一真货币 = **骨币**（异变兽骨+阵法锁真元）；矿物=交易筹码非货币；灵石=劣质衰变物+燃料；金银=废土。
- **zone 命名**：写新 zone 前查 worldview.md §十三 L1253-L1260 区域表和 `server/zones.json` 既有 ID（已立：spawn / 青云残峰 / 血谷 / north_wastes / lingquan_marsh 等）。
- 引用格式统一 `worldview.md §X L<line>`，便于回查。

## 真元/灵气守恒律（最高优先级硬约束，吞真元 = 阻塞合并）

全服灵气总量 `SPIRIT_QI_TOTAL` 恒定（const 当前 100.0；**测试断言取 const 引用，不写字面 100**）。所有真元/灵气流动**必须**走 `qi_physics::ledger::QiTransfer { from, to, amount, reason }`。

**红旗（出现就停下重设计）**：
- `cultivation.qi_current += X`（无对应 zone 减）、`zone.spirit_qi -= Y`（无对应玩家增）、容器/衰变把真元"凭空消失"不归还 zone、招式释放只扣攻方不写入环境 —— 全是守恒律红旗。
- 离屏/抽象战斗死亡：携 `qi_current > 0` 的快照直接 `store.remove` 丢弃、或只 `emit QiTransfer` 事件却**无 system 消费**应用到 `WorldQiAccount` = 吞真元。离屏战死必须走 `release_dormant_qi_to_zone` → `ledger.transfer(ReleaseToZone)`。
- **自定真元物理常数/公式**：新模块出现 `*_DECAY*` / `*_DRAIN*` / `*_ATTEN*` / `*_HALF_LIFE*` / `RHO` / `BETA` / 形如 `0.0X_f64` 的"看起来像衰减率"常数 / `fn *_decay()` → **必查 `qi_physics`**（plan-qi-physics-v1）。已存在就调用；不存在就**先扩 `qi_physics::constants` 再 import**，**禁止 plan 自己写一份**。
- 唯一允许的"系统外流"= 天道每时代衰减 1-3%（`qi_physics::tiandao::era_decay_step`，常数 `QI_TIANDAO_DECAY_PER_ERA_MIN/MAX`）。注意它**不是凭空蒸发**：`WorldQiBudget::apply_era_decay` 把衰减量挪进被追踪的沉降槽 `era_decay_accum`，不变式 `current_total + era_decay_accum == initial_total` 恒成立。守恒口径用 `qi_physics::ledger::assert_conservation(before, after, era_decay)` 断言。
- 释放走 `qi_release_to_zone`，吸收走 `qi_excretion`，坍缩渊塌缩走 `collapse_redistribute_qi`（中转站不是终点）。

> 完整孤岛红旗清单见 `docs/CLAUDE.md §四`——碰 gameplay/qi plan 时先读一遍。

## 招式/技能 A/V 差异化（战斗/skill 类 plan 的红线）

任何 skill / cast / 招式 / 主动能力落地，**必须**携带**每招独立可辨**的：① animation ② particle/VFX ③ SFX ④ HUD 反馈 ⑤ hotbar/SkillBar 槽位 PNG icon。

- "只动 server 算子先 ship、客户端 P 后补" = **红旗**——招式没视觉就不算 P0/P1 完成。仅 server 算子/仅 schema enum 不算"实装"。
- skill plan 的 `§N 客户端动画/VFX/SFX` 段必须**表格化列出每招独立的 animation+粒子+音效+HUD+icon 名**（基线范本：`docs/finished_plans/plan-yidao-v1.md §5`）。
- 验收末阶段必须含**视觉/听觉差异化回归 + icon 显示回归**（玩家能从远处分辨"对面在用 X 不是 Y"）。
- icon 资产：新招 PNG 走 `/gen-image item`（`scripts/images/gen.py`），路径 `client/src/main/resources/assets/bong/textures/skill/<style>/<skill_id>.png`（16×16/32×32，化虚级 `<skill_id>_void.png` 高分辨率+染色描边）；server `SkillDef.icon_id` → schema 双端镜像 → client `SkillIconRegistry` 查图。
- **当前 harness 跑不了 `/gen-image` 时**：写好 server/schema/client 接线 + 占位资源 + 在该 TODO 标 `[BLOCKED: 需 /gen-image 生成 <清单>]`，继续其它 TODO，不要画手绘糊弄、也不要跳过接线。

## 视觉资产纪律（NBT 建筑 / layout / 模型 / 贴图）

- **3 轮打磨 + `<PROMISE>` 担保**：NBT 建筑、worldgen layout 摆位、复杂模型、视觉资产**禁止一把 commit**。Round 1 first cut → Round 2 自评（截图渲染/structure dump/ASCII 平面投影）→ Round 3 终轮，commit message 标 `(round N/3)`；终轮 commit 末尾写 `<PROMISE>...已 3 轮打磨...已检查[...]...仍存局限[...]</PROMISE>` 块（**拼写是 PROMISE 不是 PROMIS**）。纯 Rust/TS 逻辑 TODO 不适用。
- **复杂模型分部件做**：拆 `part_base()` / `part_body()` / ... 函数，逐件单独预览，最后 `all_cubes()` 拼接（别整件一把梭埋掉单件缺陷）。bbmodel 真长相用 `scripts/models/render_bbmodel.py` 看，别只信平涂示意图。
- **item icon 批量出**：新增 ItemTemplate 必配 icon，走 `/gen-image item`（批量、不需多轮）。跑不了 `/gen-image` 的 harness 标 `[BLOCKED: 需 /gen-image]`。

## 架构硬约束（entity / 动画）

- **禁止 vanilla MC entity hack**：不准用 armor stand / invisible mob 充当碰撞箱或交互载体。Bong 的 entity 是 **Marker + 自定义渲染**，交互走 **C2S 请求**（client 注册 IntentHandler / InteractKeyRouter / 右键准星检测 → C2S → server），不走 vanilla InteractEntityEvent。范本：NPC 的 `NpcEngagementIntentHandler → NpcInspectRequest`。绝不切到有碰撞箱的 EntityBundle。
- **PlayerAnimator 四大库坑**（写动画必读，不看源码猜不到）：
  1. **循环动画单帧衰减**：`isLooped=true` 时只在 tick 0 放关键帧的 axis 会被插值回 `defaultValue`——每个用到的 axis 必须在 `endTick` 补一个同值 keyframe。
  2. **MC 无 IK**：`leg.pitch > ~35°` 腿腹断连。大 pitch 用 `bend`（小腿后折）承担，pitch 控在 40° 内；别给 leg 加 z 偏移（更糟）。
  3. **`body.*` 走 MatrixStack 不是 ModelPart**：整体位移/旋转（含头发盔甲手持物）。要"上半身扭下盘不动"用 `torso.yaw`，不用 `body.yaw`。
  4. **`bend` 需 bendy-lib 否则静默 no-op**：已配 `bendy-lib 4.0.0`（MC 1.20.1 唯一可用版本）于 client depends，别动版本。
  迭代姿态用 headless 工具 `client/tools/render_animation.py`（出三视图 PNG，免 build jar + runClient）。完整约定见 `docs/player-animation-conventions.md`。

## 测试诚实性 + 构建/CI 坑

- **绝不把自己引入的失败甩锅 "pre-existing"**：上一已 merge 阶段是 `0 failed`、本阶段突现 N failed = 本 PR 引入；共同 signature（registry/asset 加载、schema parse、template exist）= 单点根因（一个坏 config 连锁红一片）。
- **`ItemCategory` 合法集**（`server/src/inventory/mod.rs`）：Pill / Herb / Scroll / Misc / Weapon / Armor / Tool / Treasure / RecipeFragment / RecipeHint / BoneCoin / Container —— **无 Material**。炼丹材料用 `Misc`（用 `material` 会让整个 item registry 加载失败 → 连锁红几十个测试）。
- **schema 改了必重建 dist**：`agent/packages/tiandao` 经 `@bong/schema` 引用的是构建产物 `dist/`，不是 src。改了 `agent/packages/schema/src/*.ts`（新增 export/改 schema）后必须 `cd agent && npm run build -w @bong/schema`，否则 agent 启动崩 `SyntaxError: does not provide an export named 'X'`。
- **headless/CI 启服必设 `export BONG_SKIP_SKIN_PREFETCH=1`**：否则 `maintain_skin_pool` 因缺 `MINESKIN_API_KEY` panic（`src/skin/pool.rs`）。配 dummy key 没用（超时再 panic）。
- **真集成 gate = `e2e`**（`bash scripts/smoke-test-e2e.sh` / `e2e` CI check），不是 snapshot；判 worldgen/server 改动看 e2e + 单测最可靠。`main` 未设保护，多数 check 非 required。

## PR review gate

- **gate 只看 `/review` + CodeRabbit，绝不等 Codex**。`chatgpt-codex-connector`（"Codex usage limits reached"）是与本仓库无关的噪音，忽略。
- **单一 `/review` 入口**：在 PR 评论 `/review` 触发（独立 issue comment，写在 PR body 不生效）。不要用 `@pi`/`@hive`/`@claude`——会 mention 到 GitHub 上的真实陌生用户。CodeRabbit 仍自动跑（额度耗尽限流失败是计费问题不是代码问题）。
- 等待用 `ScheduleWakeup delaySeconds=1200`（~20 min/回合，最多 3 回合卡死才停交人工），禁止 sleep loop / busy-poll。修完 review 意见要重新等 re-review，不自判"应该过了"（完整协议见 `docs/CLAUDE.md §6.5`）。
