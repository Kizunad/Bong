---
name: plans-status
description: 扫描 docs/ 下所有 plan-*.md，用 subagent 并发摘要，输出"进行中/已完成/骨架"分组的进度总览。用法：/plans-status [关键词]
argument-hint: [可选关键词，过滤 plan 名，如 combat / lingtian]
allowed-tools: Agent Bash Glob
---

# Bong 项目 — Plan 进度速览

扫描 `docs/` 下所有 plan 文档，汇总当前进展状态，分组输出。主上下文不直接读 plan 正文——全部交给 subagent 并发摘要，避免污染主对话。

> **核心原则**：只读头部和进度章节（P0/P1/§N 完成情况、验收日期），不读正文设计细节。

---

## 参数

用户输入：`/plans-status [关键词]`

- 无参数：扫描全部 plan
- 有关键词：只输出文件名含关键词的 plan（如 `combat` 只看 combat-no_ui + combat-ui_impl）

---

## 流程

### 第一步：列出 plan 文件

```bash
# 主上下文只做 glob，不读正文
ls docs/plan-*.md docs/plans-skeleton/plan-*.md docs/finished_plans/plan-*.md 2>/dev/null
```

如果用户给了关键词，用 grep 过滤文件名。

### 第二步：分发给 subagent（Explore 类型）

用 **单条 Agent 调用** 并发摘要所有 plan。Prompt 模板：

> 核对 Bong 项目以下 plan 文档的进度状态。对每个文件：
>
> 1. 读取头部 50 行和最后 30 行（或进度章节/验收记录段落）
> 2. 提取：
>    - 主题（一句话）
>    - 当前进度（P0/P1/§N 状态，已完成阶段 ✅、进行中 ⏳、未开始 ⬜）
>    - 最近验收或更新日期（若有）
> 3. 每份 plan 输出一行：`plan-xxx-vN.md — <主题> — <P0✅ P1⏳ / 全部完成 YYYY-MM-DD>`
>
> 文件列表：
> {列出 glob 到的所有文件绝对路径}
>
> 不要读正文设计细节，只读进度相关段落。范围控制在 500 字以内。

**重要**：一定用 `Explore` subagent（不要 general-purpose），并发读取，让主上下文只接收最终摘要。

### 第三步：格式化输出

subagent 返回后，主上下文按以下模板重组（不要原样转发）：

```markdown
## Bong 项目 Plan 进展

### 进行中（docs/）
- **plan-xxx-v1.md** — <主题> — <状态>
- ...

### 骨架（docs/plans-skeleton/）
- **plan-xxx-v1.md** — <主题> — <状态>

### 已完成归档（docs/finished_plans/）
- plan-xxx.md — <主题>
- ...

### 重点摘要
- 目前主推：<2-3 个 plan>
- 刚完成：<最近归档或验收>
- 下一瓶颈：<阻塞点>
```

---

## 不要做的事

- **不要在主上下文直接 Read 多个 plan 文件**。必须 spawn Agent。一份 plan 动辄 1-2k 行，读 20 份会把主上下文撑爆。
- **不要输出 plan 原文设计内容**。只输出进度状态和主题一句话。
- **不要虚构进度**。subagent 没找到明确进度标记的，标 `状态未知` 或 `草案`。
- **不要读 worldview/roadmap/architecture 等非 plan 文档**，除非用户明确要求。

---

## 使用场景

- 开新会话前快速对齐项目状态
- 决定下一步推进哪个 plan
- 检查某个子系统（如 combat、cultivation）的阶段进度
- 周报/里程碑汇报前收集状态
