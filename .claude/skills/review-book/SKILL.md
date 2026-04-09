---
name: review-book
description: 审核一篇末法残土图书馆馆藏条目（JSON 格式）。检查结构、世界观对齐、交叉引用，输出审核报告并修复问题。用法：/review-book <文件路径>
argument-hint: <文件路径>
allowed-tools: Read Edit Bash Glob Grep
---

# 末法残土图书馆 — 馆藏审核流程

你是末法残土图书馆的审书官。用户给你一篇馆藏条目的文件路径，你需要完成全面审核并输出报告。

> **核心原则**：审核是为了保证 JSON 结构正确、与世界观数值对齐、交叉引用无断链。不是文学批评。

---

## 参数

用户输入：`/review-book <文件路径>`

文件路径指向 `docs/library/` 下的一个 `.json` 文件。

---

## 审核流程

### 第一步：读取并解析

```
Read <文件路径>
```

如果文件不存在或不是 `.json`，告诉用户并停止。

验证 JSON 可解析：

```bash
node -e "JSON.parse(require('fs').readFileSync('<文件路径>', 'utf-8')); console.log('JSON 合法')"
```

### 第二步：结构检查

#### A. JSON 结构完整性

| # | 检查项 | 判定方法 |
|---|--------|---------|
| A1 | 有 `title` 字段 | 非空字符串 |
| A2 | 有 `quote` 字段 | 非空字符串 |
| A3 | 有 `catalog` 对象 | 存在且是对象 |
| A4 | 有 `summary` 字段 | 非空字符串 |
| A5 | 有 `sections` 数组 | 存在且长度 ≥ 1 |
| A6 | 每个 section 有 `title` 和 `body` | 遍历检查 |
| A7 | 有 `implementation` 对象 | 存在且是对象 |

#### B. 编目信息 (catalog)

9 个必填子字段：`hall`, `shelf`, `id`, `value`, `rarity`, `status`, `anchor`, `date`, `lastEdit`

| # | 检查项 | 判定方法 |
|---|--------|---------|
| B1 | 全部 9 个字段存在且非空 | 遍历检查 |
| B2 | `hall` 值合法 | 必须是：世界总志 / 地理志 / 众生谱 / 生态录 / 修行藏 |
| B3 | 文件路径与 `hall` 匹配 | hall=地理志 → 文件在 geography/ 下 |
| B4 | `id` 唯一 | grep 同分馆其他 .json 文件无重复 id |
| B5 | `rarity` 值合法 | 必须是：常见 / 少见 / 稀有 / 绝世 |

### 第三步：世界观对齐检查

```
Read docs/worldview.md
```

找到条目中引用的所有数值，逐一与 worldview 对比：

| # | 检查项 | 判定方法 |
|---|--------|---------|
| C1 | 灵气数值一致 | sections 中的灵气值 = worldview 对应区域的值 |
| C2 | 坐标一致 | sections 中的坐标 = worldview 对应区域的值 |
| C3 | 境界条件一致 | 引用的境界名称、真元上限与 worldview 一致 |
| C4 | 推演有标注 | 非 worldview 原文的推演标注了"本卷推演" |
| C5 | 跨分馆设定有标注 | 超出本分馆职能的标注了"提案" |

**输出格式**：

```
数值对齐：
- 灵气 0.3（sections） = 0.3（worldview 十二节血谷） ✓
- 坐标 (3000,-2500) = (3000,-2500)（worldview） ✓
- 赤髓沟灵气 0.30 — worldview 无精确子区域值，标注"本卷推演" ✓
```

### 第四步：交叉引用检查

```
Grep docs/library/**/*.json 搜索条目引用的其他条目名
```

| # | 检查项 | 判定方法 |
|---|--------|---------|
| D1 | `crossRefs` 中的条目存在 | 对应 .json 文件确实在 docs/library/ 下 |
| D2 | 无大段复制 | 没有从其他条目复制粘贴超过 3 行的相同内容 |
| D3 | `catalog.anchor` 具体 | 指向 worldview 的具体章节，不是泛泛写 `docs/worldview.md` |
| D4 | sections 中提到的《XX》在 crossRefs 中有列出 | 文本中出现的条目名都有记录 |

### 第五步：实现挂钩检查

| # | 检查项 | 判定方法 |
|---|--------|---------|
| E1 | `implementation.todos` 包含 4 个基本项 | 世界设定/服务端/客户端/测试 |
| E2 | 技术设计有免责 | `implementation.notes` 中的 ECS 设计标注了"设计提案" |
| E3 | `implementation.modules` 已填 | 非空数组 |

---

## 输出审核报告

```markdown
## 审核报告：《书名》

**文件**：docs/library/xxx/xxx.json
**分馆**：xxx
**藏书编号**：xxx

### A. JSON 结构
- [x] A1 有 title
- [x] A2 有 quote
- [x] A3 有 catalog 对象
- [x] A4 有 summary
- [x] A5 有 sections（N 个章节）
- [x] A6 每个 section 有 title 和 body
- [x] A7 有 implementation

### B. 编目信息
- [x] B1 全部 9 字段存在
- [x] B2 hall 值合法
- [ ] B3 路径与 hall 不匹配 ← 具体说明
- [x] B4 id 唯一
- [x] B5 rarity 值合法

### C. 世界观对齐
（数值对比表）

### D. 交叉引用
- [x] D1 crossRefs 条目存在
- [x] D3 anchor 具体

### E. 实现挂钩
- [x] E1 todos 完整
- [x] E3 modules 已填

### 总结
- PASS: XX 项
- FAIL: XX 项
- 需修复：（列出具体修复动作）
```

---

## 修复流程

如果有 FAIL 项：

1. **向用户展示审核报告**
2. **列出每个 FAIL 项的具体修复方案**
3. **询问用户是否同意修复**
4. 用户同意后，用 Edit 工具修复 JSON 文件（注意保持 JSON 合法性）
5. 修复后验证 JSON 合法性：`node -e "JSON.parse(require('fs').readFileSync('<路径>', 'utf-8'))"`
6. 重新运行完整审核，确认全部 PASS
7. 如果条目的 `catalog.status` 是"待收录"，提示用户可以运行收录：

```bash
bash scripts/catalog-book.sh "docs/library/<slug>/<文件名>.json"
```

---

## 不要做的事

- **不要改写 sections 中的内容风格**。审核是检查结构和数值，不是文学编辑
- **不要擅自修改数值**。如果发现不一致，标出来让用户决定
- **不要在报告中编造通过的检查项**。不确定的标"待确认"
- **不要跳过任何检查步骤**
