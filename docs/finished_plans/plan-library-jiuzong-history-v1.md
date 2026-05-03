
---

## Finish Evidence

### 落地清单

| P | 描述 | 落地文件 |
|---|---|---|
| P0 | 七宗 lore 框架表 | 内化于 P1 七篇 JSON，含祖师姓名/根本经典/标志技法/传承代际/灭宗时间/灭宗经过/现代遗存 |
| P1 | 七篇 JSON 全文写作 | `docs/library/peoples/{血溪,北陵,南渊,赤霞,玄水,太初,幽暗}志.json` |
| P2 | 交叉引用 + index.md | `docs/library/peoples/index.md`（补入7条新记录）；各篇 crossRefs 互引完整 |
| P3 | 壁文 narration episode | 每宗 3-5 段 fragments[]，总计 29 段壁文（血溪4+北陵4+南渊3+赤霞4+玄水4+太初5+幽暗5） |

### 关键 commit

| commit | 日期 | 消息 |
|---|---|---|
| `e843ae6` | 2026-05-04 | feat(library): 七宗志 P1 入库——血溪/北陵/南渊/赤霞/玄水/太初/幽暗 |
| `301d698` | 2026-05-04 | docs(library): 众生谱 index.md 补入七宗志条目（peoples-0010~0016） |

### 测试结果

纯 lore 写作 plan，无 server/agent/client 代码改动。测试项：
- JSON schema 校验：全部 7 篇通过 `json.load()` 验证
- 模板对齐：每篇含 title/essence/synopsis/quote/catalog/sections(5节)/fragments(3-5段)/crossRefs/implementation
- ID 连续：peoples-0010 至 peoples-0016

### 跨仓库核验

纯文档 plan，不涉及 server/agent/client 代码。fragments[] 供 plan-terrain-jiuzong-ruin-v1 P3 壁文 narration template 消费。

### 七宗框架摘要

| 宗名 | 祖师 | 根本经典 | 技法 | 代际 | 灭宗时间 | 流派对齐 |
|---|---|---|---|---|---|---|
| 血溪 | 赤练子 | 《血神经》《锻骨换髓录》 | 血炼九转 | 7代 | 末法纪78年 | baomai |
| 北陵 | 陵光真人 | 《地脉图说》《六合万象阵解》 | 葬龙诀 | 11代 | 末法纪156年 | zhenfa |
| 南渊 | 孙百草 | 《万蛊谱》《百毒医典》 | 本命蛊 | 9代 | 末法纪112年 | dugu |
| 赤霞 | 雷震子 | 《引雷真诀》《七十二器谱》 | 掌心雷 | 6代 | 末法纪89年 | anqi |
| 玄水 | 陆沉渊 | 《玄水剑经》《截脉十三式》 | 截脉剑 | 8代 | 末法纪134年 | zhenmai |
| 太初 | 太初道人 | 《太初原始经》《万法归宗论》 | 原始真元 | 3代 | 末法纪41年 | multi-style |
| 幽暗 | 影老人 | 《影遁经》《替尸九法》 | 蜕壳术 | 5代 | 末法纪178年 | tuike |

### 七种遗老声音

| 宗名 | 声音 | 视角特征 |
|---|---|---|
| 血溪 | 散修后代 | 祖上捧血盘童子，传承不过百句旧事 |
| 北陵 | 守墓人 | 守陵四十载，代代标记半活性阵点 |
| 南渊 | 流浪老儒 | 游历六域九十载，考据蛊医源流 |
| 赤霞 | 反目逃徒 | 叛出四十三年，"我需要你的雷" |
| 玄水 | 末代少年杂役 | 14岁目睹灭宗，自学阵法欲补剑修之短——"今朝为我少年郎，敢问天地试锋芒" |
| 太初 | 拾荒者 | 太初原灰草叶纹拼凑十九年 |
| 幽暗 | 追迹者 | 追踪巳蛇三年，欠命还命 |

### 遗留 / 后续

- `/review-book` 审核（本 plan P2 列出，需人工或专门 agent 执行）
- plan-terrain-jiuzong-ruin-v1 P3 消费 fragments[] 壁文 narration
- library index.md 可能需 `scripts/rebuild-library-index.sh` 重建以保持排序一致（当前手动追加）

