# plan-surface-stash-search-hud-label-gap-v1（骨架）

> **骨架（草案）**。一句话主题：`ContainerKind::SurfaceStash` 已在 server / schema / proto / bridge 全链路落地，但 client `TsyContainerView.kindLabelZh()` 仍未补 `"surface_stash"` 分支，导致地表散修遗缴的 world-surface 搜刮 HUD 与完成提示恒回退通用“容器”。影响是：**玩家在 spawn 周边打开散修遗缴时，看不到“散修遗缴”这一专属反馈，只会看到与 TSY 泛容器相同的搜刮文案，削弱新手资源点识别、每日限额理解与 loot 来源心智**。

> 立项动机：当前分支已把 `surface_stash` 作为正式可搜索容器种类接入 server runtime 与桥接链，且活跃 plan 已明确记过“client 标签缺失”但没有单独跟踪 client 修复。本轮 bughunt 聚焦 world HUD / world surface 路径，确认该缺口仍实存于 `HEAD`，值得补一份独立 skeleton 收口复现、根因、影响和修复建议。

## 阶段总览

| 阶段 | 主题 | 路由 | 状态 |
|------|------|------|------|
| P0 | `surface_stash` 搜刮 HUD / 完成提示标签缺口 | fix_pr | ⬜ |

## P0 - `surface_stash` 搜刮 HUD / 完成提示标签缺口

- **复现路径**：
  1. server 侧 `ContainerKind::SurfaceStash` 已是正式容器种类，定义为“地表可见、无需钥匙、搜索 60 ticks”的散修遗缴，字符串线名就是 `"surface_stash"`（`server/src/world/tsy_container.rs:38-39,68-77`）。
  2. 搜刮完成逻辑对 `SurfaceStash` 走专门分支，完成时还会记录每玩家 24h 限额，说明这不是死枚举，而是玩家正常可达主链（`server/src/world/tsy_container_search.rs:591-599`）。
  3. client `ContainerInteractionHandler` 会把 `container_state.kind` 原样写入 `TsyContainerView.kind()`，后续 `search_started/search_progress/search_completed` 全都经 `kindLabel(entityId)` 读取 `view.kindLabelZh()` 作为 HUD 文案来源（`client/src/main/java/com/bong/client/network/ContainerInteractionHandler.java:39-50,53-59,67-70,94-109`）。
  4. 但 `TsyContainerView.kindLabelZh()` 只覆盖 `dry_corpse / skeleton / storage_pouch / stone_casket / relic_core` 五种，**没有** `"surface_stash"`，因此命中 `default -> "容器"`（`client/src/main/java/com/bong/client/tsy/TsyContainerView.java:25-33`）。
  5. `SearchProgressHudPlanner` 会把这个标签直接渲染到“正在搜刮：%s”与“搜刮完成：%s”文案里，所以玩家实际看到的是通用“容器”而非“散修遗缴”（`client/src/main/java/com/bong/client/hud/SearchProgressHudPlanner.java:49-71`）。

- **根因链路**：
  - `surface_stash` 作为新容器 kind 已进入 server / schema / proto / bridge 契约，但 client 侧**人类可读标签映射表**停留在旧五种容器集合。
  - `ContainerInteractionHandler` 没有独立 label 表，而是把所有搜索 HUD 文案都委托给 `TsyContainerView.kindLabelZh()`。
  - 因为 `kindLabelZh()` 缺 `"surface_stash"` case，所有 `surface_stash` 搜刮相关 HUD 文案都会统一退化到默认值“容器”。
  - 测试面也印证了这个缺口：`ContainerInteractionHandlerTest` 目前只锁 `storage_pouch` / `stone_casket` 等旧种类，没有任何 `surface_stash` label pin，用例缺口使该回归长期存活（`client/src/test/java/com/bong/client/network/ContainerInteractionHandlerTest.java:55-75,144-170`）。

- **为什么这是 bug，不是刻意设计**：
  - 活跃 plan 已明确把 `ContainerKind::SurfaceStash` / `ContainerKindV1::SurfaceStash` 认定为既有正式契约，并且已经在 pre-P0 复核里记录“`kindLabelZh()` 缺 `"surface_stash"` case，玩家标题栏显示通用‘容器’而非‘散修遗缴’专属标签”（`docs/plan-surface-stash-runtime-scatter-gap-v1.md:24-27`）。
  - 这说明仓库自己已经承认 client 标签缺失是未补完项，而不是故意把 `surface_stash` 与泛容器做成无差别显示。

- **这个 bug 对实际游玩体验的影响**：
  - spawn 周边的散修遗缴是新手阶段的地表资源点，玩家搜刮时 HUD 却只显示“容器”，无法从即时反馈区分“这是入门 surface stash”还是“普通 TSY 容器”。
  - `surface_stash` 还带每玩家 24h 限额与专属 loot pool 语义；标签丢失会让玩家更难把“今天这类点为何不给了”“这批灵水/碎片来自哪类容器”与散修遗缴规则建立稳定心智。
  - 这不是纯静态命名洁癖，而是 world-surface 教学反馈缺口：系统已经在玩法上把它当独立类别，HUD 却没有把类别差异告诉玩家。

- **修复建议**：
  - 最小修法：在 `client/src/main/java/com/bong/client/tsy/TsyContainerView.java` 的 `kindLabelZh()` 补 `"surface_stash" -> "散修遗缴"`。
  - 同步补 pin 测试：`ContainerInteractionHandlerTest` 增一条 `surface_stash` 经过真实 proto wire / `search_started` 后，`kind()` 应为 `"surface_stash"`、`kindLabelZh()` 与 `SearchHudStateStore.snapshot().containerKindZh()` 应为“散修遗缴”。
  - 若团队后续继续扩容 `ContainerKind`，建议把 label 映射集中到单点常量表，并补“server enum 全变体都有 client label”测试，避免下次再漏新 kind。

- **验收抓手**：
  1. proto wire 过桥后的 `ContainerStateProto{ kind=CONTAINER_KIND_SURFACE_STASH }` 必须在 `TsyContainerView.kind()` 中保留 `"surface_stash"`。
  2. `search_started/search_progress/search_completed` 三条路径的 HUD 文案都必须显示“散修遗缴”，而不是“容器”。
  3. 旧五种容器标签不能回归。
  4. 若以后新增 `ContainerKind`，测试应能在漏标签时直接 RED。

## 反方裁决摘要

- **退化说明**：当前 Codex 会话没有可用的 subagent / delegate tool，本轮无法再开独立反方子代理；因此这里采用“主代理双轮反方论证 + 逐条代码复核”的退化流程，并把反方论点与驳回理由显式记录。

1. **Round 1 反方论点**：“这只是 UI 文案瑕疵，不影响搜刮计时、loot pool、respawn，算不上值得单开 skeleton 的 bug。”
   - **驳回理由**：`surface_stash` 不是普通同类皮肤，而是带独立资源定位、搜索时长与 24h 限额的正式玩法类别（`server/src/world/tsy_container.rs:38-39,50,59,76`；`server/src/world/tsy_container_search.rs:591-599`）。HUD 恰恰承担“把类别告诉玩家”的职责；把独立类别退化成泛称，会直接伤害新手资源点识别和规则理解，属于实际可感知缺陷，不是纯代码洁癖。

2. **Round 2 反方论点**：“活跃 plan 已经在备注里提过这个已知遗留，没必要再单独立 bug skeleton。”
   - **驳回理由**：现有活跃 plan 的目标是 server runtime 生产补丁，且文档明确写了“非本 plan 范围，留给未来 client 侧 PR”（`docs/plan-surface-stash-runtime-scatter-gap-v1.md:27`）。这意味着问题**被记录但未被跟踪解决**；在当前 bughunt 任务要求下，把它抽成独立 skeleton 正是为了给未来 fix PR 提供复现、根因、影响和验收入口，而不是继续埋在旁注里。

## 开放问题

1. 终稿文案是沿用“散修遗缴”，还是改成更短的 HUD 词（例如“遗缴”）以适配窄屏；需要在 fix PR 里统一 `kindLabelZh()` 与相关 UI 词表。
2. 是否顺手给 `TsyContainerView.kindLabelZh()` 的所有 kind 做一个枚举覆盖测试，避免 `surface_stash` 之后再出现第二个漏接的新容器种类。

## 审计来源

bughunt 定向轮（范围：world HUD / world surface；显式避开 zone_info stale、dugu v2 hud disconnect bleed、tide sky omen client bridge、locust warning duration drift）。候选经主代理人工搜索、world-surface 调用链复核、两轮退化版反方裁决后保留。当前结论是 **report-only**：只提交 skeleton，不改源码。
