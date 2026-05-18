# Plan 制定指南（防孤岛）

立新 plan 之前的调研流程 + plan 演进 / 消费的工作流方法论。**根 `CLAUDE.md` 的"Plan 工作流"讲三态流转和 plan 文件本身的结构**；本文件 §一-§四 讲**立 plan 前的调研**（确保新模块跟现有玩法接得上、不自成孤岛），§五-§六 讲 **plan 立后的演进与消费规范**（开放问题收口 / 实施工作流 / consume-plan 多 PR 编排）。

---

## 一、立 plan 前必读（按顺序）

1. **`docs/worldview.md`（正典）** —— 任何境界 / 货币 / 修炼名词 / 经济锚点都先 grep `worldview.md`。六境界（醒灵 / 引气 / 凝脉 / 固元 / 通灵 / 化虚）、骨币货币、灵石燃料、末法去上古，以这里为准。

2. **`docs/finished_plans/`（35+ 份已落地玩法）** —— 已实装模块的接口面，新 plan 必查。分类索引：
   - **修炼底盘**：`plan-cultivation-v1.md`、`plan-cultivation-mvp-cleanup-v1.md`、`plan-tribulation-v1.md`、`plan-death-lifecycle-v1.md`、`plan-skill-v1.md`
   - **产出侧**：`plan-alchemy-v1.md`、`plan-forge-v1.md`、`plan-botany-v1.md`、`plan-shelflife-v1.md`、`plan-armor-v1.md`、`plan-weapon-v1.md`
   - **战斗 / 视觉**：`plan-combat-no_ui.md`、`plan-vfx-v1.md`、`plan-hotbar-modify-v1.md`
   - **社交 / NPC**：`plan-social-v1.md`、`plan-npc-ai-v1.md`、`plan-npc-skin-v1.md`、`plan-baomai-v1.md`
   - **末法残土（tsy）**：`plan-tsy-v1.md` + `plan-tsy-{worldgen,zone,zone-followup,dimension,extract,container,hostile,lifecycle,loot}-v1.md`
   - **底层 / 基建**：`plan-server.md`、`plan-client.md`、`plan-agent.md`、`plan-agent-v2.md`、`plan-ipc-schema-v1.md`、`plan-audio-v1.md`、`plan-worldgen.md`、`plan-worldgen-v3.md`、`plan-worldgen-v3.1.md`、`plan-mvp01-plan.md`、`plan-ipc-schema-v1.md`

3. **`docs/plan-*.md`（active）** —— 正在跑的 plan。新 plan 不要跟它们 PR 撞车、不要重复定义同一个 component / event / schema。

4. **`docs/plans-skeleton/plan-*.md` + `reminder.md`** —— 同伴的"将来要写"占位。**优先合并进现有骨架，而不是另起一个新版本号**。`reminder.md` 是跨 plan 待办登记——你的新功能可能正好填某条空缺。

## 二、接入面 checklist（新 plan 头部必须写）

根 CLAUDE.md 的 plan 文件结构讲了"可核验交付物抓手"；这里再加一节 **接入面**，避免新模块自成孤岛。新 plan 头部必须明列：

- **进料**：从哪些现有模块取数据 / 物品 / event？
  例：「从 `inventory` 消耗草药 → 查 `botany::PlantRegistry` → 订阅 `cultivation::BreakthroughEvent`」
- **出料**：产出去哪里？
  例：「输出 `Pill` 实例进 `inventory` → emit `alchemy::BrewedEvent` 给 `skill` 加经验 → 接 `shelflife` 走腐败检查」
- **共享类型 / event**：复用了哪些已有 component / event / schema？另建一份的话理由是什么？（防止"又造一个 `BreakthroughEvent`"）
- **跨仓库契约**：server / agent / client 各自命中的 symbol（IPC schema 名 / Redis key / CustomPayload type ID）
- **worldview 锚点**：这个玩法对应 `worldview.md` 哪一节？（境界？经济？传承？阵法？）
  没锚点的玩法要么补 worldview、要么不该立。
- **qi_physics 锚点**：玩法涉及真元 / 灵气 / 衰减 / 逸散 / 半衰 / 距离损耗 / 排斥 / 吸力的，必须列出调用了 `qi_physics`(见 `plan-qi-physics-v1`) 的哪些函数 / 常数。新引入的物理常数必须先扩 `qi_physics` 而非本 plan 内写——本 plan 只声明物理参数（注入率、纯度、容器类型等），底层公式归 `qi_physics` 唯一实现。worldview §二「真元极易挥发」是全局唯一物理入口。

## 三、调研工具

```bash
grep -rn "<关键词>" docs/finished_plans/          # 历史 plan 处理过没
grep -rn "<模块名>" server/src/<其他模块>/         # 实际代码哪些在引用
grep -rn "<EventName>" server/src/                # 同名 / 近义 event 检查
```

- `/plans-status [关键词]` 快速看代码↔文档实装差异
- `/audit-plans-progress` 全量审进度（多 agent 并发 grep + git log）
- `/library-lore` 查阅 `docs/library/` 馆藏，写世界观 / 编书籍前用

## 四、孤岛红旗（出现就停下重设计）

立 plan 时遇到以下任一情况，**停下来重看 §一、§二**：

- **自产自消自存**：新模块跟 `inventory` / `cultivation` / `combat` / `agent` 都没接口，单机闭环
- **近义重名**：新增 component / event 跟已有命名重叠（例：又造 `BreakthroughEvent` 不复用 `cultivation::BreakthroughEvent`）
- **无 worldview 锚点**：纯"觉得这样好玩"加的玩法，找不到 worldview 章节对应
- **skeleton 已有同主题却没合并**：开新版本号 / 改方向却没在 plan 头部说明为什么不并入既有骨架
- **跨仓库契约缺一面**：只动 server 不动 agent / client（除非确实是纯服务端模块），或者只加 schema 不在两端 import
- **自定真元 / 灵气物理常数或公式**：新模块出现 `*_DECAY*` / `*_EXCRETION*` / `*_DRAIN*` / `*_ATTEN*` / `*_HALF_LIFE*` / `RHO` / `BETA` / 形如 `0.0X_f64` 的"看起来像衰减率"的常数 / `fn ..._decay()` `fn ..._excretion()` 等衰变函数 → **必查 `qi_physics`**(`plan-qi-physics-v1`)。已存在就调用，不存在就**先扩 qi_physics 再 import**，**禁止 plan 自己写一份**。同源现象（worldview §二「真元极易挥发」）只允许一份代码实现——目前正典 0.03/格 vs `combat/decay.rs` 硬编 0.06、shelflife 5 套独立 profile、tsy_drain 与 dead_zone 两套互不相识的衰减公式，就是各 plan 自己拍数留下的烂账
- **自定真元生成 / 释放路径，绕过守恒律**：worldview §二/§十 正典「全服灵气总量 `SPIRIT_QI_TOTAL` 恒定；修炼消耗 = 别人少掉」（const 当前 100.0，暂定可配置——**测试断言取 const 引用，不写字面 100**）。代码里所有真元/灵气流动**必须**走 `qi_physics::ledger::QiTransfer { from, to, amount }`——任何 `cultivation.qi_current += X`（无对应 zone 减）、`zone.spirit_qi -= Y`（无对应玩家增）、容器衰变把真元"凭空消失"（不归还 zone）、招式释放只扣攻方不写入环境，**都是守恒律红旗**。释放走 `qi_release_to_zone(amount, region, env)`，吸收走 `qi_excretion(initial, container, elapsed, env)`（已 clamp 到 zone 浓度下限符合压强法则）。唯一允许的"系统外流出"= 天道每时代衰减 1-3%（`QI_TIANDAO_DECAY_PER_ERA_*`），这条不是 plan 自由度。坍缩渊吸入也是中转站不是终点——塌缩时走 `collapse_redistribute_qi`，不消失
- **视听不完整或一笔带过**：任何涉及玩家可感知行为的 plan（招式 / 状态变化 / 采集 / 炼制 / 阵法 / 世界事件 / 灾劫等）的视听规格**必须写到能直接实现的精度**——"粒子：绿色雾气"是不合格的。具体要求：
  - **粒子**：必须写明基类（`BongLineParticle` / `BongRibbonParticle` / `BongSpriteParticle` / `BongGroundDecalParticle`）、数量、lifetime tick、速度/方向、颜色 hex、spawn 模式（burst / continuous / radial）、贴图 ID（新增还是复用）、VfxPlayer 类名、`bong:vfx_event` ID
  - **音效**：必须写明 audio_recipe JSON 结构——每层的 vanilla sound ID（`entity.xxx.yyy`）、pitch、volume、delay_ticks；不允许写"雷鸣音效"一笔带过
  - **HUD / 屏幕效果**：必须写明 HudRenderLayer、overlay 类型（vignette / tint / shake）、颜色 hex + opacity、持续时间 tick、fade in/out 曲线、受影响境界范围
  - **天象 / 环境**：必须写明天空色温变化的具体 RGB shift、雾气浓度变化、方块替换规则（哪些 block → 哪些 block）、terrain profile 变更（永久还是临时）
  - **动画**：必须写明 PlayerAnimator JSON 的 endTick、关键骨骼姿态（pitch/yaw/roll/bend 弧度值）、body 位移、easing 函数；或指定 gen_*.py 生成脚本名
  - **narration 模板**：必须写出 2-3 条具体的 narration 文案示例（不只是"天道会说一句话"），且标明 scope（broadcast / zone / player）和 style（perception / narrative / dialogue）
  
  视听规格**必须内联在对应机制的阶段块中**（跟着 P0/P1/... 一起写），不允许全部推到一个单独的"P-视听"阶段——那样做会导致视听与机制脱节，实施时才发现冲突。每个机制写完 server 逻辑后紧跟它的视听规格，是同一个交付物的两面。
  
  纯 server 逻辑 plan 无此要求（如 qi_physics / persistence / schema 对齐等）
- **招式注册不声明依赖经脉**：新增 `SkillRegistry::register` / `register_skills` 调用未在 `cultivation::meridian::severed::SkillMeridianDependencies::declare(skill_id, vec![...])` 注册依赖经脉的 → 经脉永久 SEVERED 时该招式不会被通用 `check_meridian_dependencies` 拦截 → 玩家断了肺经的飞剑手仍能 cast 飞剑（worldview §四:286 物理可见性破坏）。**必查 `plan-meridian-severed-v1`**（`docs/finished_plans/`）+ §3 流派依赖经脉清单 + `cultivation::meridian::severed` 模块 trait。所有 v2 流派 plan / 未来招式 plan 注册时必走 `.declare(...)`；漏写 = 红旗，与 qi_physics 同级强约束

---

> 一句话原则：**新 plan 的第一段不应该是"我要做 X"，而应该是"我要做 X，它从 A/B 进料、向 C/D 出料、对应 worldview §N"**。

---

## 五、Plan 演进：开放问题 pre-P0 收口模式

plan 立完后常带 **§N 开放问题**（设计未决 / 数值待校准 / 接口待选）。**严禁带着开放问题进 P0 实施**——agent 自动消费时会替你拍板，方向走偏后回退成本巨大。

### 5.1 §N → §N.1 决议模式

每份 plan 在最后留一节 `## §N 开放问题（P0 决策门前需收口）`（建议 §8 / §最末数字），列出所有未决项。**实施前必须**追加一节 `## §N.1 决议（pre-P0 收口，YYYY-MM-DD）`，每条开放问题对应一段决议，结构如下：

```markdown
### #M <问题简述>

**决议**：
1. <核心结论，一句话>
2. <实施方案，含具体函数 / 字段 / 数值>
3. <边界条件 / 拒绝某条路线的理由>

**落点**：`server/src/.../foo.rs:123-145`（依据代码）/ `client/src/.../Bar.java:67`（依据代码）/ plan §X.Y（涉及修改的章节）
```

**关键约束**：

- **每条决议必须落到「文件:行号 + plan 章节」双锚点**——不允许"建议这样"含糊收尾。落点是 P0 实施时直接抓的入口。
- **决议数据必须靠 Explore agent 并行核查代码现状产出**，不能拍脑袋——经脉效率公式、状态机暴露接口、worldgen 现有 zone 范本、botany spawn 速率，全部要 grep 代码确认。
- **决议触发 plan 章节更新时同步修改**——比如 §8.1 #1 改了某个数值，plan 头部对应表格也要同步打补丁注释「数值见 §8.1 #1」。
- **§N 原表保留作历史回溯**，但末尾必须写「全部已在 §N.1 收口。原表保留以备追溯，**实施时以 §N.1 决议为准**」。

### 5.2 启动方式

```
1. 立完 plan，写完 §N 开放问题
2. 并行起 3-4 个 Explore agent，每个 agent 负责 2-3 个开放问题
   - Explore agent prompt 必须含「只读、不改文件」+ 明确问的事 + 输出格式（每段 200-400 字 + 文件:行号引用）
3. 收齐 Explore reports → 合成 §N.1 决议章节追加到 plan
4. 若决议触发新问题（比如发现现有代码与 plan 假设不符），回到步骤 1 列入 §N
5. 全部收口才能开 P0
```

---

## 六、Plan 消费规范：写入 plan §10 章节

任何 scope ≥ 4 PR 的 plan，**必须在 plan 末尾写一节 §10 实施工作流**，写清以下 5 条。consume-plan agent 跑这份 plan 时按 §10 执行；这是对 `commands/consume-plan.md` 通用流程在该 plan 特殊场景下的细化，不是替代。

### 6.1 建筑 / 视觉资产类：3 轮自我打磨 + `<PROMISE>` 担保

任何涉及 NBT 建筑搭建、worldgen layout placement 摆位、复杂视觉资产产出的 TODO，**禁止一次 commit 完成**。强制：

1. **Round 1 first cut** → commit message 标 `(round 1/3)`
2. **Round 2 自我 review**（截图渲染 / structure dump / ASCII 平面投影验证布局）→ 修 → commit `(round 2/3)`
3. **Round 3 终轮 review**（与 spec 一致性 + 视觉叙事检查）→ 修 → commit `(round 3/3)`
4. 终轮 commit message 末尾写 `<PROMISE>` 担保块（**注意拼写：PROMISE 不是 PROMIS**）：

   ```
   <PROMISE>该建筑(/layout/...) 已经过 3 轮自我打磨 + review，达到当前能力上限。
   已检查：[比例对称 / 入口朝向 / 内部连通 / 视觉叙事 / spec 一致]
   仍存在的局限：[一两条诚实承认的不足]</PROMISE>
   ```

   `<PROMISE>` 不是免责声明，是"已尽全力"的可追溯信号——后续 review 仍按严重性修，但不再要求继续打磨。

**纯逻辑代码 TODO 不适用本节**——按常规 atomic commit + 测试全绿即可。

### 6.2 Worldgen 建筑场地：deterministic layout，不用 noise density

**人工建筑遗迹**（宗门废墟 / 古殿 / 阵法布局）**禁止用 density-based noise spawn**——必须 deterministic layout：

- terrain profile schema 加 `architectural_layout: "<layout_id>"` 字段 + `height.compound_flatten_radius: <半径>`（POI 周围强制摊平到固定高程）
- 新建 `worldgen/scripts/terrain_gen/layouts/<layout_id>.py`，用 `LayoutSpec` 定义 `Placement` 列表（坐标公式 / NBT 投放 / block_grid stamp），相对 POI 中心点摆放
- layout 半径内 stitcher density spawn mask 自动遮蔽，避免野草长到建筑屋顶
- 自然杂物（野草 / 散落骨片 / 小石头）仍走 `DecorationSpec` density spawn，只覆盖 layout 半径外区域

**典型 layout 公式**：八卦布局（内外两环 × 8 方位 + 22.5° 偏转）/ 中轴对称（沿 z 轴 ±N 格） / 网格 plant_grid（药圃每格按规律种一种灵草）。布局公式驱动而非随机，是「人工建筑」与「自然地貌」的本质区分。

**测试要求专属**：layout determinism（同 seed 两次跑坐标完全一致） / region density mask（layout 半径内不出杂物） / flatten_radius（POI 周围高程恒定）。

### 6.3 单 plan 多 PR 序列化（vs 拆多 plan）

scope ≥ 4 PR 的 plan，**不拆多 plan**——单 plan 内分多个 PR 序列化提交。理由：拆 plan 增加 plan 文件管理成本（多份 active + 维护交叉引用），且子 plan 体量通常不够立项门槛。

§10.2 应明列推荐拆分点（依赖顺序，前一个 merge 后开下一个），例如：

1. **PR-1 基础设施**：worldgen / schema / 底层框架扩展（独立成 PR 避免与玩法 PR review 混杂）
2. **PR-2 核心系统底盘**：纯 server 逻辑 + IPC schema
3. **PR-3 资产 / UI**：依赖 PR-1/2 的视觉资产 + 客户端渲染
4. **PR-4 集成 / 平衡 / BOSS**：依赖前 3，集成测试 + 数值校准

**唯一例外**：`docs/worldview.md` 修改必须单独 PR，CLAUDE.md / AGENTS.md 严禁 agent 自动改 worldview，必须人工 review。归档前必须先 land。

### 6.4 PR 实施用独立 subagent（context 隔离）

**主线 agent 不亲自跑 PR 实施**——每个 PR 起独立 subagent，主线只接收 subagent result（200-500 token），实现"每 PR 后自动清理 context"。

**强制配置**（写入 plan §10.5）：

```
Agent(
  subagent_type: "claude",          # catch-all + 全工具集（Edit/Write/Bash/gh 可用）
  model: "opus",                    # 强制 Opus 4.7（最强模型）
  prompt: "...任务...\n\nultrathink"  # 末尾 ultrathink 触发最高思维 budget（≈ "xhigh"）
  # isolation 不用 worktree（共享主 worktree 避免 nested）
)
```

**主流程**（伪代码）：

```
for pr_n in [PR-1..PR-N]:
    result = Agent(...subagent, prompt 含本 PR 范围 + 必读 §10.1 多轮 + 测试要求...)
    pr_url = parse(result)
    # 等 CR review（§6.5）
    while gh pr checks pr_url == "pending":
        ScheduleWakeup(1200, "等 CR PR #N")
    if has_review_issues:
        Agent(...修复 subagent...)  # 修复也用独立 subagent
        重等
    gh pr merge --squash --delete-branch
归档 plan
```

**context 估算**：主线亲自跑 4 PR ≈ 200k token；subagent 模式 ≈ 2-5k token（实质等价于"每 PR 后清理"）。

**关键约定**：
- `subagent_type: "claude"`（不要 general-purpose / Explore——前者语义偏研究、后者只读）
- `model: "opus"` 显式（不要 sonnet/haiku，实施 + 多轮 review 需要顶级模型）
- prompt 末尾 `ultrathink`（思维 budget 阶梯 `think` < `think hard` < `think harder` < `ultrathink`）
- subagent 只负责**实施 + 提 PR**，**不等 review**（subagent 是 single-call，没有跨调用 ScheduleWakeup 能力；等待逻辑归主线）
- 主线 merge 命令简单不消耗 context，主线亲自做

### 6.5 CodeRabbit ScheduleWakeup 等待协议

CodeRabbit 是 GitHub Actions check run，`gh pr checks <PR>` 看状态：

| 状态 | 含义 | 动作 |
|------|------|------|
| `pass` | review 通过 | 进 merge |
| `pending` | 仍在跑（典型 ~20 min） | `ScheduleWakeup delaySeconds=1200` 等下回合 |
| `fail` | 不通过 | 按 commands/consume-plan.md step 7 严重性桶处理 |

**等待节奏硬约束**：

- **禁止 sleep loop / busy poll**——必须 `ScheduleWakeup`
- 每回合 1200s（20 min，对齐 CR 单回合典型耗时）
- 最多 3 回合 = 总 60 min 卡死才停交人工
- 修完 review 意见**必须重新等 CR re-review**，不自行判定"我修好了应该过"（对齐 memory `feedback_wait_coderabbit_approve.md`）
- 多 PR 场景每个 PR 各自走完整等待协议，前一个未 APPROVED/收敛不开下一个

### 6.6 §10 章节模板

新立 plan 的 §10 章节按本指南 §六 各小节顺序写（建筑多轮 / 多 PR / subagent / CR 等待），最末加一节 **§10.N 单次 consume-plan 全自动到 merge**，重申"用户提交 `/consume-plan` 后即可下班，醒来看 plan 是否在 finished_plans/"。

可参考 `docs/plan-dandao-path-v1.md` §10（2026-05-18 首次实践）作为模板，复制结构 + 按本 plan 实际范围替换具体内容。
