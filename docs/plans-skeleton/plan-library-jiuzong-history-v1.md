# Bong · plan-library-jiuzong-history-v1 · 骨架

**已崩七宗各自立一篇宗门志入 library/peoples/**——为 plan-terrain-jiuzong-ruin-v1 提供 lore 锚点（壁文 narration / 残卷 lore / 守墓人 NPC 自述）。

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
- [ ] **流派对齐**——每宗主修流派与现代 7 流派 plan 一一对应（Q-J1 强绑定）：血溪→baomai / 北陵→zhenfa / 南渊→dugu / 赤霞→anqi / 玄水→zhenmai / 太初→multi-style / 幽暗→tuike
- [ ] **lore 一致性**——七宗灭宗时间线必须自洽（worldview "末法纪二百年前后九宗已去其七"），不能 A 宗死 100 年前 B 宗死 300 年前。约束在"末法纪初到二百年间陆续灭"窗口
- [ ] **不渲染九宗大斗细节**——大斗本身留 worldview 模糊处理（worldview §一 仅言"九宗死伤过半灵脉大伤"），各宗志只写"我宗在大斗中如何"，不写"大斗全貌"
- [ ] **古意 / 残卷感**——文风对齐既有 `peoples/宗门残息.json`：用"老夫" / "余" 等第一人称遗老视角，残缺记录感

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

| origin_id | 宗名 | 立宗根本 | 标志性事件 | 灭宗经过 | 主修流派 plan |
|---:|---|---|---|---|---|
| 1 | 血溪 | 体修 / 以血养身 | TODO | TODO | plan-baomai-v1 ✅ |
| 2 | 北陵 | 阵法 / 地师 | TODO | TODO | plan-zhenfa-v1 ✅ |
| 3 | 南渊 | 毒蛊 / 医道双修 | TODO | TODO | plan-dugu-v1 ⏳ |
| 4 | 赤霞 | 雷法 / 暗器 | TODO | TODO | plan-anqi-v1 ⏳ |
| 5 | 玄水 | 御剑 / 截脉 | TODO | TODO | plan-zhenmai-v1 ⏳ |
| 6 | 太初 | 任督全能 | TODO | TODO | plan-multi-style-v1 🆕 |
| 7 | 幽暗 | 暗器 / 隐遁 / 替尸 | TODO | TODO | plan-tuike-v1 ⏳ |

**P0 任务**：填满 TODO 列；user 评审；时间线与既有 library 三篇核对（末法纪略 / 宗门残息 / 灵气零和记）。

---

## §3 P1 json schema 模板

参考 `library/peoples/宗门残息.json` 现有 schema：

```json
{
  "title": "<宗名>志",
  "quote": "<一句意境引言>",
  "synopsis": "<200 字内宗门概括>",
  "chapters": [
    { "title": "立宗", "body": "..." },
    { "title": "鼎盛", "body": "..." },
    { "title": "大斗", "body": "..." },
    { "title": "余烬", "body": "..." }
  ],
  "essence": "<100 字内最浓缩 takeaway>"
}
```

---

## §4 开放问题

- [ ] 七宗志是否要相互交叉引用？（如血溪志提到"那场大斗中赤霞宗弟子曾来援"）—— 倾向**轻度交叉**，避免每篇独立成"小说"
- [ ] 文风第一人称视角统一（老夫 / 余）还是各宗用不同遗老视角？—— 倾向**各宗不同遗老**，丰富叙事
- [ ] 灭宗时间线精确到"末法纪 X 年"还是模糊"末法纪初 / 末法纪百年间"？—— 倾向**模糊**（古籍残缺感）
- [ ] 是否每宗志都加一段"现代遗存"（散修后代 / 残卷流向 / 守墓人）—— 倾向**是**（直接 lore 锚 plan-terrain-jiuzong-ruin-v1 守墓人 NPC）
- [ ] 太初宗（任督全能）的 lore 写作难度最大——其他六宗有明显修法特色，太初是"什么都修"，怎么写出独特感？P0 user 重点 review 此条

---

## §5 进度日志

- **2026-05-04**：从 plan-terrain-jiuzong-ruin-v1 Q-J2 决策派生立项。current state：library 既有三篇（末法纪略 / 宗门残息 / 灵气零和记）已含九宗概要 + 仅存二宗细节，缺已亡七宗各自详 lore。本 plan 补此缺。
