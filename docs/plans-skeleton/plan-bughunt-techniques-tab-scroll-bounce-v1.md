# BugHunt: 功法面板列表整体重建刷新，滚动位置随每次刷新弹回顶部

## Bug 摘要

**严重度：medium**（未调整）

`TechniquesTabPanel.refreshVisibleTechniques()`（`client/src/main/java/com/bong/client/combat/inspect/TechniquesTabPanel.java:145-186`）每次刷新功法列表都先整体 `techniqueList.clearChildren()`（149 行）清空 `techniqueScroll` 包裹的 `FlowLayout`，再逐条重建行组件。这是本仓库已知的 owo-lib 反模式：`ScrollContainer.layout()` 在被包裹内容清空的瞬间把 `maxScroll` 算成 0 并把 `scrollOffset` 钳回 0，随后行加回来滚动位置也回不来。玩家已学会的功法数超过约 8 条（出现滚动条）后，只要触发一次刷新——搜索框每敲一个字符，或 server 推送新的 `techniques_snapshot`（学会新招/熟练度变化）——正在往下滚看的功法列表就会瞬间弹回顶部。

本仓库同一 owo 反模式已经在别处踩过并修复：`CraftRecipeListWidget.java` 用「按 id 序列 diff、只在结构变化时重建、同 id 项原地更新」的模式规避（`refresh()` / `shouldRebuildRows()` / `needsRebuild()`，47-156 行），`ScrollReadScreen.java` 的类文档注释（32-40 行）也明确记录了这个坑。`TechniquesTabPanel` 是这个已知修复模式的漏网实例。

## 实际游玩体验影响

跨越多个境界、习得功法较多的中后期角色打开 InspectScreen「功法」Tab、把长列表滚动到中间或底部查看某个招式详情，此时只要在搜索框里敲一个字符，或者游戏内任何提升某功法熟练度 / 学会新功法的正常游玩行为触发 server 端 `techniques_snapshot` 重推，列表就会不由分说地弹回顶部，玩家刚才滚动定位的位置全部丢失——需要重新滚一遍才能继续看。这是一个持续性的 UX 摩擦：功法列表本该是稳定可浏览的参考面板，却因为后台数据刷新（哪怕玩家完全没有主动操作列表）而不断打断玩家的浏览状态，尤其在玩家一边战斗一边打开面板查看功法配置时（熟练度随时在涨）体验更差。

## 证据定位

- `client/src/main/java/com/bong/client/combat/inspect/TechniquesTabPanel.java:70-79` — `techniqueList`（70 行 `Containers.verticalFlow` 声明）被 `Containers.verticalScroll(...)` 包裹成 `techniqueScroll`（76-78 行），固定视口高度 `LIST_VIEWPORT_HEIGHT = 180`（31 行，注释标明约 8 行才出现滚动条）。
- `client/src/main/java/com/bong/client/combat/inspect/TechniquesTabPanel.java:139-186` — `rebuildTechniques()`（139-143 行）调用 `refreshVisibleTechniques()`（145-186 行）；后者 149 行 `techniqueList.clearChildren()` 清空整棵子树，150 行清空 `techniqueRows`，158-178 行 for 循环逐条 `new TechniqueRowComponent(...)` + `techniqueList.child(row.component())` 重建，全程未做 id 序列 diff、不判断"内容是否真的变了"。
- `client/src/main/java/com/bong/client/combat/inspect/TechniqueSearchBar.java:17-20` — `input.onChanged().subscribe(...)` 每次按键都会触发 `query` 更新并回调 `onChanged.accept(query)`；`TechniquesTabPanel.java:64-67` 把这个回调直接接到 `refreshVisibleTechniques()`，即每敲一个字符就清空重建一次列表。
- `client/src/main/java/com/bong/client/combat/inspect/TechniquesTabPanel.java:108-113` — `techniquesListener` 注册进 `TechniquesListPanel.addListener(techniquesListener)`（113 行），`client.execute(() -> rebuildTechniques(next))`（110 行）在每次 `TechniquesListPanel.replace(...)`（`TechniquesListPanel.java:183-188`）被调用时触发。
- `server/src/network/techniques_snapshot_emit.rs:14-22` — `TechniquesSnapshotFilter = (With<Client>, Changed<KnownTechniques>)`（14 行），`emit_techniques_snapshot_payloads`（18-24 行）在 `KnownTechniques` 组件 `Changed` 时（学会新功法 / 熟练度变化等正常游玩路径，非 dev-only）向对应 client 重推 `techniques_snapshot`，最终驱动上面的 `techniquesListener` 回调。
- `client/src/main/java/com/bong/client/craft/CraftRecipeListWidget.java:36-156`（尤其 47-48、101-139、146-156 行）— 本仓库已有的正确范式：`renderedIds` 记录上次渲染的 id 序列（47-48 行），`refresh()`（101-139 行）先算 `nextIds`，`shouldRebuildRows()` / `needsRebuild()`（146-156 行）判断 id 序列是否结构性变化，未变化时走 `applyRowContent()` 原地更新文案/颜色/tooltip（109-121 行），只有结构变化才 `rows.clearChildren()` 重建（124 行起）。
- `client/src/main/java/com/bong/client/scroll/ScrollReadScreen.java:32-40` — 类文档注释显式记录同一坑及规避方式（"翻页刷新用 `LabelComponent#text` 原地更新...不 `clearChildren()` 重建"）。
- **补充证据（复核时新增）**：`client/src/main/java/com/bong/client/combat/inspect/TechniqueRowComponent.java:21,35,80-100` — `technique` 字段是 `final`，构造时绑定，`refresh(boolean selected, String lockReason)`（80-100 行）只用这个固定字段重渲染，**没有接口可以把新的 `Technique` 数据（比如新的熟练度）灌进已存在的行**。这意味着直接照搬 `CraftRecipeListWidget` 的 `applyRowContent` 模式并不够——即便 id 序列不变，若该 id 对应功法的熟练度/激活态等字段变了（这正是 `techniques_snapshot` 重推最常见的触发原因），复用旧 `TechniqueRowComponent` 实例而不更新其内部 `technique` 引用会导致行内容 stale。Skeleton Fix Plan 必须先给 `TechniqueRowComponent` 加一个能替换绑定 `Technique` 的更新入口，这是本 finding 相对 `CraftRecipeListWidget` 范本的额外落地工作。

## 触发路径

1. **路径 A（搜索框每次按键）**：玩家已学会 >8 条功法（滚动条出现），在功法 Tab 把列表滚动到非顶部位置，然后在搜索框里敲任意一个字符 → `TechniqueSearchBar.java:17-20` 的 `onChanged` 回调直接调用 `TechniquesTabPanel.java:64-67` 的 `refreshVisibleTechniques()` → `clearChildren()`（149 行）清空 `techniqueScroll` 的子树 → owo `ScrollContainer.layout()` 把 `maxScroll` 算成 0、`scrollOffset` 钳回 0 → 行逐条加回来（158-178 行）但滚动位置已经回不来了。
2. **路径 B（server 推送 `techniques_snapshot`）**：玩家打开功法 Tab 并滚动列表，随后进行任意会改变 `KnownTechniques`（学会新功法 / 熟练度提升 / 激活态切换）的正常游玩行为 → server 端 `Changed<KnownTechniques>` 过滤器命中（`techniques_snapshot_emit.rs:14`）→ 重新序列化并推送 `techniques_snapshot` payload → client 收到后驱动 `TechniquesListPanel.replace(...)` → 遍历 `listeners`（`TechniquesListPanel.java:187`）回调到 `TechniquesTabPanel.java:108-113` 的 `techniquesListener` → `client.execute(() -> rebuildTechniques(next))` → `rebuildTechniques()`（139-143 行）→ `refreshVisibleTechniques()`（145-186 行）→ 同上 `clearChildren()` 触发滚动钳位归零。

两条路径都发生在正常游玩范围内，不依赖 dev-only 命令，且互相独立触发（玩家可能什么都没打字就纯被 server 推送打断）。

## 反方审查记录

- 第一轮质疑：
  - 是否可能是误报——owo `ScrollContainer` 会不会在 `clearChildren()` 后自动记忆并恢复 `scrollOffset`？逐层核对 owo-lib 0.11.2+1.20 源码路径：`BaseParentComponent.child()/clearChildren()` 同步调用 `updateLayout()`，触发被清空的 `FlowLayout` 重新 `inflate()`→`layout()`，尺寸变化后经 `onChildMutated()` 向上冒泡到外层 `ScrollContainer`，其 `layout()` 按子节点**当前**尺寸重算 `maxScroll` 并把 `scrollOffset` clamp 进 `[0, maxScroll+.5]`——`scrollOffset` 一旦被钳到 0 之后没有任何恢复逻辑。结论：不是误报，机制成立。
  - 是否该组件根本不会触发滚动条（因为内容总是很短）？核对 `LIST_VIEWPORT_HEIGHT = 180` 注释「约 8 行才出现滚动条」——中后期角色（跨越多个境界后）习得功法数超过 8 条是常见情形，可达性成立。
  - 是否已有同类修复覆盖了这个组件？搜索仓库内已落地的同款坑修复位置（`CraftRecipeListWidget.java`、`ScrollReadScreen.java`），确认二者均已用 id-diff / 原地更新规避，但 `TechniquesTabPanel.refreshVisibleTechniques()` 仍是整体 `clearChildren()` 重建，未套用同一修复模式——这是范式已知但漏网的具体实例，不是"已知问题、已经处理"。
  - 是否与在跑的其他 bughunt skeleton 撞车？检索 `docs/plans-skeleton/` 与开放 PR，`plan-bughunt-cast-sync-config-window-thread-v1.md` 同样涉及 `TechniquesTabPanel`，但那份关注的是 `SkillConfigPanelManager` 的线程安全问题，与本 finding 的滚动条钳位是完全不同的缺陷类别和触发条件，无重叠。
  - 初裁：倾向通过。
- 第二轮补证：
  - 复核 `TechniqueRowComponent` 内部结构，发现其 `technique` 字段为 `final`、无更新接口——这意味着即使照搬 `CraftRecipeListWidget` 的「id 不变则原地更新」模式，也不能简单复用旧行实例展示新熟练度数据，必须先给 `TechniqueRowComponent` 补一个更新方法。这一点补进 Skeleton Fix Plan，作为比原始 `fix_sketch` 更细的落地要求。
  - 让步：本 finding 未新增可执行的自动化测试，目前是通过源码路径 + 已知同款坑修复范本的静态复现；`ScrollContainer`/`FlowLayout` 的具体钳位算法引自 owo-lib 0.11.2+1.20 依赖源码而非本仓库文件，无法在本次复核中逐行重新验证该第三方库行为，但仓库内两处（`CraftRecipeListWidget.java`、`ScrollReadScreen.java` 文档注释）对同一机制的独立佐证已构成足够确证。
  - 终裁：通过。反方认为这是「已知修复模式未套用到新组件」的具体实例，范围明确、修复路径清晰，不需要扩大成通用 owo 组件基类重构。
- 主循环复核：已亲读关键行确认（`TechniquesTabPanel.java` 全文、`TechniqueSearchBar.java`、`TechniquesListPanel.java` addListener/replace、`techniques_snapshot_emit.rs`、`CraftRecipeListWidget.java` 全文、`ScrollReadScreen.java` 类注释、`TechniqueRowComponent.java` 全文），行号已按实际代码位置核对/修正（`refreshVisibleTechniques` 实际方法体为 145-186 行，`rebuildTechniques` 为独立的 139-143 行，二者合并引用时已在本文档拆开标注）。

## Skeleton Fix Plan

- [ ] 给 `TechniqueRowComponent` 增加一个更新入口（例如 `void update(TechniquesListPanel.Technique next, boolean selected, String lockReason)`），把当前 `final` 的 `technique` 字段改为可替换（或改用可变引用容器），使同一行组件能在不重建的情况下展示新的熟练度/激活态/绑定槽位数据。
- [ ] 在 `TechniquesTabPanel` 中新增 `Map<String, TechniqueRowComponent> rowById`（`LinkedHashMap` 保留插入顺序，同 `CraftRecipeListWidget.rowById` 的用法）与 `List<String> renderedIds` 字段，替代现有只在构造时使用一次的 `techniqueRows` 列表语义。
- [ ] 重写 `refreshVisibleTechniques()`：先计算 `nextIds = visibleTechniques.stream().map(Technique::id).toList()`；仿 `CraftRecipeListWidget.shouldRebuildRows()` / `needsRebuild()` 判断是否需要整体重建（首次刷新、或 id 序列结构性变化——新增/删除/重排/空↔非空 时才重建）。
- [ ] 不需要重建时：对 `nextIds` 中每个 id，取出 `rowById` 里已存在的 `TechniqueRowComponent`，调用新增的 `update(...)` 方法原地刷新文案（复用当前 `refresh()` 逻辑的渲染部分），**不调用 `techniqueList.clearChildren()`**，`techniqueScroll` 的 `scrollOffset` 不会被 owo 钳位归零。
- [ ] 需要重建时（id 序列结构性变化）：保留现有 `clearChildren()` + 逐条新建路径，但同步维护 `rowById` / `renderedIds`；`selectedTechniqueId` 兜底逻辑（179-181 行）与「选中态变化时通知 `configPanelManager`」（183-185 行）逻辑不变。
- [ ] 「空列表」占位 label（151-156 行，等待 `techniques_snapshot` / 未找到功法两种文案）分支必须保留在重建路径里，且首次刷新（`snapshot`/`visibleTechniques` 均为空）必须强制走一次重建——照抄 `CraftRecipeListWidget.rowsBuilt` 布尔标记，避免"空 id 序列 == 空 id 序列"被误判为"无需重建"导致占位 label 永远不渲染。
- [ ] 搜索框逐字符触发（`TechniqueSearchBar` 每次 `onChanged`）与 server `techniques_snapshot` 推送触发（`TechniquesListPanel` 监听）两条路径调用的都是同一个 `refreshVisibleTechniques()`，无需分别改造——修好这一个方法即覆盖两条触发路径。
- [ ] 本 bug 纯 UI 交互缺陷，不涉及真元/灵气流动、不涉及 C2S 权威判定，无需 `qi_physics::ledger` 或 server gate 相关改动。

## 验收测试计划

栈：`client/` JUnit（沿用 `CraftRecipeListWidget` 已有的 `shouldRebuildRows`/`needsRebuild` 风格纯逻辑单测，无需起 Minecraft client）。

- **happy path — id 序列不变时不重建**：构造 `visibleTechniques` 两次快照，id 序列完全一致（顺序也一致），断言 `TechniquesTabPanel` 对应的 `needsRebuild`（或新增的等价静态方法）返回 `false`，且 `refreshVisibleTechniques()` 调用后 `techniqueList` 的子组件引用（`FlowLayout` 内部 children 列表）与调用前完全相同（同一批 `TechniqueRowComponent.component()` 实例，未被替换），验证滚动位置得以保留（不依赖真实 owo 渲染，断言"引用未变"即等价于"未 clearChildren"）。
- **边界 — 空列表 → 空列表（首次刷新）**：`snapshot` 与 `visibleTechniques` 均为空的首次 `refreshVisibleTechniques()`，断言占位 label（"等待 techniques_snapshot…" 或 "未找到功法"）确实被渲染进 `techniqueList`，不因为"空 id 序列 == 空 id 序列"被误判跳过重建。
- **边界 — 单条功法 ↔ 空列表相互切换**：先有 1 条功法再清空（模拟搜索词过滤到 0 条），断言触发重建、占位 label 出现；再从空恢复到 1 条，断言重建、行组件重新出现。
- **状态转换 — 同 id 但字段变化（熟练度提升）**：`visibleTechniques` 前后 id 序列相同，但某个 `Technique` 的 `proficiency`/`proficiencyLabel`/`active` 字段不同，断言：① 不触发整体重建（子组件引用不变）；② 对应行组件的展示文案（`proficiencyLabel`、绑定槽位提示等）确实更新为新值——覆盖 `TechniqueRowComponent.update(...)` 新增入口的正确性，防止"未重建但也没更新数据"的假绿。
- **状态转换 — id 序列重排（相同集合、顺序变化）**：`nextIds` 与 `renderedIds` 集合相同但顺序不同，断言判定为需要重建（对齐 `CraftRecipeListWidget.needsRebuild` 的"含顺序"语义），重建后行组件渲染顺序与 `nextIds` 一致。
- **错误分支 — 搜索关键字不命中任何功法**：`searchBar.query()` 设置为不匹配任何功法的字符串，断言 `visibleTechniques` 为空、占位 label 文案为 "未找到功法"（区分于"等待 techniques_snapshot…"的初始态文案）。
- **回归 — 选中态与详情卡联动不受重构影响**：id 序列变化导致重建后，若之前选中的 `selectedTechniqueId` 仍在新列表中，断言选中状态与高亮保持；若不在新列表中，断言 fallback 到 `visibleTechniques.get(0)`（179-181 行既有逻辑）且 `configPanelManager.onSelectedTechniqueChanged` 被正确调用一次（183-185 行既有逻辑不回归）。
- **回归 — 两条触发路径都命中同一修复**：分别模拟「搜索框 `onChanged` 回调」和「`TechniquesListPanel` 监听器回调」两条路径调用到 `refreshVisibleTechniques()`，断言两者共享同一套 diff 判定，行为一致（防止只改了一条路径的调用点、漏了另一条）。

可选联调（非本 plan 必须）：手动在 `runClient` 里学到第 9 条功法出现滚动条后，向下滚动，敲搜索框任意字符，肉眼确认滚动条位置不再跳回顶部。

## 风险

- `TechniqueRowComponent` 从"构造时绑定 `final Technique`"改为"可更新引用"是本 finding 里唯一超出 `CraftRecipeListWidget` 现成范本的改动面，需要仔细核对 `refresh()` 内所有读取 `technique.*` 字段的地方（85-98 行）在改造后依然读到最新数据，避免"看似原地更新、实际仍读旧闭包变量"的隐蔽 stale bug。
- `selectedTechniqueId` 的 fallback 逻辑（179-181 行）与 `hoverTechniqueId` 清空（147 行）目前耦合在 `clearChildren()` 分支之后执行；重构成 diff 路径后要确认"不重建"分支下选中态/悬停态判断（尤其是 `findVisibleTechnique` 依赖的 `visibleTechniques` 字段）仍然正确刷新，不要因为跳过了重建分支而漏掉这部分状态同步。
- 本 bug 只涉及客户端 UI 组件树管理，无跨维度/守恒律/权威判定风险，回归范围局限在 `client/` 栈，`server/` 不受影响。
