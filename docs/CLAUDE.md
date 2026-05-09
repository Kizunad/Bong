# Plan 制定指南（防孤岛）

立新 plan 之前的调研流程。**根 `CLAUDE.md` 的"Plan 工作流"讲三态流转和 plan 文件本身的结构**；本文件讲**立 plan 之前要做什么调研**，确保新模块跟现有玩法接得上、不自成孤岛。

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
- **无独特视听体验**：任何涉及玩家可感知行为的 plan（招式 / 状态变化 / 采集 / 炼制 / 阵法等）**必须包含差异化的 Audio + 粒子 + HUD/UI 设计**，不能只写 server 逻辑不管客户端体验。每个招式 / 状态 / 交互必须有**专属**的动画姿态、粒子效果、音效 recipe、HUD 组件——不允许多招共用同一套视觉反馈（参见根 `CLAUDE.md` 中「招式音效/特效/HUD 区分硬约束」）。纯 server 逻辑 plan 无此要求（如 qi_physics / persistence / schema 对齐等）
- **招式注册不声明依赖经脉**：新增 `SkillRegistry::register` / `register_skills` 调用未在 `cultivation::meridian::severed::SkillMeridianDependencies::declare(skill_id, vec![...])` 注册依赖经脉的 → 经脉永久 SEVERED 时该招式不会被通用 `check_meridian_dependencies` 拦截 → 玩家断了肺经的飞剑手仍能 cast 飞剑（worldview §四:286 物理可见性破坏）。**必查 `plan-meridian-severed-v1`**（`docs/finished_plans/`）+ §3 流派依赖经脉清单 + `cultivation::meridian::severed` 模块 trait。所有 v2 流派 plan / 未来招式 plan 注册时必走 `.declare(...)`；漏写 = 红旗，与 qi_physics 同级强约束

---

> 一句话原则：**新 plan 的第一段不应该是"我要做 X"，而应该是"我要做 X，它从 A/B 进料、向 C/D 出料、对应 worldview §N"**。
