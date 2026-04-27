---
name: library-lore
description: 查阅 docs/library/ 馆藏书籍，按三层策略避免 agent "吃书"（遗忘/幻觉）。写世界观、编书籍、实现机制时触发。
---

# 馆藏查书 · 三层策略

## 核心原则

馆藏 JSON 每条记录有三层结构。Agent 按需逐层深入，**不一次性吞全文**。

| 层 | 字段 | 用途 | 何时用 |
|---|---|---|---|
| 一 | `essence` + `title` + `catalog` | 一句话事实，≤80 字白话 | 批量注入上下文、交叉引用 |
| 二 | `synopsis` | 一段话展开，100-300 字白话 | 需要更多因果/人物/地点时 |
| 三 | `sections[]` / `fragments[]` / `crossRefs[]` / `implementation` | 原文全文 | 需要逐章细节或落代码时 |

`essence` 和 `synopsis` 是**白话精炼**，不是古文。古文只留在层三原文里。

## 查书流程

### 1. 定位书籍

```bash
# 列出所有馆藏
find docs/library/ -name '*.json' ! -name '馆藏条目模板.json'

# 按关键字搜 title/essence
grep -rl '<keyword>' docs/library/*/
```

### 2. 逐层提取

```bash
# 层一：注入上下文用（token 极省）
jq '{title, essence, catalog: {hall, shelf, id}}' docs/library/<分馆>/<书名>.json

# 层二：需要细节时追加
jq '{title, synopsis}' docs/library/<分馆>/<书名>.json

# 层三：原文全文（仅必要时）
cat docs/library/<分馆>/<书名>.json
```

### 3. 交叉引用追踪

```bash
# 查哪些书引用了目标锚点
grep -rl '<worldview.md 章节>' docs/library/*/
```

## Agent 行为约束

- **写世界观/编新书前**：必须先 grep 定位相关馆藏 → 批量读取层一确认不冲突
- **实现机制前**：读层一 + 层二，确认理解后再动代码
- **不确定细节时**：读层三 `sections[]` 对应章节
- **禁止**：凭记忆编造馆藏已有的事实，必须查了再写
- **禁止**：一次性 `cat` 多本书全文灌入上下文
