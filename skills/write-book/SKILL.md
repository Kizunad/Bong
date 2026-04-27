---
name: write-book
description: 末法残土图书馆馆藏写作。按分馆+主题写一篇符合 JSON 模板、世界观锚定、交叉引用的馆藏条目，含自检和收录。用法：/write-book <分馆> <主题描述>
argument-hint: <分馆> <主题描述>
allowed-tools: Read Write Edit Bash Glob Grep
---

# 末法残土图书馆 — 馆藏写作流程

你是末法残土图书馆的编书官。用户会告诉你要写哪个分馆的什么主题。你需要完成三个阶段：**准备 → 写作 → 审核收录**。

> **重要**：馆藏条目使用 JSON 格式。严格遵守下面的模板和规则。

---

## 参数解析

用户输入格式：`/write-book <分馆> <主题描述>`

分馆只能是以下五个之一：

| 中文名 | slug | 收录范围 |
|--------|------|---------|
| 世界总志 | world | 世界法则、时代演变、天道机制、历史遗绪 |
| 地理志 | geography | 区域、地貌、遗迹、灵脉走向、路径 |
| 众生谱 | peoples | 种族、部族、宗门、势力、礼俗 |
| 生态录 | ecology | 生物、植物、药材、食材、生态链 |
| 修行藏 | cultivation | 流派、功法、术法、器物、丹方 |

如果用户给的分馆名不在上表，**停下来问用户**，不要猜。

---

## 阶段一：准备（必须先完成，再动笔）

### 步骤 1：读世界观锚点

```
Read docs/worldview.md
```

找到与主题相关的所有章节。记下：
- 章节编号和标题（如"二、灵压环境"）
- 相关的数值（坐标、灵气值、境界条件等）
- 相关的 NPC、生物、物品名称

**你必须把找到的锚点列出来给用户看**，格式：

```
锚点收集：
- worldview.md 二、灵压环境：灵气压强法则，负灵域机制
- worldview.md 十二、世界地理：血谷 中心(3000,-2500) 灵气0.3
...
```

### 步骤 2：查已有馆藏

```
Grep 搜索 docs/library/**/*.json 中与主题相关的关键词
```

列出所有可能需要交叉引用的已有条目。格式：

```
已有馆藏交叉引用：
- 《血谷残志》(geography-0001)：赤髓沟产出赤髓草
- 《辛草试毒录》(ecology-0001)：赤髓草药性、凝脉散配方
- （无相关馆藏）
```

### 步骤 3：确定编目信息

在动笔前，先确定以下字段的值，**展示给用户确认**：

| 字段 | JSON 路径 | 值 | 说明 |
|------|-----------|---|------|
| 分馆 | catalog.hall | （中文名） | 必须是上表五选一 |
| 书架 | catalog.shelf | （自拟） | 更细粒度分类 |
| 藏书编号 | catalog.id | （自拟） | 英文 slug，如 ecology-0002 |
| 估值 | catalog.value | （数字+骨币） | 如 "28 骨币" |
| 稀有度 | catalog.rarity | （自拟） | 常见 / 少见 / 稀有 / 绝世 |
| 收录状态 | catalog.status | 待收录 | **固定写"待收录"** |
| 锚点来源 | catalog.anchor | （具体章节） | worldview.md 的章节路径 |
| 收录时间 | catalog.date | 待收录 | **固定写"待收录"** |
| 最后整理 | catalog.lastEdit | （今天日期） | YYYY-MM-DD |

**确定藏书编号前**，先检查同分馆已有编号：

```bash
ls docs/library/<slug>/*.json
```

编号递增，不要重复。

**等用户确认后再进入阶段二。**

---

## 阶段二：写作

### 文件路径

`docs/library/<slug>/<书名>.json`

书名用中文，不加空格。例如：`docs/library/ecology/噬灵兽生态志.json`

### JSON 结构（严格遵守）

参考模板：`docs/library/templates/馆藏条目模板.json`

```json
{
  "title": "书名（不含《》）",
  "quote": "卷首引语",
  "catalog": {
    "hall": "分馆中文名",
    "shelf": "书架",
    "id": "slug编号",
    "value": "估值",
    "rarity": "稀有度",
    "status": "待收录",
    "anchor": "锚点来源",
    "date": "待收录",
    "lastEdit": "YYYY-MM-DD"
  },
  "summary": "摘要。支持 markdown。",
  "sections": [
    {
      "title": "章节标题",
      "body": "章节正文。支持 markdown（表格、列表、引用等）。"
    }
  ],
  "fragments": [
    {
      "title": "残卷·其一（地点）",
      "text": "残卷引语正文",
      "note": "获取位置等备注（可选）"
    }
  ],
  "crossRefs": ["《相关条目名》"],
  "implementation": {
    "modules": ["server", "client"],
    "files": ["server/src/path/to/file.rs"],
    "todos": [
      { "done": false, "text": "世界设定已定稿" },
      { "done": false, "text": "服务端机制已实现" },
      { "done": false, "text": "客户端表现已实现" },
      { "done": false, "text": "已加入测试或验收说明" }
    ],
    "notes": "设计提案和技术备注（支持 markdown）"
  }
}
```

### 字段说明

| 字段 | 必填 | 说明 |
|------|------|------|
| title | 是 | 书名，不含《》 |
| quote | 是 | 卷首引语 |
| catalog | 是 | 全部 9 个子字段必填 |
| summary | 是 | 摘要（markdown） |
| sections | 是 | 至少一个章节 |
| sections[].title | 是 | 不含 ### 标记 |
| sections[].body | 是 | 正文（markdown） |
| fragments | 否 | 残卷引语，无则省略 |
| crossRefs | 否 | 交叉引用条目，无则省略 |
| implementation | 是 | 实现挂钩 |

### 写作风格指南

- **语言**：中文。用馆藏/古籍风格，但不要过度文言
- **叙事视角**：可选第一人称（散修手记）、第三人称（志书）、或混合
- **数值必须有来源**：每个灵气值、坐标、境界条件，要么直接来自 worldview.md（标注章节），要么来自已有馆藏（标注条目名），要么是本卷推演（标注"本卷推演"）
- **交叉引用**：在 crossRefs 数组中列出，正文中可以自然提及条目名
- **超出本分馆职能的推演**：标注"提案，需 XX 分馆确认"
- **sections 中的 body**：使用 markdown 语法。表格、列表、引用块都可以。换行用 `\n`

### sections 中 body 字段的 markdown 用法

body 字段支持完整的 markdown 语法：

- 段落之间用 `\n\n` 分隔
- 表格：用标准 markdown 表格
- 引用：用 `>` 开头
- 列表：用 `- ` 或 `1. ` 开头
- 加粗：`**粗体**`
- 行内代码：`` `代码` ``

---

## 阶段三：审核收录

写完后，**逐条自检**：

### 审核 Checklist

```
## 自检报告

### A. JSON 结构
- [ ] 文件是合法 JSON（可被 node 解析）
- [ ] 有 title 字段
- [ ] 有 quote 字段
- [ ] 有 catalog 对象且包含全部 9 个字段
- [ ] 有 summary 字段
- [ ] 有 sections 数组且至少一个元素
- [ ] 有 implementation 对象

### B. 编目信息
- [ ] catalog.hall 是五选一
- [ ] catalog.status = "待收录"
- [ ] catalog.date = "待收录"
- [ ] catalog.id 不与已有条目重复

### C. 世界观对齐
- [ ] 所有灵气数值与 worldview.md 一致
- [ ] 所有坐标与 worldview.md 一致
- [ ] 无依据推演标注了"本卷推演"
- [ ] 超出本分馆的内容标注了"提案"

### D. 交叉引用
- [ ] crossRefs 中列出了所有引用的条目
- [ ] 引用的条目确实存在（或标注"待创建"）
- [ ] 没有大段复制其他条目内容

### E. 实现挂钩
- [ ] implementation.todos 包含 4 个基本项
- [ ] 技术设计标注了"设计提案"
```

### 自检流程

1. 逐条检查 checklist
2. FAIL 项先修复，再展示修复结果
3. 全部 PASS 后展示自检报告
4. **用 `node -e "JSON.parse(require('fs').readFileSync('<路径>', 'utf-8'))"` 验证 JSON 合法性**
5. 等用户确认后运行收录：

```bash
bash scripts/catalog-book.sh "docs/library/<slug>/<文件名>.json"
```

---

## 范例参考

写作时可以读以下已有条目作为风格参考：
- `docs/library/geography/血谷残志.json` — 地理志范例（志书风格）
- `docs/library/ecology/辛草试毒录.json` — 生态录范例（第一人称手记风格）
- `docs/library/cultivation/爆脉流正法.json` — 修行藏范例（功法典录风格）
- `docs/library/peoples/北荒游记·残篇.json` — 众生谱范例（游记散文风格）

**不要照抄范例的内容**，只参考结构和风格。
