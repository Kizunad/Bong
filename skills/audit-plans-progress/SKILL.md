---
name: audit-plans-progress
description: 实地大审核所有 plan 进度，用多个 sonnet subagent 并发 grep+git log+文档对照，重写 docs/plans-progress.yaml 后跑 scripts/plans_progress.py 更新 README。用法：/audit-plans-progress [group]
argument-hint: [可选 group：tsy / combat-vfx / cultivation-economy / gameplay-npc / infrastructure / skeleton。无参=全审]
allowed-tools: Agent Bash Read Edit Write
---

# Bong 项目 — Plan 进度实地审核

把 `docs/plans-progress.yaml` 从"手工维护"升级为"按需重新核对"。一次跑完会重写 YAML，再渲染 README，让 dashboard 反映最新真实代码状态。

> **核心原则**：subagent **必须实地核对代码**（grep、git log、文件存在检查），不能只看 plan 自述。代码与文档不一致时，以代码为准，YAML 的 `summary` 字段记录差异。

---

## 何时跑

- 大批量 PR 合并后（如本周一次性 merge 多个 plan 实装）
- 里程碑汇报前需要准确进度
- 长时间没维护 YAML，想批量纠偏
- 怀疑某个 plan 的 dashboard 数字不准（针对单 group 跑）

**不要跑**：单 PR merge 后想立刻刷 dashboard —— 直接 edit YAML 中那一条更快、更准。

---

## 参数

`/audit-plans-progress [group]`

- 无参数：全审 5 组（约 38 份 active plan，骨架和归档不需要审）
- `tsy`：只审 TSY 系列 10 份
- `combat-vfx`：只审战斗/HUD/视觉 8 份
- `cultivation-economy`：只审修炼/经济 8 份
- `gameplay-npc`：只审玩法/NPC 7 份
- `infrastructure`：只审基础设施 5 份

---

## 流程

### 第一步：读现状

```bash
# 看现有 YAML 中 plans 数组的分组和 file 清单（不要读全文）
grep -E "^\s+- file:|^\s+group:" docs/plans-progress.yaml
```

记下要审的 plan 文件清单（按用户给的 group 过滤）。

### 第二步：分组并发 spawn sonnet Explore subagent

**重要**：用 `model: "sonnet"`，`subagent_type: "Explore"`。**每组一个 subagent，全部 run_in_background=true**，让主上下文不被堵塞。

每个 subagent 的 prompt 模板（按需替换 `<group_id>` 和 `<file_list>`）：

````
你是 Bong 项目（AI-Native Xianxia Minecraft 沙盒）的 plan 进度审计 agent。

## 任务

核对 <group_id> 组 N 份 plan 的"文档自述 vs 实际代码"对齐状态。每份 plan
既要读文档自述，也要 grep 仓库代码确认真实落地情况。

## 文件列表

<file_list>

## 核对方法

每份 plan：
1. 读头部 50 行（标题 + 摘要） + §进度日志/§Audit 章节（通常文件末尾）
2. 抽 plan 关键 struct/enum/system/contract 名字 + 关键文件路径
3. **grep 实际代码确认**：
   - server/src/ 下 grep -r "<名字>"
   - client/src/main/java/ 下同样
   - agent/packages/、worldgen/scripts/ 等其他相关子目录
   - git log --oneline | grep "<plan-name>" 找 PR 落地记录
4. 综合判断真实进度百分比（0-100），代码与文档不一致时以代码为准

## 输出格式（严格 YAML，每份 plan 一段）

```yaml
- file: plan-xxx-v1.md
  group: <group_id>
  title: "<主题一句话 ≤30 字>"
  state: "<merged | active-implementing | active-design | skeleton>"
  percent: 65
  last_updated: "YYYY-MM-DD"
  pr_refs: [N, M]
  blocking_on: ["plan-xxx-v1", ...]
  summary: "<grep 验证后 1-2 句描述代码状态 + 文档差异（如有），≤120 字>"
```

按 file 字母序排。范围控制：每条 ≤200 字。**最关键是 percent / state /
pr_refs 准确**。

并发读取所有文件 + 并发 grep 加速。
````

**state 字段约定**：
- `merged`：核心代码已 merged 主线，plan 主体落地（百分比通常 ≥80）
- `active-implementing`：部分代码已落地，仍在推进（百分比 30-80）
- `active-design`：设计 active，零或近零代码（百分比 ≤20）
- `skeleton`：在 plans-skeleton/ 目录（百分比固定 5-10）

### 第三步：等所有 subagent 完成

每个 subagent 通过 task notification 通知完成。**不要 sleep 轮询**。所有完成后进入第四步。

### 第四步：聚合 + 写入 YAML

收齐所有 subagent 的 YAML 片段后：

1. **保留 YAML 元字段**：`generated_at`（更新为今日）、`project`、`groups`（不动）
2. **替换 `plans` 数组**：
   - 全审模式：替换全部 plans
   - 单组模式：只替换该 group 的 plan，其他 group 保持不变（用 Edit 工具按条目替换）
3. **不动**的条目：`group: skeleton` 和 `group: finished` 部分（除非用户显式 `/audit-plans-progress skeleton`）

写入手段优先级：
- 单组审：用 Edit 工具按条目替换（精确）
- 全审：用 Write 工具重写整个 YAML（更省 token）

### 第五步：渲染 README + 报告

```bash
python3 scripts/plans_progress.py
git diff --stat README.md docs/plans-progress.yaml
```

输出给用户：
- 哪些 plan 的 percent / state 变了（before → after）
- 哪些 plan 是新核对发现的差异（如代码已落地但文档没更新）
- 总进度变化（旧总进度 → 新总进度）

---

## 输出格式（给用户）

```markdown
## Plan 进度审核报告（YYYY-MM-DD）

### 总进度变化
- 旧：X.X% → 新：Y.Y%

### 状态升级（实装超过文档）
- **plan-xxx-v1**：active-design / 8% → merged / 90%（PR #NN 已合并，文档进度日志未跟）

### 状态降级（文档高估）
- **plan-yyy-v1**：merged / 95% → active-implementing / 70%（核心代码缺失 X/Y）

### 无变化
- N 份 plan 状态不变（list 文件名）

### 已写入
- docs/plans-progress.yaml（M 条更新）
- README.md（已渲染）
```

---

## 不要做的事

- **不要在主上下文 Read 多个 plan 文件**。一份 plan 1-2k 行，读多份会撑爆主上下文。**所有 plan 内容只在 subagent 里读**。
- **不要不 grep 直接信文档**。subagent 必须实地核对代码，否则审核失去意义。
- **不要改 `groups` 元字段**。groups 定义是 schema 的一部分，不能因审核而变。
- **不要改 finished 组**。已归档 plan 100% 完成是定论。
- **不要 commit**。脚本只更新 YAML 和 README，是否 commit 由用户决定。
- **不要重复跑** scripts/plans_progress.py 多次。一次 audit 跑一次足够。

---

## 错误处理

- 某个 subagent 报告 YAML 格式错误：要求重新输出该组（或主上下文兜底解析）
- subagent 漏了某个 plan 文件：单独再 spawn 一个 subagent 补
- YAML 写入失败（语法错误）：先 git stash，让用户决定回滚

---

## 使用场景

- 周/双周大审：无参数全审，~5 分钟拿到准确 dashboard
- 单组聚焦：刚集中 merge 完 TSY 系列，跑 `/audit-plans-progress tsy`
- 怀疑 dashboard 不准：跑相关 group 验证

---

**注意**：本 skill 是 `plans-progress.yaml` 的**审核机制**，与简单查看进度的 `/plans-status`（只读 plan 自述）不同。前者会写文件，后者只输出。
