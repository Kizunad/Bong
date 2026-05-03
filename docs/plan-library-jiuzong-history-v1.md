# Bong · plan-library-jiuzong-history-v1 · Active

> **状态**：⏳ active（2026-05-04 升级，user 拍板）。schema 模板已对齐 `library/templates/馆藏条目模板.json`（id 段 peoples-0010 至 0016）。§4 4 决策闭环（Q-LJ1/Q-LJ2/Q-LJ3 + 道教参考锚定）。

**已崩七宗各自立一篇宗门志入 library/peoples/**——为 plan-terrain-jiuzong-ruin-v1 提供 lore 锚点（壁文 narration / 残卷 lore / 守墓人 NPC 自述）。**写作密度参考真实道教**（龙虎山天师道 / 茅山 / 武当 / 全真）：每宗具体到祖师姓名 / 根本经典 / 标志技法 / 传承代际数。

**世界观锚点**：
- `worldview.md §一 末法时代`（"曾经的大能们早已飞升或陨落"）
- `worldview.md §三 六境界`（九宗大斗发生在远古，正典提及但未细节）
- `worldview.md §五 战斗流派`（七宗特征流派与现代 7 流派 plan 的对应）

**library 既有锚点**（**不重复**，本 plan 只补缺）：
- `library/world/末法纪略.json` —— 已有"九宗大斗 → 仅余其二"叙事概要
- `library/peoples/宗门残息.json` —— 已有青云外门 + 灵泉丹宗细节（**存活两宗**）
- `library/ecology/灵气零和记.json` —— 已含"九宗鼎盛万人同坐灵脉枯速"推演

**本 plan 缺口**：已亡七宗（血溪 / 幽暗 / 北陵 / 南渊 / 赤霞 / 玄水 / 太初）**各自**的历史 / 流派 / 标志性事件 / 灭宗经过——现存 library 仅概要提名，无细节。

**交叉引用**：
- `plan-terrain-jiuzong-ruin-v1`（active）— 本 lore 是其 §3 origin 表 + §9 Q-J1 强绑定（残卷 = 该宗 plan 功法）的 lore 背书；P3 壁文 narration 直接引本 plan 7 篇 json 中的 episode body
- `plan-baomai-v1` ✅ / `plan-zhenfa-v1` ✅ / `plan-dugu-v1` / `plan-anqi-v1` / `plan-zhenmai-v1` / `plan-tuike-v1` / `plan-multi-style-v1` — 各 plan vN+1 接收对应宗门残卷接口（详 jiuzong P3）

---

## 接入面 Checklist

- **进料**：worldview §一/§三/§五 + 既有 library 三篇（末法纪略 / 宗门残息 / 灵气零和记）锚点
- **出料**：7 篇 json 文件入 `docs/library/peoples/<宗名>志.json`（同 `宗门残息.json` schema）
- **共享类型**：复用 library json schema（quote / synopsis / chapters / essence）
- **跨仓库契约**：
  - 不动 server / agent / client 代码——纯 lore 写作 plan
  - 但触发 plan-terrain-jiuzong-ruin-v1 P3 narration template 的 input
- **worldview 锚点**：§一 / §三 / §五（不动 worldview 本身，仅引用）

---

## §0 设计轴心

- [ ] **七宗各自有独特性**——不能"七宗都是修仙宗门，区别只是颜色"。每宗必须有：(a) 立宗根本（什么样的修法 / 哲学）、(b) 标志性事件（鼎盛时做了什么）、(c) 灭宗经过（怎么死的，与九宗大斗关系）、(d) 现代遗留（散修后代 / 残卷 / 废墟特征）
- [ ] **参考真实道教"具体感"**（user 2026-05-04）——写作时参考道教真实宗派密度：龙虎山天师道（张天师 → 正一符箓 → 代代单传）/ 茅山（三茅真君 → 上清符箓 → 茅山十三代）/ 武当（张三丰 → 太极内丹 → 七子）/ 全真（王重阳 → 全真心法 → 北七真）。每宗必须给到：**开宗祖师姓名 / 根本经典 1-2 部 / 标志性技法 / 传承代际**——不要"某宗祖师立宗于某山"的模糊带过
- [ ] **流派对齐**——每宗主修流派与现代 7 流派 plan 一一对应（Q-J1 强绑定）：血溪→baomai / 北陵→zhenfa / 南渊→dugu / 赤霞→anqi / 玄水→zhenmai / 太初→multi-style / 幽暗→tuike
- [ ] **太初宗 = 多流派变异源头**（user Q-LJ3 决策 2026-05-04）——太初宗"任督全能" 不是字面"什么都修"，而是历史上**过早分化**：弟子各自带部分修法散出，**变异演化**成现代多种散修流派的祖型。太初故地残卷可能含其他流派的"原始版本"（与现代版差异度最大），这是其独特感来源
- [ ] **lore 一致性**——七宗灭宗时间线必须自洽（worldview "末法纪二百年前后九宗已去其七"），不能 A 宗死 100 年前 B 宗死 300 年前。约束在"末法纪初到二百年间陆续灭"窗口
- [ ] **不渲染九宗大斗细节**——大斗本身留 worldview 模糊处理（worldview §一 仅言"九宗死伤过半灵脉大伤"），各宗志只写"我宗在大斗中如何"，不写"大斗全貌"
- [ ] **七篇七种遗老声音**（user Q-LJ2 决策 2026-05-04）——文风对齐既有 `peoples/宗门残息.json` 残缺记录感，但**每宗志用不同遗老视角**（散修后代 / 守墓人 / 流浪老儒 / 反目逃徒 ...），不统一一个"老夫"声音。七篇七声，丰富叙事；难度高但 lore 价值大
- [ ] **轻度交叉引用**（user Q-LJ1 决策 2026-05-04）——七宗志互相**轻度提及**（如血溪志提一句"赤霞援手"），但不写成连贯小说。交叉点 ≤2 句 / 篇，避免读单篇时被强迫读其他篇

---

## §1 阶段总览

| 阶段 | 内容 | 验收 |
|---|---|---|
| **P0** ⬜ | 7 宗 lore 框架表 + 时间线 + 流派对齐 review | 7 行表格通过 user review；时间线无矛盾 |
| **P1** ⬜ | 7 篇 json 全文写作（按 P0 框架） | 7 个 `library/peoples/<宗名>志.json` 文件入库；schema 校验通过 |
| **P2** ⬜ | `/review-book` 通过 + 交叉引用补全（与既有三篇 library + worldview 互引） | 全部 7 篇通过 review；index.md 更新 |
| **P3** ⬜ | 壁文 narration episode 抽取（每宗 3-5 段） | 每宗 3-5 短句可被 plan-terrain-jiuzong-ruin-v1 P3 narration template 引用 |

---

## §2 P0 七宗框架表（待 P0 实施细化）

参考道教真实宗派写作密度（龙虎山 / 茅山 / 武当 / 全真）——每宗必填**开宗祖师姓名 / 根本经典 / 标志技法 / 传承代际数 / 灭宗时间窗口**：

| origin_id | 宗名 | 立宗根本 | 开宗祖师 | 根本经典 | 标志技法 | 传承代际 | 灭宗时间 | 标志性事件 | 灭宗经过 | 现代遗存 | 主修流派 plan |
|---:|---|---|---|---|---|---|---|---|---|---|---|
| 1 | 血溪 | 体修 / 以血养身 | TODO | TODO | TODO | TODO | TODO | TODO | TODO | TODO | plan-baomai-v1 ✅ |
| 2 | 北陵 | 阵法 / 地师 | TODO | TODO | TODO | TODO | TODO | TODO | TODO | TODO | plan-zhenfa-v1 ✅ |
| 3 | 南渊 | 毒蛊 / 医道双修 | TODO | TODO | TODO | TODO | TODO | TODO | TODO | TODO | plan-dugu-v1 ⏳ |
| 4 | 赤霞 | 雷法 / 暗器 | TODO | TODO | TODO | TODO | TODO | TODO | TODO | TODO | plan-anqi-v1 ⏳ |
| 5 | 玄水 | 御剑 / 截脉 | TODO | TODO | TODO | TODO | TODO | TODO | TODO | TODO | plan-zhenmai-v1 ⏳ |
| 6 | 太初 | **过早分化的全能祖型** | TODO | TODO | TODO（多流派原始版本）| TODO | TODO | TODO（弟子变异散落）| TODO | TODO（多流派祖型残页）| plan-multi-style-v1 🆕 |
| 7 | 幽暗 | 暗器 / 隐遁 / 替尸 | TODO | TODO | TODO | TODO | TODO | TODO | TODO | TODO | plan-tuike-v1 ⏳ |

**P0 任务**：
1. 填满 TODO 列（每宗框架完整）
2. user 评审框架表（不写正文，先核框架）
3. 时间线与既有 library 三篇核对（末法纪略 / 宗门残息 / 灵气零和记）
4. 七宗祖师姓名互相不重名，传承代际数互不雷同
5. 太初宗"分化变异"具体写出：哪几个变异流派 → 现代散修哪个支派的祖型

---

## §3 P1 json schema 模板

**严格遵守 `docs/library/templates/馆藏条目模板.json`**（既有 9 篇 peoples 全跟此 schema）：

```json
{
  "title": "<宗名>志",
  "essence": "<≤80 字纯事实要点，给 agent 注入用>",
  "synopsis": "<100-300 字关键细节，给 agent 快速阅读>",
  "quote": "<卷首引语一句>",
  "catalog": {
    "hall": "众生谱",
    "shelf": "丙一·宗门残谱",
    "id": "peoples-00XX",
    "value": "<X 骨币>",
    "rarity": "稀有",
    "status": "待收录",
    "anchor": "末法遗老追忆 / docs/worldview.md §一",
    "date": "末法纪 XXX",
    "lastEdit": "2026-05-04"
  },
  "summary": "<1-3 段摘要 + 解决什么设定问题>",
  "sections": [
    { "title": "立宗", "body": "..." },
    { "title": "鼎盛", "body": "..." },
    { "title": "大斗中", "body": "..." },
    { "title": "余烬", "body": "..." },
    { "title": "现代遗存", "body": "..." }
  ],
  "fragments": [
    { "title": "残卷·壁文其一", "text": "...", "note": "<宗名> 故地 大殿核心壁文" },
    { "title": "残卷·壁文其二", "text": "...", "note": "<宗名> 故地 长老坐化处" },
    { "title": "残卷·壁文其三", "text": "...", "note": "<宗名> 故地 万人讲堂残基" }
  ],
  "crossRefs": ["《末法纪略》", "《宗门残息》", "《灵气零和记》", "worldview.md §一"],
  "implementation": {
    "modules": ["agent"],
    "files": [
      "agent/packages/tiandao/src/narration/zong_lore.ts"
    ],
    "todos": [
      { "done": false, "text": "lore 入库 + /review-book 通过" },
      { "done": false, "text": "壁文 narration template 引用 fragments[]" }
    ],
    "notes": "fragments 直接喂 plan-terrain-jiuzong-ruin-v1 P3 壁文 narration"
  }
}
```

> **关键约束**：
> - id 续号：既有 peoples-0001 至 0009 已用（北荒游记/野修志/骨语人/宗门残息/异变图谱/战斗流派源流/散修百态/遗言四则/地师手记），七宗志按顺序占 **peoples-0010 至 peoples-0016**
> - `fragments[]` 是 plan-terrain-jiuzong-ruin-v1 P3 壁文 narration 的直接 input —— 每宗 ≥3 段
> - `crossRefs` 必含既有三篇核心引用 + worldview 锚点

---

## §4 开放问题

- [x] **Q-LJ1 ✅**（user 2026-05-04 A）：**轻度交叉引用**——七宗志互相轻度提及（≤2 句 / 篇），不写成连贯小说。详 §0 第 7 条
- [x] **Q-LJ2 ✅**（user 2026-05-04 B）：**各宗不同遗老视角**——七篇七种声音（散修后代 / 守墓人 / 流浪老儒 / 反目逃徒等），不统一"老夫"。详 §0 第 6 条
- [x] **Q-LJ3 ✅**（user 2026-05-04 B+变异）：**太初宗 = 过早分化的全能祖型**——弟子各自带部分修法散出变异演化为现代多种散修流派祖型。残卷含其他流派"原始版本"。详 §0 第 4 条 + §2 第 6 行 + P0 任务第 5 条
- [x] **道教参考锚定**（user 2026-05-04）：写作密度参考龙虎山 / 茅山 / 武当 / 全真——具体到祖师姓名 / 根本经典 / 标志技法 / 传承代际，不要"某宗祖师立宗于某山"模糊带过。详 §0 第 2 条 + §2 框架表新增列
- [ ] 灭宗时间线精确到"末法纪 X 年"还是模糊"末法纪初 / 末法纪百年间"？—— 倾向**模糊**（古籍残缺感），P0 框架填表时拍板

---

## §5 进度日志

- **2026-05-04**：从 plan-terrain-jiuzong-ruin-v1 Q-J2 决策派生立项。current state：library 既有三篇（末法纪略 / 宗门残息 / 灵气零和记）已含九宗概要 + 仅存二宗细节，缺已亡七宗各自详 lore。本 plan 补此缺。
- **2026-05-04**：skeleton → active 升级（user 拍板）。schema 模板修正（对齐 `馆藏条目模板.json`，id 段 peoples-0010~0016）；§4 全部 4 决策闭环（Q-LJ1/Q-LJ2/Q-LJ3 + 道教参考锚定）：
  - 七宗志轻度交叉（≤2 句/篇）
  - 七篇七种遗老声音（散修后代 / 守墓人 / 流浪老儒 / 反目逃徒）
  - 太初宗 = 过早分化的全能祖型（弟子变异演化为多流派源头）
  - 写作密度参考龙虎山 / 茅山 / 武当 / 全真——祖师姓名 + 根本经典 + 标志技法 + 传承代际
  - §2 框架表加 6 列（开宗祖师 / 根本经典 / 标志技法 / 传承代际 / 灭宗时间 / 现代遗存）
- 下一步起 P0：填七宗框架表 TODO（不写正文，先核框架）+ user 评审。
