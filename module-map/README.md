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

## webui 功能

- 顶栏：实时搜索（`/` 聚焦，`Esc` 清空，匹配模块/组件/文件/符号）+ 层 tab 过滤
- 模块卡：点击展开组件拆解、跨层契约、红旗；按 fileCount 降序
- **⚑ 缺口/红旗** tab：聚合全部 warn/critical，按等级排序 —— 检阅清单
- **跨层 Features** tab：黑武士等跨层实体 dossier

## 生成流程

由 workflow 产出：sonnet 并发调查每个模块（读码→结构化拆解），opus 按 cluster 维护/审校/确认接线与缺口，最后注入本文件数据块。重跑见会话记录。
