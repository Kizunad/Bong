# plan-bughunt-alchemy-screen-fill-overflow-v1（骨架）

> **骨架（草案）**。一句话主题：`AlchemyScreen` 三处把 `Sizing.fill(100)` spacer 放在 horizontalFlow 兄弟节点**中间**——owo 0.11.2 的 `fill(100)` 是"父容器整宽"而非"剩余空间"，整宽 spacer 把其后的兄弟节点顶出固定宽度面板边界：炼丹等级/火候容差标签、"I 起炉 · F 注真元 · T 结算"操作提示、温度当前数值、真元注入数值**四组 gameplay 关键信息全部渲染在可视区域外**。

> 立项动机：Client-B 分区 bughunt（owo UI/HUD/Screen + mixin + PlayerAnimator）全 Screen fill(100) 扫描发现。**去重说明**：同型缺陷曾在手搓台修过（`CraftScreen.buildHeader` 注释"fill spacer 必须排在最后：owo fill(100) 占整宽，放在中间会把副标题顶出右边界"、`CraftActionBar.java` 同注释），bughunt r2 #663 的 UI 修复未覆盖 AlchemyScreen；origin/main 现有 alchemy 系 skeleton（furnace-persistence / furnace-scope-gate / recipe-fragment-handoff / start-intervention-agent-drop / takeback-full-inventory-loss）全部是逻辑缺陷，无一涉及本布局问题（`git grep alchemySkillLabel|buildTempRow|buildQiRow origin/main -- docs/` 零命中）。

## Bug 摘要

`AlchemyScreen` 的 `buildHeader()` / `buildTempRow()` / `buildQiRow()` 均采用「label → `fill(100)` spacer → 后续 label」的排布。owo 0.11.2 源码（sources jar `io/wispforest/owo/ui/core/Sizing.java:34`）：`FILL -> Math.round((value/100f) * space)`；`FlowLayout` 布局算法（`io/wispforest/owo/ui/container/FlowLayout.java:183-185` 等）对**每个 child 用同一份完整 childSpace inflate** 后顺序 mount——fill(100) 子节点占满父容器内容宽度，其后兄弟被 mount 到父容器右边界之外。

三个受害容器均为固定宽度（面板 `PANEL_W=600`、炉体列 `MID_W=220`），FlowLayout 不换行，被顶出的 label 永不可见。

## 对实际游玩体验的影响

- `alchemySkillLabel`（"炼丹 Lv.X · 本次火候容差 +N%"）不可见——玩家看不到自己的炼丹技艺等级与本次容差加成。
- "§7I 起炉 · F 注真元 · T 结算" 操作键提示不可见——新玩家在炼丹屏内无从得知三个核心操作键。
- `tempValueLabel`（"当前温度/目标温度"数值）不可见——火候对准只剩下方色带/knob 的粗略位置可猜，精确控温不可能。
- `qiValueLabel`（"已注真元/目标"数值）不可见——F 注真元只能盲注。

## 证据定位（行号基于 origin/main b398c4071）

- `client/src/main/java/com/bong/client/alchemy/AlchemyScreen.java:195-206`（`buildHeader`）：
  ```java
  h.child(Components.label(Text.literal("§f§l炼丹炉")));
  h.child(Containers.horizontalFlow(Sizing.fill(100), Sizing.content()));   // 整宽 spacer
  h.child(alchemySkillLabel);                                               // 被顶出
  h.child(Containers.horizontalFlow(Sizing.fill(100), Sizing.content()));   // 第二个整宽 spacer
  h.child(Components.label(Text.literal("§7I 起炉 · F 注真元 · T 结算"))); // 被顶出
  ```
- `client/src/main/java/com/bong/client/alchemy/AlchemyScreen.java:319-328`（`buildTempRow`）：`hdr.child(温标签)` → `hdr.child(fill(100) spacer)` → `hdr.child(tempValueLabel)`。
- `client/src/main/java/com/bong/client/alchemy/AlchemyScreen.java:344-353`（`buildQiRow`）：同型，受害者 `qiValueLabel`。
- `client/src/main/java/com/bong/client/alchemy/AlchemyScreen.java:42,45`：`PANEL_W=600` / `MID_W=220` 固定宽度。
- owo 0.11.2 sources jar：`Sizing.java:31-37`（FILL = value% × 传入 space）、`FlowLayout.java:183-185,231-233,286-288`（每个 child 对同一 childSpace inflate，顺序 mount，无剩余空间分配/无 shrink）。
- 同仓先例：`client/src/main/java/com/bong/client/craft/CraftScreen.java` `buildHeader` 注释 + `CraftActionBar.java` 注释（历史实锤：spacer 放中间会吃掉后续节点，手搓台副标题/「开始制作」按钮曾因此消失）。

## 触发路径

1. 玩家放置丹炉（主手炉物品右键 → `sendAlchemyFurnacePlace`），右键炉子方块（`MixinClientPlayerInteractionManagerAlchemy.interactBlock` → `AlchemyScreenBootstrap.requestOpenAlchemyScreen`）。
2. `AlchemyScreen.build()` → `buildHeader()` / 炉体列 `buildTempRow()` `buildQiRow()`。
3. owo 布局：fill(100) spacer 占满父宽，后续 label mount 在边界外——打开即命中，无需任何特殊状态。

## 反方审查记录

### Round 1（无上下文 review-skeptic subagent）

**反方结论**：NOT_REAL。理由：① AlchemyScreen 的打开路径"未被生产验证"，bot 场景走后端 intent 不开屏；② owo 对 horizontalFlow 多子节点"可能有隐式 shrink-to-fit"；③ CraftScreen 注释只证明手搓台情景，不能外推。

**裁决：驳回，判 REAL**。逐条反驳：

1. 打开路径是真实生产链路：`MixinClientPlayerInteractionManagerAlchemy.interactBlock` 对 FURNACE 方块 + `AlchemyFurnaceInteractionRules.shouldOpenAlchemyFurnace` 直接 `requestOpenAlchemyScreen`，玩家右键即达；"bot 测试没开过屏"只说明测试盲区，不构成不可达。
2. "隐式 shrink"被 owo 0.11.2 **源码直接否定**：`FlowLayout` 布局对每个 child `inflate(childSpace)` 用的是同一份完整空间（`FlowLayout.java:183-185`），`Sizing.inflate` 的 FILL 分支就是 `value% × space`（`Sizing.java:34`），全程无剩余空间计算、无 clamp、无 shrink。
3. CraftScreen 注释描述的是 `Sizing`/`FlowLayout` 库级语义（并给出 `Sizing.inflate` 出处），非手搓台特例；且同语义已在本仓两个不同屏幕（CraftScreen #205 副标题、CraftActionBar 按钮）实锤过两次。

## Skeleton Fix Plan

| 阶段 | 主题 | 路由 | 状态 |
|------|------|------|------|
| P0 | 三处 fill spacer 重排 + 布局回归测试 | fix_pr | ⬜ |

### P0 — 三处 fill spacer 重排

- 参照 `CraftScreen.buildHeader` 既有修法：spacer 移到行尾，或改用固定间距/右对齐（`horizontalAlignment` + `Sizing.content()` 组合），保证 `alchemySkillLabel`、键位提示、`tempValueLabel`、`qiValueLabel` 全部落在父容器内。
- 修复面限定 `AlchemyScreen.java` 三个 builder，不动数据流/store/协议。
- 回归测试：仿 `CraftScreen` 的 layout 描述类测试（或新增 headless 布局断言），pin「header 全部子节点 x+width ≤ 父容器宽度」；至少覆盖 buildHeader/buildTempRow/buildQiRow 三处。

## 验收测试计划

1. `cd client && ./gradlew test build`（JDK 17）——新布局 pin 测试绿。
2. `runClient` 目视：打开炼丹屏可见炼丹 Lv 标签、键位提示、温度与真元数值；起炉后数值随 session 更新可读。

## 风险

- 600px 面板 header 同行塞四组文案，spacer 重排后可能拥挤——必要时缩短提示文案或分两行，属实施细节。
- `refreshSessionText`/`refreshAlchemySkillText` 原地 `text()` 更新不受重排影响，无协议/状态耦合风险。

## 审计来源

Client-B 分区 bughunt 定点轮。方法：全 Screen `Sizing.fill` 扫描 → 与 CraftScreen/CraftActionBar 历史修复对拍 → owo 0.11.2 sources jar 源码级验证（`Sizing.inflate` / `FlowLayout` 布局算法）→ origin/main（b398c4071）复验 + 文档去重 → 无上下文 review-skeptic 对抗（NOT_REAL 异议被源码证据驳回，过程如实记录于上）。本 PR 仅新增 report-only skeleton，不改代码。
