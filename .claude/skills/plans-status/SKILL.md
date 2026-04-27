---
name: plans-status
description: 扫描 docs/ 下 plan-*.md，subagent 实地核验代码（grep+git log）对照 plan 自报状态，输出"文档↔代码"差异化进度。用法：/plans-status [关键词]
argument-hint: [可选关键词，过滤 plan 名，如 combat / cultivation]
allowed-tools: Agent Bash Glob
---

# Bong 项目 — Plan 进度速览（实地核验版）

扫描 `docs/` 下 plan 文档，但 **不相信 plan 自报**——subagent 必须 grep / Read / git log 落到代码里核对，对照文档与现实的差异。

> **核心原则**：plan 文档声称 "P1 ✅" 不算数，必须在 server/agent/client/worldgen 里找到对应模块/类型/测试才算数。差异（文档超前、代码超前、虚标）必须显式标出。

---

## 参数

用户输入：`/plans-status [关键词]`

- 无参数：扫描全部 plan（成本较高，建议先 filter）
- 有关键词：只看文件名含关键词的 plan（如 `combat` → combat-no_ui + combat-ui_impl）

---

## 流程

### 第一步：列出 plan 文件

```bash
ls docs/plan-*.md docs/plans-skeleton/plan-*.md docs/finished_plans/plan-*.md 2>/dev/null
```

关键词过滤用 `grep -F`。

### 第二步：分发给 subagent（Explore 类型）

用 **单条 Agent 调用** 并发处理。如果筛后只剩 1-3 份 plan，可改为每份 plan 一个 Agent 并发跑（更彻底）。

Prompt 模板（**重点**）：

> 核对 Bong 项目以下 plan 的 **实际** 进度。**不要相信 plan 文档自报的状态**——必须 grep / Read / git log 落地到代码里核验，把"文档声明"和"代码现实"显式对比。
>
> 项目布局：`server/`（Rust/Valence）`agent/`（TS/Node）`client/`（Java/Fabric）`worldgen/`（Python）。
>
> 对每份 plan：
>
> **1. 抽 plan 自报的进度信号**
> - 读头部 80 行 + 尾部 50 行 + 任何 `## 进度` / `## 验收` / `P0/P1/§N` / `✅⏳⬜` / `YYYY-MM-DD` 段落
> - 提取：主题（一句话）；文档自报阶段状态；**文档点名的具体交付物**（模块名 / 文件路径 / struct/enum/fn 名 / 测试名 / schema 名 / Redis key / 配置字段）
>
> **2. 代码侧实地核验**（**每个声称 ✅ 的阶段都要查**）
> - `grep -rn '<symbol>' server/ agent/ client/ worldgen/ --include='*.rs' --include='*.ts' --include='*.java' --include='*.py'` 看声明的 symbol 是否真在
> - `find server/src -name 'foo.rs'` / `Glob` 验证声明"新增 N 个模块"是否真出现
> - `git log --oneline -n 30 -- <相关路径>` 看最近 1-2 周有无对应 commit；提取最近一条 commit 的日期+短消息
> - 测试声明（"228 测试通过"等数字）：跑 `grep -rc '#\[test\]' server/src/<相关 mod>` 或 `grep -rc '^test(' agent/packages/<pkg>` 估算数量级，不要求精确
> - **未声称 ✅ 的阶段（⏳/⬜）**：用 grep 快速扫一遍，看是否已"代码超前于文档"
>
> **3. 对比并定级**
> - 文档 ✅ + 代码命中 → `✅ 已验证`（附 1-2 个命中证据：模块路径 / commit hash）
> - 文档 ✅ + 代码找不到 → `⚠️ 文档自报已完成但代码未找到` + 列出失踪 symbol
> - 文档 ⏳ + 代码大量命中 → `🔄 代码超前于文档`
> - 文档 ⬜ + 代码已有 → `🔄 代码超前于文档`
> - 文档 ⏳/⬜ + 代码也无 → `⬜ 一致`
>
> **4. 每份 plan 输出（≤ 500 字）**
>
> ```
> plan-xxx-vN.md — <主题一句话>
> 阶段核验：
>   - P0: 文档 ✅ → 代码 ✅ 已验证（mod foo @ server/src/foo.rs / 5 个 test）
>   - P1: 文档 ⏳ → 代码 🔄 超前（mod bar 已存在 + 12 个 test）
>   - P2: 文档 ⬜ → 代码 ⬜ 一致
> 最近 git 活动：YYYY-MM-DD <hash> <commit msg>
> 差异/风险：<一句话；没有就写 "无">
> 下一步（plan 里写明的）：<一句话>
> ```
>
> **文件列表**（绝对路径）：
> {全部 plan 路径}
>
> **不要做**：
> - 不要复述 plan 设计正文 / 不要解释每个 P0/P1 在做什么
> - 不要只 Read 不 grep，必须有代码侧证据
> - 不能确认的标 `状态未知`，**不要编**

**重要**：用 `Explore` subagent。让主上下文只接收最终对比报告，不接触 plan 正文 / 代码原文。

### 第三步：主上下文格式化输出

subagent 返回后，主上下文按以下模板重组：

```markdown
## Bong 项目 Plan 进展（实地核验）

### 进行中（docs/）
- **plan-xxx-v1.md** — <主题>
  - 阶段：P0 ✅✓ / P1 ⏳ → 🔄 代码超前
  - 最近活动：YYYY-MM-DD <hash>
  - <差异 or 下一步>

### 骨架（docs/plans-skeleton/）
- **plan-xxx-v1.md** — <主题> — <状态>

### 已完成归档（docs/finished_plans/）
- plan-xxx.md — <主题>

### 红旗（文档 ↔ 代码不一致）
- **plan-yyy-v1.md** — 文档声明 P2 ✅，但 grep 找不到 `XxxManager` / 相关 commit 也无
- ...

### 重点摘要
- 目前主推：<2-3 个 plan>
- 刚完成（已核验）：<最近归档或验收>
- 下一瓶颈：<阻塞点>
```

"红旗"小节是这版 skill 的关键产出，**没有红旗就显式写"无"**。

---

## 不要做的事

- **不要只信文档**。这版 skill 的全部价值在于代码核验。subagent 没 grep 就给结论 = 失败
- **不要在主上下文直接 Read plan 文件**。必须 spawn Agent
- **不要输出 plan 原文设计内容**。只输出进度状态、核验结果、差异
- **不要虚构进度或证据**。grep 没命中就标 `⚠️ 未找到`，不要补脑
- **不要顺手改 docs/plans-progress.yaml 或 README**——那是 `/audit-plans-progress` 的活，本 skill 只读不写

---

## 与 `/audit-plans-progress` 的边界

| | plans-status（本 skill） | audit-plans-progress |
|---|---|---|
| 触发 | 想快速看现状 | 周期性大审核 |
| 子代理 | 单 Agent 批处理 / 小批量并发 | 多 sonnet 并发 |
| 写盘 | **不写** | 重写 plans-progress.yaml + 更新 README |
| 输出 | 控制台对比报告 | 持久化 + 控制台 |
| 成本 | 中（要 grep 但不写盘） | 高 |

不要在 plans-status 里做 audit 该做的持久化工作。

---

## 使用场景

- 开新会话前快速对齐"代码到底走到哪一步"——尤其当怀疑 plan 文档没及时更新时
- 决定下一步推进哪个 plan（看 🔄 代码超前的就知道哪些 plan 应该补文档/收口）
- 检查某个子系统（combat / cultivation / worldgen）的真实阶段
- 周报前先用本 skill 看红旗，再用 `/audit-plans-progress` 落地
