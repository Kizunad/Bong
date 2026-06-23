# Bong · AGENTS.md

> 这份文件被 **oh-my-opencode 的 `directory-agents-injector` hook** 自动注入到任何读取本仓库文件的 opencode session 中。内容是给 agent 看的硬约束，不是给人看的使用文档（`CLAUDE.md` 才是）。
>
> 简短原则：**CLAUDE.md 描述项目**，**AGENTS.md 约束 agent 行为**。两者互补，不复制。

---

## 1. plan 消费流水线（`docs/plan-*.md` → Atlas → 归档）

适用于 `/consume-plan`、`scripts/bong-plan-auto.sh`、手动 `@plan` 启动的所有流水线 session。

### 1.1 plan 来源白名单

| 目录 | 状态 | 流水线是否可消费 |
|---|---|---|
| `docs/plan-*.md` | 活跃定稿 plan | ✅ 仅此可消费 |
| `docs/plans-skeleton/*.md` | 仅标题占位（见该目录 README） | ❌ 禁止 |
| `docs/finished_plans/*.md` | 已归档历史 | ❌ 禁止 |

若调用方传入骨架或归档 plan，立即 `<promise>BLOCKED: 不能消费骨架或已归档 plan</promise>` 退出。

### 1.2 运行态 vs 源码态隔离

- `docs/` 是 **source of truth**：流水线**只读取**，**不回写**。
- 运行态全部落在 `.sisyphus/`：
  - `.sisyphus/inputs/<name>.md` —— 从 `docs/` 拷入的 plan 快照
  - `.sisyphus/plans/<name>.md` —— Prometheus 规整输出
  - `.sisyphus/boulder.json` —— Atlas 执行状态（支持中断恢复）
  - `.sisyphus/drafts/` —— Prometheus interview drafts（本场景通常不触发）
- **唯一允许的 docs/ 写入**：所有 TODO 绿 → `bash scripts/plan-finish.sh <name>` → `git mv docs/plan-<name>.md docs/finished_plans/`。

### 1.3 四阶段编排（不可跳过、不可重排）

1. **Prometheus** 把 `.sisyphus/inputs/<name>.md` 视为**已完成的 interview transcript**，规整为 `.sisyphus/plans/<name>.md`。**禁止** interview、**禁止**扩写需求、**禁止**改动 `docs/`（`prometheus-md-only` hook 会强制）。
2. **Metis** 做预分析（hidden intent / AI failure points），结果**回填**到 `.sisyphus/plans/<name>.md` 对应 TODO，不新开文件。
3. **Momus** 以 high-accuracy 模式审核，拒绝 → Prometheus 修正 → 再审。`/ulw-loop` max 100 iter 兜底。
4. **Atlas** 执行 `/start-work <name>`，按 TODO 逐个落地。

### 1.4 失败即标注，不阻断

| 失败粒度 | 行为 |
|---|---|
| 单 TODO 测试红 | `session-recovery` + `ralph-loop` 自动续 |
| 同 TODO 连续 3 轮红 | 在 `.sisyphus/plans/<name>.md` 该 TODO 下标 `[BLOCKED: <原因 + 测试名 + 关键错误>]`，**跳过继续下一个** |
| 全部走完有 BLOCKED | `<promise>BLOCKED: N 项阻塞</promise>`，不归档 |
| 全部绿 | `bash scripts/plan-finish.sh <name>` → `<promise>DONE</promise>` |

---

## 2. 三栈命令矩阵（严格对齐 `CLAUDE.md`）

| 栈 | 目录 | 命令 |
|---|---|---|
| server (Rust) | `server/` | `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`；运行 `cargo run`（offline mode） |
| client (Java/Fabric) | `client/` | `./gradlew test build`；UI 验证 `./gradlew runClient`（JDK **17**，不是系统默认 21） |
| agent (TypeScript) | `agent/` | `npm run build`；子包 `packages/tiandao` `npm start` / `npm run start:mock` / `npm test`；`packages/schema` `npm test` |
| worldgen (Python) | `worldgen/` | `python -m scripts.terrain_gen`、`bash worldgen/pipeline.sh`；raster 校验走 `worldgen/scripts/terrain_gen/harness/raster_check.py` |
| 联调 | 仓库根 | `bash scripts/dev-reload.sh`、`bash scripts/smoke-test.sh`、`bash scripts/smoke-test-e2e.sh` |

**不要跨栈调命令**（server 里不跑 npm、agent 里不跑 cargo）。

---

## 3. 禁止动作

- 用户明确要求 `commit` / `push` / `commit + push` / `开 PR` / `gh pr create` 时，视为已授权普通提交、普通推送和 PR 创建，**无需二次确认**。
- 仍需明确确认：`git push --force`、`git reset --hard`、`git commit --amend`、交互式 rebase、`--no-verify`、批量删除/移动文件、依赖版本或生产配置改动。
- `git push --force`、`git reset --hard`、`git commit --amend`（无明确用户授权时）
- `--no-verify`、`--no-gpg-sign`、`-c commit.gpgsign=false`
- 绕过 "Java 17 for Fabric" 约定
- 改 `.gitignore`、`package.json`、`Cargo.toml` 的依赖版本（除非当前 plan 明确要求）
- 向 `docs/worldview.md` 回写（世界观锚点，只在核心 canon 改动时手动修）
- 向 `docs/library/` 主动回写（图书馆域由专门的 `library-curator` agent 负责，plan 流水线不跨界）
- **`git stash push` 无对等 `git stash pop`**：任何在主仓库 auto-stash 的流程，完成时必须把自己产生的 WIP stash pop 回来；不得在主仓库留下 `WIP before inspecting ...` 孤儿 stash（历史教训：曾 stash + `reset --hard` 主仓库但不 pop，用户 worktree 改动凭空"消失"直到从 stash 捞出）

---

## 4. 委派偏好

| 任务 | 目标 |
|---|---|
| 架构 / review / 难 debug | `@oracle`（read-only） |
| 代码库大范围搜索 | `@explore` |
| 多仓 / OSS 实现参考 | `@librarian` |
| UI / 前端视觉 | `delegate_task(category="visual-engineering", ...)` |
| 硬逻辑 / 架构决策 | `delegate_task(category="ultrabrain", ...)`（已配 `openai/gpt-5.4`） |

---

## 5. Commit 约定

- `git-master` skill 负责拆分：3+ 文件 ≥ 2 commits，5+ 文件 ≥ 3 commits
- commit message **中文**，匹配仓库近 30 提交风格（git-master 自动检测）
- 每章节 TODO 绿 → commit；不堆积巨型 commit
- 归档 commit 形如：`归档 plan-<name>：<一句话总结>`
- Bong 关闭 `git_master.commit_footer`（见 `.opencode/oh-my-opencode.json`），保留 `Co-authored-by` 尾签

---

## 6. 零交互

整个流水线不向用户发问、不等确认。歧义点处理顺序：

1. 本文件
2. `CLAUDE.md`
3. `docs/worldview.md`（仅世界观相关歧义）
4. 真正无解 → `[BLOCKED: ...]` 标注，继续其它 TODO，不阻断

---

## 7. 沟通语言

中文。commit、narration、plan 注释、`<promise>` 消息统一中文。

---

## 8. 世界观正典硬锚（写代码/schema/命名前先对，别凭"修仙常识"）

<<<<<<< Updated upstream
唯一权威 `docs/worldview.md`。下面是**最常被违反**的几条，违反 = Pi/Hive review 直接打回：

- **六境界**（§三 修炼体系 L67-72，顺序固定）：**醒灵 → 引气 → 凝脉 → 固元 → 通灵 → 化虚**。严禁上古称呼：练气 / 筑基 / 金丹 / 元婴（L63 明禁）。
=======
唯一权威 `docs/worldview.md`。下面是**最常被违反**的几条，违反 = @pi/@hive review 直接打回：

- **六境界**（§一 L68-72，顺序固定）：**醒灵 → 引气 → 凝脉 → 固元 → 通灵 → 化虚**。严禁上古称呼：练气 / 筑基 / 金丹 / 元婴（L63 明禁）。
>>>>>>> Stashed changes
- **命名禁词**（L63）：末法时代禁用 玄/陨/星/仙/太/古；优选衰败素朴意象 残/碎/锈/杂/粗/髓/朴/枯。例外：已入世俗医药的矿名（丹砂/朱砂/雄黄）OK。
- **经济**：唯一真货币 = **骨币**（异变兽骨+阵法锁真元）；矿物=交易筹码非货币；灵石=劣质衰变物+燃料；金银=废土。
- **zone 命名**：写新 zone 前查 worldview §十表格（已立：spawn / 青云残峰 / 血谷 / north_wastes / lingquan_marsh 等）。
- 引用格式统一 `worldview.md §X L<line>`，便于回查。

## 9. 真元/灵气守恒律（最高优先级硬约束，吞真元 = 阻塞合并）

全服灵气总量 `SPIRIT_QI_TOTAL` 恒定（const 当前 100.0；**测试断言取 const 引用，不写字面 100**）。所有真元/灵气流动**必须**走 `qi_physics::ledger::QiTransfer { from, to, amount, reason }`。

**红旗（出现就停下重设计）**：
- `cultivation.qi_current += X`（无对应 zone 减）、`zone.spirit_qi -= Y`（无对应玩家增）、容器/衰变把真元"凭空消失"不归还 zone、招式释放只扣攻方不写入环境 —— 全是守恒律红旗。
- 离屏/抽象战斗死亡：携 `qi_current > 0` 的快照直接 `store.remove` 丢弃、或只 `emit QiTransfer` 事件却**无 system 消费**应用到 `WorldQiAccount` = 吞真元。离屏战死必须走 `release_dormant_qi_to_zone` → `ledger.transfer(ReleaseToZone)`。
- **自定真元物理常数/公式**：新模块出现 `*_DECAY*` / `*_DRAIN*` / `*_ATTEN*` / `*_HALF_LIFE*` / `RHO` / `BETA` / 形如 `0.0X_f64` 的"看起来像衰减率"常数 / `fn *_decay()` → **必查 `qi_physics`**（plan-qi-physics-v1）。已存在就调用；不存在就**先扩 `qi_physics::constants` 再 import**，**禁止 plan 自己写一份**。
- 唯一允许的"系统外流"= 天道每时代衰减 1-3%（`qi_physics::tiandao::era_decay_step`，常数 `QI_TIANDAO_DECAY_PER_ERA_MIN/MAX`）。注意它**不是凭空蒸发**：`WorldQiBudget::apply_era_decay` 把衰减量挪进被追踪的沉降槽 `era_decay_accum`，不变式 `current_total + era_decay_accum == initial_total` 恒成立。守恒口径用 `qi_physics::ledger::assert_conservation(before, after, era_decay)` 断言。
- 释放走 `qi_release_to_zone`，吸收走 `qi_excretion`，坍缩渊塌缩走 `collapse_redistribute_qi`（中转站不是终点）。

> 完整孤岛红旗清单见 `docs/CLAUDE.md §四`（**Codex 默认不自动加载，碰 gameplay/qi plan 时先读一遍**）。

## 10. 招式/技能 A/V 差异化（战斗/skill 类 plan 的红线）

任何 skill / cast / 招式 / 主动能力落地，**必须**携带**每招独立可辨**的：① animation ② particle/VFX ③ SFX ④ HUD 反馈 ⑤ hotbar/SkillBar 槽位 PNG icon。

- "只动 server 算子先 ship、客户端 P 后补" = **红旗**——招式没视觉就不算 P0/P1 完成。仅 server 算子/仅 schema enum 不算"实装"。
- skill plan 的 `§N 客户端动画/VFX/SFX` 段必须**表格化列出每招独立的 animation+粒子+音效+HUD+icon 名**（基线范本：`docs/finished_plans/plan-yidao-v1.md §5`）。
- 验收末阶段必须含**视觉/听觉差异化回归 + icon 显示回归**（玩家能从远处分辨"对面在用 X 不是 Y"）。
- icon 资产：新招 PNG 走 `/gen-image item`（`scripts/images/gen.py`），路径 `client/src/main/resources/assets/bong/textures/skill/<style>/<skill_id>.png`（16×16/32×32，化虚级 `<skill_id>_void.png` 高分辨率+染色描边）；server `SkillDef.icon_id` → schema 双端镜像 → client `SkillIconRegistry` 查图。
- **Codex 自身不能跑 `/gen-image`（Claude skill）**：碰到要生成 icon/贴图的 TODO，写好 server/schema/client 接线 + 占位资源 + 在该 TODO 标 `[BLOCKED: 需 /gen-image 生成 <清单>]`，继续其它 TODO，不要画手绘糊弄、也不要跳过接线。

## 11. 视觉资产纪律（NBT 建筑 / layout / 模型 / 贴图）

- **3 轮打磨 + `<PROMISE>` 担保**：NBT 建筑、worldgen layout 摆位、复杂模型、视觉资产**禁止一把 commit**。Round 1 first cut → Round 2 自评（截图渲染/structure dump/ASCII 平面投影）→ Round 3 终轮，commit message 标 `(round N/3)`；终轮 commit 末尾写 `<PROMISE>...已 3 轮打磨...已检查[...]...仍存局限[...]</PROMISE>` 块（**拼写是 PROMISE 不是 PROMIS**）。纯 Rust/TS 逻辑 TODO 不适用。
- **复杂模型分部件做**：拆 `part_base()` / `part_body()` / ... 函数，逐件单独预览，最后 `all_cubes()` 拼接（别整件一把梭埋掉单件缺陷）。bbmodel 真长相用 `scripts/models/render_bbmodel.py` 看，别只信平涂示意图。
- **item icon 批量出**：新增 ItemTemplate 必配 icon，走 `/gen-image item`（批量、不需多轮）。Codex 同 §10 处理——标 `[BLOCKED: 需 /gen-image]`。

## 12. 架构硬约束 + 端到端联调

- **禁止 vanilla MC entity hack**：不准用 armor stand / invisible mob 充当碰撞箱或交互载体。Bong 的 entity 是 **Marker + 自定义渲染**，交互走 **C2S 请求**（client 注册 IntentHandler / InteractKeyRouter / 右键准星检测 → C2S → server），不走 vanilla InteractEntityEvent。范本：NPC 的 `NpcEngagementIntentHandler → NpcInspectRequest`。绝不切到有碰撞箱的 EntityBundle。
- **PlayerAnimator 四大库坑**（写动画必读，不看源码猜不到）：
  1. **循环动画单帧衰减**：`isLooped=true` 时只在 tick 0 放关键帧的 axis 会被插值回 `defaultValue`——每个用到的 axis 必须在 `endTick` 补一个同值 keyframe。
  2. **MC 无 IK**：`leg.pitch > ~35°` 腿腹断连。大 pitch 用 `bend`（小腿后折）承担，pitch 控在 40° 内；别给 leg 加 z 偏移（更糟）。
  3. **`body.*` 走 MatrixStack 不是 ModelPart**：整体位移/旋转（含头发盔甲手持物）。要"上半身扭下盘不动"用 `torso.yaw`，不用 `body.yaw`。
  4. **`bend` 需 bendy-lib 否则静默 no-op**：已配 `bendy-lib 4.0.0`（MC 1.20.1 唯一可用版本）于 client depends，别动版本。
  迭代姿态用 headless 工具 `client/tools/render_animation.py`（出三视图 PNG，免 build jar + runClient）。完整约定见 `docs/player-animation-conventions.md`。

## 13. 测试诚实性 + 构建/CI 坑

- **饱和化测试**（root CLAUDE.md 已述）：每个新函数/组件/协议测 happy path + 所有边界 + 所有错误分支 + 所有状态转换；测契约不测实现；schema/enum/状态机有专属 pin 测试 + 正反 sample 对拍；e2e 走完整链路。"smoke 过了"不算。
- **绝不把自己引入的失败甩锅 "pre-existing"**：上一已 merge 阶段是 `0 failed`、本阶段突现 N failed = signature（registry/asset 加载、schema parse、template exist）= 单点根因**（一个坏 config 连锁红一片）。
- **`ItemCategory` 合法集**（`server/src/inventory/mod.rs:216`）：Pill / Herb / Scroll / Misc / Weapon / Armor / Tool / Treasure / RecipeFragment / RecipeHint / BoneCoin / Container —— **无 Material**。炼丹材料用 `Misc`（用 `material` 会让整个 item registry 加载失败 → 连锁红几十个测试）。
- **schema 改了必重建 dist**：`agent/packages/tiandao` 经 `@bong/schema` 引用的是构建产物 `dist/`，不是 src。改了 `agent/packages/schema/src/*.ts`（新增 export/改 schema）后必须 `cd agent && npm run build -w @bong/schema`，否则 agent 启动崩 `SyntaxError: does not provide an export named 'X'`。
- **headless/CI 启服必设 `export BONG_SKIP_SKIN_PREFETCH=1`**：否则 `maintain_skin_pool` 因缺 `MINESKIN_API_KEY` panic（`src/skin/pool.rs:165`）。配 dummy key 没用（超时再 panic）。
- **真集成 gate = `e2e`**（`bash scripts/smoke-test-e2e.sh` / `e2e` CI check），不是 snapshot；判 worldgen/server 改动看 e2e + 单测最可靠。`main` 未设保护，多数 check 非 required。

## 14. （仅当你的流程会开 PR 时）review gate

sisyphus 自动流水线以 `scripts/plan-finish.sh` 归档收尾、不开 PR；若你走的是开 PR 的变体：
<<<<<<< Updated upstream
- **gate 只看 `/review` + CodeRabbit，绝不等 Codex**。`chatgpt-codex-connector`("Codex usage limits reached") 是与本仓库无关的噪音，忽略。
- **Review 不再自动跑，且已合并为单一 `/review`**：旧的 `/review pi` `/review hive` `/review claude` 三入口合一，在 PR 评论 `/review` 触发即可。引擎=对峙(claude finder swarm 跑 proxy 上 deepseek/sensenova 小模型) + 怀疑投票审判 + 裁决(codex 跑 gpt-5.5)，全走自家代理 proxy.kizun4.uk。触发词用 `/review` 而非 `@x`——`@pi`/`@hive`/`@claude` 会 mention 到 GitHub 上的真实陌生用户。CodeRabbit 仍自动（但额度耗尽会限流失败，那是计费问题不是代码问题）。
=======
- **gate 只看 Pi agent + CodeRabbit，绝不等 Codex**。`chatgpt-codex-connector`("Codex usage limits reached") 是与本仓库无关的噪音，忽略。
- **Pi / Claude review 不再自动跑**：需在 PR 评论 `@pi`（结构化审核，不吃额度）/ `@claude_workflow_review` 触发；CodeRabbit 仍自动（但额度耗尽会限流失败，那是计费问题不是代码问题，可降级到 Pi-only）。
>>>>>>> Stashed changes
- 等待用轮询节奏（~20min/回合，最多 3 回合），别 busy-poll。修完 review 意见要重新等 re-review，不自判"应该过了"。
