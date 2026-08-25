# BugHunt: 采药自动采集 R 键与战斗 R 键默认冲突叠加陈旧会话快照，导致按键积压幽灵触发与同 tick 重复派发采集请求

## Bug 摘要

**严重度：medium**（skeptic 复核后维持 medium，未调整）。

`client/src/main/java/com/bong/client/botany/BotanyHudBootstrap.java` 与 `client/src/main/java/com/bong/client/combat/CombatKeybindings.java` 把两个语义完全不同的功能——战斗「法力量条持键预览」（`key.bong-client.spell_volume_hold`）与采药「自动采集」（`key.bong-client.botany_auto_harvest`）——都默认绑定到同一物理键 `GLFW_KEY_R`，是两个独立的 `KeyBinding` 实例。仓库自己在 `CombatKeybindings.java:65-69` 的注释里承认过同键双绑在本项目里是**真实故障**（旧版 `V` 键同时触发冲刺和截脉），但当时只对 `jiemai_react` 做了改默认 `UNKNOWN` 的止血，没有对 R 键做同等处理，只是给它加了一层「谁先仲裁到就消费」的机制（`BotanyHudBootstrap.shouldCaptureSpellVolumeKey()`）。

这道仲裁本身叠加了两个独立缺陷：

1. **非 interactive 态按键积压不排空**：`BotanyHudBootstrap.onStartClientTick` 在 `!session.interactive()`（玩家不在可自动采集的 botany session 中，例如正在战斗里按 R 预览法力量条）时会在触达 `autoHarvestKey().wasPressed()` 之前直接 `return`——`autoHarvestKey` 这个 `KeyBinding` 内部的按键计数器不会被消费，会持续累积。玩家随后一旦走近任意可自动采集的灵草使 session 变为 interactive，下一个 tick 的 `while (autoHarvestKey().wasPressed())` 就会把之前积压的战斗态按键一次性 drain 出来，凭空触发一次玩家当下并未按下、也并非本意的 AUTO 采集请求。
2. **dispatch 循环用陈旧 session 快照绕过 requestPending 门**：`onStartClientTick` 顶部只捕获一次 `HarvestSessionViewModel session = HarvestSessionStore.snapshot()`（不可变 record），随后整个 `while (autoHarvestKey().wasPressed()) { dispatchModeRequest(session, ...); }` 循环反复复用这同一个陈旧局部变量。`dispatchModeRequest` 内部的门禁 `session.requestPending()` 读的永远是 tick 开始时的旧值——即使 `HarvestSessionStore.requestMode(...)` 已经把静态 store 更新成 `requestPending=true` 的新 record，循环里的 `session` 局部变量也感知不到。只要同一 tick 内 `wasPressed()` 因排队计数 `>1` 而多次为真（无论是缺陷 1 的积压一次性 drain，还是玩家单纯在同一 20Hz tick 内快速连按两次 R），每次迭代都能穿过 `!session.requestPending()` 门禁，向 server 重复触发 `ClientRequestSender.sendBotanyHarvestRequest`。

两个缺陷独立存在、也会叠加放大：缺陷 1 制造积压 press，缺陷 2 让积压/连按都能绕过原本设计用来防重的 `requestPending` 门。

## 实际游玩体验影响

- 任何同时用过战斗 R 键（法力量条预览）与采药系统（采药 Lv.3 解锁自动采集，是正常成长路径，见 `BotanyHudBootstrap.java:20` `HERBALISM_AUTO_UNLOCK_LV = 3`）的玩家都会撞到：先在战斗/探索中按 R，随后走近一株可自动采集的灵草，会立即触发一次**玩家当下并未按下**的自动采集动作——体感上像是「灵草自己抢了操作」，与玩家意图完全脱节。
- 快速连按 R（自然的人类按键行为，无需毫秒级特技操作）如果落在同一个 20Hz tick 内，会让同一次意图产生**多次**独立的 `botany_harvest_request` C2S 派发，而不是被 `requestPending` 挡掉，造成协议层面的请求 spam。
- 更严重的是，`docs/plan-botany-harvest-mode-request-misroute-v1.md`（当前 active plan）已经证实：这条 `botany_harvest_request` 目前在 server 侧被错误路由进旧 `GameplayAction::Gather` 通道，即便切模式没有真正生效，也可能照常发放 `gather_qi_from_zone` 真元 / karma / inventory_score 收益。这意味着本 bug 造成的「幽灵触发 / 同 tick 重复派发」并非无害的 UI 抖动——只要 server 侧 misroute 仍未修复，client 每多发一次幽灵/重复请求，就是多一次命中那条奖励泄漏路径的机会。本 plan 范围只覆盖 client 侧重复派发本身，但风险评估必须把这层放大效应写清楚（见「风险」）。

## 证据定位

- `client/src/main/java/com/bong/client/botany/BotanyHudBootstrap.java:47-63`（`onStartClientTick`）：`!session.interactive() || client.currentScreen != null` 为真时在第 52-54 行直接 `return`，早于第 60-62 行的 `while (autoHarvestKey().wasPressed())`，导致非 interactive 期间的按键计数不被消费、持续积压。
- `client/src/main/java/com/bong/client/botany/BotanyHudBootstrap.java:95-102`（`autoHarvestKey()`）：唯一一个 `KeyBinding` 实例，默认键 `GLFW.GLFW_KEY_R`。
- `client/src/main/java/com/bong/client/botany/BotanyHudBootstrap.java:115-127`（`dispatchModeRequest`）：第 116 行 `if (!session.interactive() || session.sessionId().isEmpty() || session.requestPending())` 的 `session` 参数就是循环外层传入的同一个陈旧局部变量，从未在循环体内刷新。
- `client/src/main/java/com/bong/client/combat/CombatKeybindings.java:76-81`：`spellVolumeKey` 默认同样绑定 `GLFW.GLFW_KEY_R`（`key.bong-client.spell_volume_hold`），与 `autoHarvestKey` 是两个独立 `KeyBinding` 实例。
- `client/src/main/java/com/bong/client/combat/CombatKeybindings.java:65-69`：仓库自己的注释，第一手证据证明「同键双绑 = 两个 `KeyBinding.wasPressed()` 都会触发」在本项目里是真实发生过的故障（旧 `V` 键同时驱动冲刺与截脉），因此才把 `jiemai_react` 改成默认未绑定——但同型的 R 键冲突从未被同等处理，只加了一层仲裁（见下一条），而这层仲裁本身有缺陷。
- `client/src/main/java/com/bong/client/combat/CombatKeybindings.java:133-144`（`onTick` 内 spell-volume 边沿检测）：`if (BotanyHudBootstrap.shouldCaptureSpellVolumeKey())` 为真时战斗侧放弃处理 R——这是单向仲裁（战斗让位给 botany），但不解决 botany 侧按键积压和陈旧快照两个独立问题。
- `client/src/main/java/com/bong/client/botany/HarvestSessionStore.java:23-25`（`capturesReservedInput`）：直接转发 `snapshot().interactive()`，是 `shouldCaptureSpellVolumeKey()` 的最终依据。
- `client/src/main/java/com/bong/client/botany/HarvestSessionStore.java:27-36`（`requestMode`）：把新 record 写入**静态** `snapshot` 字段（`replace(current.withRequestedMode(mode, nowMillis))`），但循环体内复用的局部变量 `session` 不是这个静态字段的引用，无法感知更新。
- `client/src/main/java/com/bong/client/botany/HarvestSessionViewModel.java:5-20,178-198`（补充证据）：`HarvestSessionViewModel` 是不可变 Java `record`，`withRequestedMode` 返回全新实例而非原地修改字段；这从类型层面确认了「任何提前捕获的旧 `session` 局部变量都不可能反映之后的 `requestPending` 更新」，不是猜测。
- `client/src/test/java/com/bong/client/combat/CombatKeybindingsTest.java:52`：`assertDefaultKey(definitions, "key.bong-client.spell_volume_hold", GLFW.GLFW_KEY_R)` 现有测试实锤 R 默认键归属战斗侧，佐证冲突真实存在于当前 HEAD。
- `client/src/test/java/com/bong/client/botany/BotanyHudBootstrapTest.java`：现状只测 `resetOnDisconnect`，没有任何针对 `onStartClientTick` / `dispatchModeRequest` 内部按键消费/去重逻辑的用例——回归测试处于空白，修复必须一并补齐。

## 触发路径

**路径 A（战斗态按键积压 → 幽灵触发）**：

1. 玩家在战斗/探索中按下 R，意图触发法力量条持键预览（`spellVolumeKey`）；此时未处于任何可自动采集的 botany session（`HarvestSessionStore.snapshot().interactive() == false`）。
2. 由于 R 是共享物理键，`autoHarvestKey` 这个独立 `KeyBinding` 实例同样记录到一次按键，但 `onStartClientTick` 因 `!session.interactive()` 在第 52-54 行提前 `return`，从未调用 `autoHarvestKey().wasPressed()`，这次按键因此既没被消费也没被清空，滞留在 `KeyBinding` 内部计数里。
3. 玩家随后走近一株可自动采集的灵草，`HarvestSessionStore` 的 session 变为 interactive（且 `autoSelectable() == true`、`herbalismLv >= 3`）。
4. 下一个满足 interactive 条件的 tick，`while (autoHarvestKey().wasPressed())` 第一次真正被求值，滞留的按键被吐出，`dispatchModeRequest(session, AUTO)` 被调用——玩家在**这个 tick 里完全没有按 R**，却触发了一次 AUTO 采集请求。

**路径 B（同 tick 连按 → 绕过 requestPending 去重门）**：

1. 玩家已处于 interactive botany session，快速连按两次 R（人类正常按键节奏即可落在同一个 20Hz 客户端 tick 内）。
2. `onStartClientTick` 顶部捕获 `session = HarvestSessionStore.snapshot()`（此时 `requestPending() == false`）。
3. `while (autoHarvestKey().wasPressed())` 第一次迭代为真：`dispatchModeRequest(session, AUTO)` 校验 `session.requestPending() == false` 通过，调用 `HarvestSessionStore.requestMode(...)`（把**静态** `snapshot` 字段更新为 `requestPending=true` 的新 record）并 `sendBotanyHarvestRequest`。
4. `while` 循环第二次迭代因为按键计数还有余量再次为真：`dispatchModeRequest` 复用的仍是第 2 步捕获的**旧** `session` 局部变量，其 `requestPending()` 依旧读到 `false`（旧 record 的字段值），门禁被绕过，`sendBotanyHarvestRequest` 被第二次调用。
5. Server 在同一个逻辑请求周期内收到两次派发；配合 `plan-botany-harvest-mode-request-misroute-v1` 记录的 server 侧 misroute（目前落到旧 `GameplayAction::Gather` 通道），每一次幽灵/重复派发都是一次额外的潜在收益泄漏敞口。

## 反方审查记录

- 第一轮质疑：
  - 「Minecraft/Fabric 对同一物理键是否只会让一个 `KeyBinding` 变成 pressed，另一个监听器看不到事件？」——被仓库自身证据推翻：`CombatKeybindings.java:65-69` 白纸黑字记录旧 `V` 键冲突时「单次按 V 两个 `KeyBinding.wasPressed()` 都触发」，说明同物理键驱动多个独立 `KeyBinding` 实例各自计数在本项目是真实发生过的故障，不是理论假设；`CombatKeybindingsTest.java:52` 也证实 `spell_volume_hold` 当前确实默认绑 R，与 `autoHarvestKey` 冲突条件成立。
  - 「`shouldCaptureSpellVolumeKey()` 这层仲裁是否已经足够，是不是已经被 `docs/plans-skeleton/plan-bughunt-client-input-keybind-collision-v1.md` 当作『有显式仲裁、无需再修』的正面范例引用过？」——查证后发现：那份姊妹骨架确实把 `BotanyHudBootstrap.shouldCaptureSpellVolumeKey()` 引用为「显式仲裁」的正面对照，用来反衬 `O`/`U` 键完全没有仲裁层；但那份骨架审计范围只看「战斗侧是否会对同一个按键做出重复响应」，没有深入 botany 侧内部的按键消费顺序和 dispatch 循环实现。二者结论不冲突：R 键的仲裁层"存在"（战斗侧确实会让位），但仲裁层本身携带的两个内部缺陷（积压不排空、循环用陈旧快照）是姊妹骨架未覆盖的更深层问题，不是重复 finding。
- 第二轮补证：
  - 核对 `HarvestSessionViewModel` 的类型定义（`record`，`withRequestedMode` 返回新实例），从类型系统层面排除了「也许 session 局部变量其实是可变引用、能感知到静态字段更新」这一反证路径。
  - 核对 `dispatchModeRequest` 的 MANUAL 分支：`consumeManualPress` 只在 `onStartClientTick` 内部单次调用 `dispatchModeRequest(session, MANUAL)`（不在 while 循环里），因此 MANUAL 路径不受「同 tick 循环内陈旧快照绕过门禁」影响，只有 AUTO 分支因为处在 `while` 循环里才会重复派发——修复范围据此精确限定在 AUTO 路径 + 按键积压消费顺序，不需要动 MANUAL 分支。
  - 查重 `gh pr list`/skeleton 目录：`docs/plans-skeleton/plan-bughunt-client-input-keybind-collision-v1.md` 覆盖的是 `O`（IdentityPanel vs VoidAction）与 `U`（Forge vs ExtractInteraction）两组无仲裁双派发，明确排除/未涉及 R 键内部缺陷；`docs/plan-botany-harvest-mode-request-misroute-v1.md`（active）覆盖的是同一个 `dispatchModeRequest` 调用点**之后**、server 侧 `session_id` 错接旧 gather 通道的问题，是下游、不同层面的 bug，两者与本 finding 均不重叠，不构成重复立项。
  - 让步：未在本轮新增可执行测试，当前结论基于源码路径的静态复现（`wasPressed()` 计数语义 + record 不可变性 + tick 生命周期），验收阶段需要补齐真正的 JUnit pin。
  - 终裁：通过。反方认为这是需要同时修「按键积压排空」与「dispatch 循环快照刷新」两处的真实缺陷，长期根治应考虑拆分默认键位，但那是设计选择，不阻塞本次最小修复。
- 主循环复核：已亲读关键行确认。

## Skeleton Fix Plan

- [ ] 修复缺陷 1（按键积压不排空）：在 `BotanyHudBootstrap.onStartClientTick` 中，`!session.interactive() || client.currentScreen != null` 分支 `return` 之前，主动排空 `autoHarvestKey()` 的按键计数（例如 `while (autoHarvestKey().wasPressed()) { /* 丢弃，不 dispatch */ }`），确保非 interactive 期间产生的按键永远不会被下一次进入 interactive 状态的 tick 误当作「当下按键」。
- [ ] 修复缺陷 2（陈旧快照绕过 requestPending 门）：把 AUTO 分支的 `while (autoHarvestKey().wasPressed()) { dispatchModeRequest(session, BotanyHarvestMode.AUTO); }` 改为每次迭代前重新读取 `HarvestSessionStore.snapshot()`（而不是复用 tick 开头捕获的 `session` 局部变量），或者在 `dispatchModeRequest` 首次成功派发（即真正调用了 `HarvestSessionStore.requestMode(...)` 且 `sendBotanyHarvestRequest` 之后）立即 `break` 跳出 `while` 循环——两种任选其一均可，核心是保证 `requestPending()` 门禁在同一 tick 内读到的是**实时**状态，不是 tick 开始时的旧值。
- [ ] 保持 `consumeManualPress` / MANUAL 分支不变——第二轮反方审查已确认 MANUAL 路径不在 `while` 循环里，不受本 bug 影响，修复不应误伤该分支的现有行为。
- [ ] 为使新增的按键消费/去重逻辑可被 JUnit 测试到，参考 `CombatKeybindings.installBindings` / `consumeQuickSlotPresses` 已经把内部按键处理暴露成包内可见静态方法的先例，给 `BotanyHudBootstrap` 补一个可注入 `KeyBinding` 或可从测试触发单次 tick pump 的包内可见入口（不需要改变默认 `private`/`public` 的对外 API 边界，只需要测试能构造场景并驱动一次 `onStartClientTick` 等价逻辑）。
- [ ] 本 bug 是纯 client 侧的重复派发问题，不直接涉及真元/灵气流动，不需要引入新的 `qi_physics` 常数或走 ledger；但因为 `botany_harvest_request` 最终是 C2S 请求，**client 侧去重只是减少无意义网络流量的 UX/前端优化，不能被当成唯一的防重手段**——server 侧 `botany_harvest_request` handler（当前在 `docs/plan-botany-harvest-mode-request-misroute-v1.md` 范围内被记录为错接旧 `GameplayAction::Gather`）最终仍应对同一 session 的重复/高频 mode 请求具备幂等或节流的权威判定；本 plan 的修复只负责把 client 侧两处具体缺陷堵上，不越界修改 server handler，但要在验收阶段的联调用例里明确标注这层依赖关系。
- [ ] 修复完成后核对 `client/src/main/java/com/bong/client/hud/BotanyHudPlanner.java` 等 HUD 展示层是否因为 `requestPending` 状态刷新时机变化而需要同步调整（预期不需要，因为 UI 只读 `HarvestSessionStore.snapshot()`，但要在测试里显式验证一遍不回归）。

## 验收测试计划

栈：client JUnit（`client/src/test/java/com/bong/client/botany/BotanyHudBootstrapTest.java` 为主，必要时联动 `CombatKeybindingsTest.java`）。

- **happy path**：session interactive 且 `autoSelectable=true`、`herbalismLv>=3`，模拟单次 `KeyBinding.onKeyPressed(...)` 触发 `autoHarvestKey`，驱动一次等价于 `onStartClientTick` 的 pump 调用，断言：恰好一次 `HarvestSessionStore.requestMode(AUTO, ...)` 生效（`snapshot().requestPending()==true` 且 `mode()==AUTO`）、恰好一次 `sendBotanyHarvestRequest` 被记录。
- **边界（同 tick 连按/积压重复计数 ≥2）**：在同一次 pump 调用前，通过 `KeyBinding.onKeyPressed(...)` 连续触发两次，使 `wasPressed()` 在循环内为真两次；断言修复后**只有一次** `sendBotanyHarvestRequest` 被派发（对照修复前应为两次，验证回归锁定的是"计数一致性"而非"实现细节"）。
- **边界（非 interactive 态积压不排空）**：先在 `!session.interactive()` 状态下触发若干次 `autoHarvestKey().onKeyPressed(...)`，再把 `HarvestSessionStore` 切到 interactive 状态并跑一次 pump；断言**不产生任何** `dispatchModeRequest`/`sendBotanyHarvestRequest` 调用（玩家在这个 tick 没有真实按键，不应该有幽灵触发）——这是本 bug 的核心回归锁，必须显式断言"即使此前有积压按键，interactive 后的首个 tick 在没有新按键时不得触发"。
- **错误分支（session 非 interactive 时的正常静默）**：`!session.interactive()` 期间正常按 R，断言不调用 `dispatchModeRequest`、不改变 `HarvestSessionStore` 状态（现状行为，确认修复没有引入误报）。
- **错误分支（`autoSelectable=false` 或 `herbalismLv<3`）**：session interactive 但不满足 AUTO 解锁条件时按 R，断言 `dispatchModeRequest` 提前返回、`requestPending` 不变，不发请求（现有 `dispatchModeRequest` L119-124 逻辑保持不变的回归锁）。
- **状态转换（interactive → interrupted → interactive）**：session 从 interactive 变为 `interrupted`（如 `locallyInterrupted`）再恢复 interactive，确认恢复后的首个 tick 不会因为「非 interactive 期间残留的按键计数」误触发（覆盖缺陷 1 在中途打断场景下的等价情形，不只是"从未 interactive"这一种起点）。
- **状态转换（requestPending 由 false→true→false 的实时性）**：显式验证 `dispatchModeRequest` 使用的门禁读取的是**当次判定时刻**的 `HarvestSessionStore` 状态而非函数入参捕获的旧值——构造「第一次 dispatch 后 store 已经是 `requestPending=true`」，同一逻辑 tick 内如果 `wasPressed()` 又为真，断言第二次调用被门禁挡下且不产生副作用（这是缺陷 2 的直接契约测试，断言外部可观察副作用——`sendBotanyHarvestRequest` 调用次数与 `snapshot()` 字段——而不是绑死内部实现是"重读快照"还是"提前 break"哪种修法）。
- **共享键回归（跨模块）**：在 `CombatKeybindingsTest` 或联合测试里验证——botany session interactive 时按 R，`CombatKeybindings.onTick` 的 `shouldCaptureSpellVolumeKey()` 分支必须让位（不触发 `spellVolumeHandler`），且 `autoHarvestKey` 侧按本 plan 修复后的去重规则恰好派发一次；非 interactive 时按 R，战斗侧正常触发 `spellVolumeHandler`，botany 侧不产生任何派发（也不允许悄悄积压——已被上面的边界用例覆盖）。
- **联调标注**：验收文档需注明"client 侧去重通过后，仍需在 `plan-botany-harvest-mode-request-misroute-v1` 收口前假设 server 可能重复处理同一逻辑请求"，不得把 client 侧测试全绿等同于"协议层完全防重"。

## 风险

- 本 plan 只修复 client 侧的两处具体缺陷（按键积压排空 + dispatch 循环快照刷新），**不修改** server 侧 `botany_harvest_request` 的处理逻辑；`docs/plan-botany-harvest-mode-request-misroute-v1.md` 记录的 server 侧 misroute（错接旧 `GameplayAction::Gather`，可能发放真元/karma/inventory_score 收益）仍然存在。在那份 plan 收口之前，本 bug 造成的"幽灵触发/重复派发"依然是命中该收益泄漏路径的额外敞口——两份 plan 应被视为同一调用链上下游的独立缺陷，修复顺序不影响彼此正确性，但完整消除风险需要两者都落地。
- 长期根治建议：给「战斗法力量条预览」与「采药自动采集」拆分成互不冲突的默认键位（例如仿照 `jiemai_react` 把其中一个默认改为 `GLFW_KEY_UNKNOWN`，交给玩家在控制设置里显式绑定），从根源消除同键双绑，而不是继续靠软件层仲裁掩盖冲突——`docs/plans-skeleton/plan-bughunt-client-input-keybind-collision-v1.md` 已经在收集同型（`O`/`U`）冲突，未来如果做「全局默认键唯一性测试基建」，应该把 R 键这组也一并纳入白名单核查范围。这属于设计层面的后续工作，不在本 plan 最小修复范围内。
- 修复引入的「排空非 interactive 态按键」逻辑必须确认不会误伤真正合法的场景：例如玩家在非 interactive 状态下按 R 只是想预览法力量条、之后立刻走近灵草——这种情况下"排空"应该只丢弃 `autoHarvestKey` 内部的计数，不能影响 `spellVolumeKey`（两者是独立 `KeyBinding` 实例，正常互不干扰，但测试仍需覆盖以防意外耦合）。
- 若修复改为"重新读取快照"而非"首次派发后 break"，需注意 `while (autoHarvestKey().wasPressed())` 的循环条件本身也会继续消费按键计数——两种修法都必须保证按键计数被完全耗尽（不留新的积压），而不是只堵住 dispatch 副作用、却让计数继续累积到下一 tick。
