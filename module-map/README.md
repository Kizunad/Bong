# Bong 模块图谱 (Module Map)

server / client / agent 三层**全模块第一性原理拆解**的可浏览总览。项目从高速开发期转入"各模块检阅"期的活地图。

## 查看

```bash
bash scripts/runwebui.sh      # 或在 Claude Code 里 /runwebui
```

单文件自包含 HTML（`index.html`），`file://` 直接打开，无构建、无外部依赖、无需服务器。

## 维护：新增 / 修改模块

**只需编辑 `index.html` 里 `=== DATA:START ===` 与 `=== DATA:END ===` 之间的数据块**，渲染逻辑无需改动。

- 普通模块 → 往 `MODULES` 数组加一个对象
- 跨层实体/系统（如黑武士、渡劫、天道 loop）→ 往 `FEATURES` 数组加一个对象

### MODULES 条目 schema

```js
{
  id:"server/npc",            // 唯一 id：<layer>/<name>
  layer:"server",             // server | client | agent
  name:"npc",                 // 目录/包名
  path:"server/src/npc/",     // 仓库相对根路径
  title:"NPC AI 与生命周期",   // 中文标题
  summary:"一两句话职责。",
  fileCount:72,               // 源文件数（卡片排序用）
  tags:["big-brain","ecs"],
  components:[ {              // 第一性原理拆出的组件（黑武士→Brain/Animation 即此粒度）
    name:"Brain (big-brain Utility AI)",
    role:"职责一句话",
    files:["server/src/npc/brain/mod.rs"],   // 仓库相对路径
    keySymbols:["scorers_combat::ThreatScorer"],
    deps:["big-brain 0.21"],
    wiring:{ upstream:["谁喂它"], downstream:["它喂谁"] },
    gaps:[ {severity:"warn", note:"…"} ]      // 组件级红旗，可空
  } ],
  crossLayer:[ { with:"client/npc", contract:"NpcMetadata payload", via:"CustomPayload + schema" } ],
  gaps:[ {severity:"info|warn|critical", note:"模块级缺口"} ]
}
```

### FEATURES 条目 schema

跨层 marquee 实体/系统。结构同上，差异：用 `aspects`（而非 `components`）+ `spans`（涉及的模块 id 列表），无 `layer`/`fileCount`。

### gap severity

- `critical` — 可达的真 bug / 守恒漏洞 / 孤岛接线缺失（红，进 skeleton plan）
- `warn` — 不完善 / 设计抉择待定 / 半接线
- `info` — 备注、占位（不计入缺口数，不进缺口总览）

## webui 功能（多视图作品版）

hash 路由的单文件 SPA，**每一项都有独立页面**，动画服务理解、不花哨，尊重 `prefers-reduced-motion`（降级为静态标注图，信息不丢）：

- **概览 `#/overview`** —— hero + 规模统计（count-up）+ **三层数据流架构图**（agent/server/client + worldgen/library-web，粒子按真实 IPC 方向流动）+ 跨层 feature 聚光 + 模块目录入口。读者 10 秒抓住"这是什么 + 三层怎么协作 + 规模 + 缺口概况"。
- **层视图 `#/layer/<server|client|agent>`** —— 该层全模块网格 + 搜索 + tag 筛选（top-16 + 「更多标签」折叠）+ 按缺口严重度/名称/组件数排序。
- **模块页 `#/module/<layer/name>`** —— 每模块独立页：hero（层徽章/path/文件数/组件数/缺口数 count-up/tags/summary）+ **★数据驱动 wiring 流图**（上游依赖→本模块组件→下游产出，粒子沿真实连线流动；按层母题异化：server=ECS Tick 流 / client=Payload→State→Render / agent=感知→推演→仲裁循环；hover/focus 高亮该组件真实接线，「重播」可重看）+ 全组件卡（role/files/keySymbols/deps/wiring/gaps）+ 跨层契约 + 缺口 + 上/下模块导航。
- **feature dossier `#/feature/feature%2F<id>`** —— 每跨层实体独立页：层覆盖徽章 + spans + 切面（aspects）卡。
- **缺口总览 `#/gaps`** —— 全库 warn/critical 聚合，三色 + 层筛选，点击跳来源。
- **全局搜索** —— `Ctrl/Cmd+K` 或 `/` 唤出命令面板，跨模块/组件/特性子串检索，键盘上下 + Enter 跳转，Esc / 路由切换自动关。

## 生成流程

由 workflow 产出。**数据**：sonnet 并发调查每模块（读码→结构化拆解），opus 按 cluster 维护/审校/确认接线与缺口，注入数据块。**引擎（多视图作品）**：3 sonnet 探索设计方向 → Playwright 真渲染选型 → sonnet 建全引擎 → Playwright 逐路由真机 QA + sonnet 打磨。重跑见会话记录。
