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

---

> 一句话原则：**新 plan 的第一段不应该是"我要做 X"，而应该是"我要做 X，它从 A/B 进料、向 C/D 出料、对应 worldview §N"**。
