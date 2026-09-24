# BugHunt: 炼丹炉界面 owo fill(100) 中间占位符顶飞关键数值标签

## Bug 摘要

**严重度：high**

`client/src/main/java/com/bong/client/alchemy/AlchemyScreen.java` 里 `buildHeader()`、`buildTempRow()`、`buildQiRow()` 三处 `FlowLayout` 组装都踩了同一个已知反模式：在真实数据标签**前面**插入一个 `Containers.horizontalFlow(Sizing.fill(100), Sizing.content())` 占位符。owo-lib 的 `Sizing.fill(100)` 语义是"占父容器自身内容宽度的 100%"，不是"剩余空间"——放在两个子节点中间会让占位符独占整行宽度，把它后面所有子节点的渲染起始 x 坐标顶到可视区域之外。结果是：

- `buildHeader()`：`alchemySkillLabel`（炼丹等级 + 本次火候容差）以及行尾提示文字"I 起炉 · F 注真元 · T 结算"均被顶出可视区（header 里有两处中间占位符，逐级顶飞）。
- `buildTempRow()`：`tempValueLabel`（当前炉温读数）被顶出可视区。
- `buildQiRow()`：`qiValueLabel`（当前注入真元量）被顶出可视区。

这个反模式在同一仓库的 `WorkbenchScreen.java:186` 和 `CraftScreen.java:176` 已经被发现并修复（代码注释明确写"fill spacer 必须排在最后：owo fill(100) 占整宽，放中间会把副标题顶出右边界"），但 `AlchemyScreen.java` 从未同步这个修复，是漏网实例。

## 实际游玩体验影响

炼丹是一个实时操作的小游戏（起炉 → 按温度/真元曲线调控 → 结算），玩家依赖 HUD 上的当前炉温、当前注入真元量、以及自身炼丹熟练度/火候容差来判断操作时机。这三处关键数值标签被顶出可视区之后：

- 玩家看不到当前炉温读数（`tempValueLabel`），无法判断是否处于目标温度带内，只能盲操作温度调控热键。
- 玩家看不到当前注入真元量（`qiValueLabel`），无法判断真元是否已经足量/超量注入。
- 玩家看不到自己的炼丹等级和本次火候容差加成（`alchemySkillLabel`），也看不到操作提示文字。

炼丹面板打开时只剩标题"炼丹炉"和"炉体"分区标题可见，进度条/滑块track本身仍在（因为它们不是被顶飞的 label），但缺了所有文字读数，是一次实装完但视觉核心反馈缺失的体验断裂，玩家只能靠猜测节奏来完成本应数值驱动的炼丹操作。

## 证据定位

- `client/src/main/java/com/bong/client/alchemy/AlchemyScreen.java:195-206`（`buildHeader()`）：
  - L196：`FlowLayout h = Containers.horizontalFlow(Sizing.fill(100), Sizing.content());`
  - L198：先加标题 `§f§l炼丹炉`
  - L199：紧接着插入第一个 `Containers.horizontalFlow(Sizing.fill(100), Sizing.content())` 中间占位符
  - L200-202：`alchemySkillLabel`（真实数据，炼丹等级+火候容差）在占位符之后加入，会被顶出
  - L203：又插入第二个同款中间占位符
  - L204：行尾提示文字 `§7I 起炉 · F 注真元 · T 结算` 在第二个占位符之后加入，被顶得更远
- `client/src/main/java/com/bong/client/alchemy/AlchemyScreen.java:319-342`（`buildTempRow()`）：
  - L322：`hdr` 内先加 `§f温 ↑↓` 标题（L323）
  - L324：中间插入 fill(100) 占位符
  - L325-327：`tempValueLabel`（真实温度读数）在占位符之后加入，被顶出
- `client/src/main/java/com/bong/client/alchemy/AlchemyScreen.java:344-362`（`buildQiRow()`）：
  - L347：`hdr` 内先加 `§b§lF 注真元` 标题（L348）
  - L349：中间插入 fill(100) 占位符
  - L350-352：`qiValueLabel`（真实真元读数）在占位符之后加入，被顶出
- `client/src/main/java/com/bong/client/alchemy/AlchemyScreen.java:477-479`、`:504-535`：`refreshAlchemySkillText()`/`refreshFurnaceText()` 证实这三个 label 都由真实 server 会话/技能快照驱动文字内容，不是死代码/占位符。
- **对照正确写法**：`client/src/main/java/com/bong/client/craft/WorkbenchScreen.java:186-187` 和 `client/src/main/java/com/bong/client/craft/CraftScreen.java:176-177` 均把 fill(100) 占位符放在 `.child(...)` 序列**最后**，并各自留有明确中文注释记录这一坑（"fill spacer 必须排在最后：owo fill(100) 占整宽，放中间会把副标题顶出右边界"）。
- `client/src/main/java/com/bong/client/mixin/MixinClientPlayerInteractionManagerAlchemy.java:163` → `client/src/main/java/com/bong/client/alchemy/AlchemyScreenBootstrap.java:14`（`requestOpenAlchemyScreen`）→ `new AlchemyScreen(pos)`：确认打开路径是普通右键交互炼丹炉方块，无需 dev 命令。
- `client/src/test/java/com/bong/client/alchemy/AlchemyScreenSkillHeaderTest.java`：现有测试只覆盖 `formatAlchemySkillHeader()`（纯字符串格式化）和 `feedCountForSlot()`，从未构建 `FlowLayout` 树，捕获不到本布局顺序缺陷。
- `client/src/test/java/com/bong/client/alchemy/AlchemyScreenInventoryWiringTest.java:30-41`：仓库里已明确记录并证实的 owo 无头测试限制——`Components.label(...)` 在无头 JUnit 环境下会因 `MinecraftClient.getInstance()==null`（`LabelComponent` 构造要读 `MinecraftClient.textRenderer`）抛 NPE，"全仓没有任何一个 `BaseOwoScreen` 子类的 `build()`/`removed()` 走完整 UI 树被单测调用过"。这意味着验收测试不能靠直接调用 `buildHeader()`/`buildTempRow()`/`buildQiRow()` 来断言渲染树，必须走该文件已验证可行的"抽出纯数据方法 + 无头断言"路数（同 `formatAlchemySkillHeader`/`feedCountForSlot` 的解法）。

## 触发路径

1. 玩家在世界内右键交互一个炼丹炉方块（无需任何前置条件、无需 dev 命令）。
2. `MixinClientPlayerInteractionManagerAlchemy.java:163` 拦截交互，调用 `AlchemyScreenBootstrap.requestOpenAlchemyScreen(client, pos)`。
3. `AlchemyScreenBootstrap` 构造 `new AlchemyScreen(pos)` 并 `client.setScreen(...)`。
4. `AlchemyScreen.build(FlowLayout root)`（L107-133）依次调用 `buildHeader()`（L117）、`buildFurnaceColumn()`（L122，内部又调用 `buildTempRow()`/`buildQiRow()`，L281-282）。
5. 三处方法各自在真实数据标签之前插入一个 `Sizing.fill(100)` 的 `horizontalFlow` 占位符。
6. owo-lib `FlowLayout` 按子节点加入顺序累加渲染偏移量，每个 `fill(100)` 子节点独立取父容器自身内容宽度的 100%（不是"剩余空间"），中间占位符因此把该行剩余宽度用尽，其后所有子节点的起始偏移都超出可视范围。
7. `alchemySkillLabel`、行尾提示文字、`tempValueLabel`、`qiValueLabel` 四个真实数据标签渲染坐标落在面板可视区之外，玩家肉眼不可见；进度条 track 本身不受影响（它们不是被顶飞的对象）。

## 反方审查记录

- 第一轮质疑：
  - 会不会是死代码/占位符文案，从未被真实数据驱动？——查 `refreshAlchemySkillText()`（L477-479）与 `refreshFurnaceText()`（L504, L534-535）证实 `alchemySkillLabel.text(...)`/`tempValueLabel.text(...)`/`qiValueLabel.text(...)` 均在会话/技能快照刷新时被写入实时数据，不是死代码。
  - 会不会与已有 in-flight/已归档 bughunt plan 重复？——查 `plan-bughunt-alchemy-ui-session-stale`（讨论的是数据陈旧/刷新时机问题，非布局位置）与已归档 `plan-bughunt-ac-alchemy-hud-zero-targets-v1`（讨论的是 server 端下发零值目标数值的正确性问题，已关闭），两者均未涉及本 finding 的 fill(100) 子节点顶飞问题，判定非重复。
  - 会不会是 owo-lib 的"剩余空间"语义，占位符放中间其实无害？——反查同仓库 `WorkbenchScreen.java:186` / `CraftScreen.java:176` 的显式代码注释与"占位符放最后"写法，证明团队自己已经踩过这个坑并记录下"fill(100) 占整宽、放中间会顶飞"的结论，AlchemyScreen 只是没有同步这个已知修复。
  - 初裁：倾向通过。
- 第二轮补证：
  - 复核可达性：确认打开路径是 `MixinClientPlayerInteractionManagerAlchemy.java:163` 响应普通右键交互，非 dev 命令，任何玩家均可触发。
  - 复核测试盲区：`AlchemyScreenSkillHeaderTest.java` 只测字符串格式化和补料计数，从未构建 `FlowLayout` 树，无法撞见此问题；同时发现 `AlchemyScreenInventoryWiringTest.java:30-41` 里已经记录了"owo `Components.label` 无头环境下会 NPE，全仓没有 `BaseOwoScreen.build()` 被直接单测调用过"这一约束——这意味着验收测试必须走"抽出纯数据方法再断言"的路数，不能直接调 `buildHeader()` 校验渲染树。
  - 让步：本 finding 只锁定 `buildHeader`/`buildTempRow`/`buildQiRow` 三处，`AlchemyScreen.java` 其余 `FlowLayout` 组装（如 `buildScrollColumn`/`buildBackpackColumn`/`buildInterventionsAndInfo`）未逐一复核是否存在同款反模式，留待后续巡查。
  - 终裁：通过。反方认为这是纯客户端布局顺序缺陷，不涉及协议/守恒/权限设计问题，修复范围明确、风险低。
- 主循环复核：已亲读关键行确认（`AlchemyScreen.java:195-206`/`319-342`/`344-362` 逐行核对，`WorkbenchScreen.java:186-187`、`CraftScreen.java:176-177` 对照写法核实，`AlchemyScreenInventoryWiringTest.java:30-41` 的无头测试限制说明核实）；额外发现 `buildHeader()` 实际有**两处**中间占位符（L199 和 L203），第二处会把行尾提示文字顶得比 `alchemySkillLabel` 更远——比原 finding 描述的"顶出 alchemySkillLabel"更严重，已在本文档"Bug 摘要"与"证据定位"中补充说明。

## Skeleton Fix Plan

- [ ] `buildHeader()`（L195-206）：将 L199 和 L203 两处中间 `Containers.horizontalFlow(Sizing.fill(100), Sizing.content())` 占位符全部移到 `.child(...)` 序列的**最后**（即先加标题 → `alchemySkillLabel` → 行尾提示文字，占位符留到最后，若需要两段独立留白可以改用固定宽度 spacer 或 `gap(...)` 承担，不再用满宽 fill 卡在中间），写法对照 `WorkbenchScreen.buildHeader()`（`client/src/main/java/com/bong/client/craft/WorkbenchScreen.java:178-189`）/`CraftScreen.buildHeader()`（`client/src/main/java/com/bong/client/craft/CraftScreen.java:169-180`）。
- [ ] `buildTempRow()`（L319-342）：将 L324 的中间占位符移到 `tempValueLabel`（L325-327）加入之后。
- [ ] `buildQiRow()`（L344-362）：将 L349 的中间占位符移到 `qiValueLabel`（L350-352）加入之后。
- [ ] 为解决 owo `Components.label(...)` 无头 JUnit NPE 限制（`AlchemyScreenInventoryWiringTest.java:30-41` 已验证），把三处方法的"子节点加入顺序"抽成一个不依赖 `MinecraftClient` 的纯数据方法（例如返回 `List<ChildRole>` 或等价的有序枚举序列，`ChildRole` 区分"数据标签"与"占位符"），`buildHeader()`/`buildTempRow()`/`buildQiRow()` 内部按该序列真实构建子节点——让运行时构建与测试断言共享同一份顺序声明，杜绝再次分叉。参照仓库里 `formatAlchemySkillHeader`/`feedCountForSlot` 已经用"抽出纯逻辑方法供无头测试"解决同类限制的先例。
- [ ] 本 fix 范围内**不**扩大到 `AlchemyScreen.java` 其余 `FlowLayout` 组装（`buildScrollColumn`/`buildBackpackColumn`/`buildInterventionsAndInfo`/权重行等）；若实施时发现同款反模式，记录到本 plan 的"风险"节或另开 bughunt finding，不在本次顺手改动范围之外扩大 diff。
- [ ] 本修复不涉及真元/灵气流动（纯客户端 UI 布局问题），不涉及 `qi_physics::ledger` 守恒口径；不涉及任何 C2S 协议改动，`AlchemyScreen` 现有的温度/真元读数仍然完全来自 server 权威下发的 `AlchemySessionStore.Snapshot`（server gate 权威性不受影响，本次只是让已经正确下发的数值在客户端重新变得可见）。

## 验收测试计划

栈：`client/`（`./gradlew test`）。因 owo `Components.label(...)` 在无头 JUnit 下依赖 `MinecraftClient.getInstance()`（已确认会 NPE，见 `AlchemyScreenInventoryWiringTest.java:30-41`），本计划的测试断言目标是 Skeleton Fix Plan 中抽出的**纯数据子节点顺序方法**（不直接调用 `buildHeader()`/`buildTempRow()`/`buildQiRow()` 走完整 owo 渲染树），并辅以现有测试的回归保护：

- **happy path**：新增 `AlchemyScreenLayoutOrderTest`（或并入 `AlchemyScreenSkillHeaderTest`），断言：
  - header 子节点顺序序列中，标题、`alchemySkillLabel` 对应角色、行尾提示文字对应角色均出现在**所有**占位符角色**之前**（即占位符角色的索引严格大于三个数据角色的索引）。
  - temp row / qi row 子节点顺序序列中，标题角色出现在占位符角色之前，占位符角色出现在数据标签角色（`tempValueLabel`/`qiValueLabel` 对应角色）之前——即断言修复后的正确顺序：`[标题, 占位符, 数据标签]`（若采用"占位符最后"的修法，则断言 `[标题, 数据标签, 占位符]`；测试断言的具体顺序需与实际选定修法一致，但核心不变式是"数据标签不得出现在占位符之后"）。
- **边界**：断言 header 序列里两个占位符角色（对应原 L199/L203）在修复后**均不**位于任何数据标签角色之前；即使未来 header 只保留一个占位符，测试也要对"占位符个数"与"占位符位置"分别断言，不写死具体个数为魔法数字。
- **错误分支 / 回归锁定**：保留并跑通现有 `AlchemyScreenSkillHeaderTest`（`headerShowsEffectiveLevelAndToleranceBonus`/`headerShowsSuppressedEffectiveLevelWhenOverCap`/`feedCountUsesRecipeRequirementInsteadOfSingleItem`/`feedCountFallsBackToAvailableStackWhenRequirementIsLarger`）全绿，证明本次修复不改变文本格式化逻辑，只改子节点加入顺序。
- **状态转换**：`AlchemyScreenSessionPresentationTest`（`client/src/test/java/com/bong/client/alchemy/AlchemyScreenSessionPresentationTest.java`）覆盖的会话状态转换（不同 `sessionPresentation` 快照下 `refreshFurnaceText()` 写入 `tempValueLabel`/`qiValueLabel` 文本）保持全绿，证明本次修复不影响数据驱动链路，只影响子节点是否可见。
- **可选联调**：`./gradlew runClient` 手动右键炼丹炉方块，肉眉核验炼丹等级/火候容差、行尾提示、当前炉温、当前真元读数四段文字均在面板可视区内完整显示（对照修复前的"只剩标题可见"截图）。

## 风险

- 本次修复只锁定 `buildHeader()`/`buildTempRow()`/`buildQiRow()` 三处；`AlchemyScreen.java` 内其余 `FlowLayout` 组装（`buildScrollColumn`/`buildBackpackColumn`/`buildInterventionsAndInfo`/权重行）未逐一复核是否存在同款反模式，可能是遗留的同类缺口，需要后续巡查或另开 finding，不在本 plan 范围内顺手修。
- owo `Components.label(...)` 的无头测试限制（`MinecraftClient.getInstance()==null` NPE）意味着验收测试只能通过"抽出纯数据顺序方法"间接锁定行为，无法对最终渲染坐标做像素级断言；如果实施时改法与"抽出纯数据方法"路线不同，需要相应调整验收测试的具体断言写法，但"数据标签不得被占位符顶出"这一核心不变式必须保留。
- header 里两处占位符的具体归并方式（挪到最后合并成一个，还是各自挪到对应数据标签之后保留两处）会影响视觉留白效果，需要实施时对照 `WorkbenchScreen`/`CraftScreen` 的实际观感做一次目测确认，避免修复后标题和提示文字之间完全没有间距挤在一起。
