# plan-bughunt-silent-signal-runtime-bridge-v1（骨架）

> **骨架（草案）**。一句话主题：`plan-pvp-encounter-v1` 已归档宣称完成的 **P1 动作信号系统**，实际只落了 `SilentSignalSystem` 纯检测器和单测，**没有任何 client 运行时注册、状态采样、HUD 消费或 per-remote-player 桥接**；因此火把和平、骨币示好、慢退、双蹲、指向、打坐六类“沉默信号”在正式游玩里**全部不生效**。

> **这个 bug 对实际游玩体验的影响**：多人遭遇时，玩家会以为“切火把 / 丢骨币 / 双蹲 / 打坐”这些动作能像归档 plan 说的那样传递社交信号，但实际对面客户端没有任何被系统识别的反馈层；结果是 PvP 遭遇重新退化成“只能靠裸动作猜”或直接开打，`plan-pvp-encounter-v1` 设计的匿名社交张力缺了最关键的一层。

## 结论

- **类型**：`social/client UI/状态桥接` 断裂
- **严重度**：major
- **建议路由**：`fix_pr`
- **范围排除**：不涉及本轮禁区 `social anonymity live refresh`，也不涉及 `identity/social renown bridge`

## 复现路径

1. 双客户端进同一区域，保持 15 格内相遇。
2. A 玩家依次执行归档 plan 承诺的六种动作之一：切火把、丢 1 枚骨币、缓慢后退、双蹲、空手指向 2s、原地打坐 3s。
3. B 玩家观察客户端：
   - 预期：依据 `docs/finished_plans/plan-pvp-encounter-v1:118-144`，应至少出现“对方给出了某种沉默信号”的客户端可见层；火把和平信号甚至明确写了“对方 HUD 显示对方手持火把 icon”。
   - 实际：现有 client runtime 没有任何 `SilentSignalSystem` 接线，B 客户端不会出现任何信号 HUD / icon / overlay / toast / store 更新。
4. 静态证据可直接确认这不是“偶发漏触发”，而是**整条运行时链路不存在**：
   - `client/src/main/java/com/bong/client/social/SilentSignalSystem.java:20-53` 只有纯函数 `detect(ActionSnapshot)`。
   - `client/src/main/java/com/bong/client/social/SilentSignalSystem.java:85-97` 定义了 `ActionSnapshot`，但主代码树没有生产者。
   - `client/src/main/java/com/bong/client/BongClient.java:136-145` 注册了 `SpiritNicheRevealBootstrap`、`SparringInviteScreenBootstrap`、`TradeOfferScreenBootstrap` 等，但没有任何 silent-signal bootstrap。
   - `client/src/test/java/com/bong/client/social/SilentSignalSystemTest.java:13-118` 只证明“纯函数单测可过”，不证明 runtime 已接通。

## 根因链路

1. `plan-pvp-encounter-v1` 把 P1 目标定义为**玩家动作在对方客户端可见的非语言沟通**，并给出双客户端手动验收（`docs/finished_plans/plan-pvp-encounter-v1:118-144`）。
2. 实装侧只写了 `SilentSignalSystem.detect()` 的纯检测逻辑（`client/.../SilentSignalSystem.java:20-53`）。
3. 该文件没有配套 bootstrap；`BongClient` 注册表里也没有任何 related `register()/bootstrap()` 入口（`client/.../BongClient.java:136-145`）。
4. 主代码树里没有 `SilentSignalSystem.detect(...)` 调用、没有 `new SilentSignalSystem.ActionSnapshot(...)`、没有 `SignalKind` 消费者；也找不到 HUD / overlay / toast / store 层对这些信号的展示桥。
5. 因此六类信号全部停留在“测试可构造输入，纯函数可返回枚举”的阶段，**永远不会在正式客户端 tick/render 流里运行**。

## 证据摘录

- **归档 plan 的交付承诺**：`docs/finished_plans/plan-pvp-encounter-v1:118-126`
  - 明写“以下玩家动作在对方 15 格内可见，作为非语言沟通”。
  - 火把和平信号甚至要求“对方 HUD 显示对方手持火把 icon”。
- **实际实现只有纯检测器**：`client/src/main/java/com/bong/client/social/SilentSignalSystem.java:20-53`
  - 仅根据 `ActionSnapshot` 返回 `SilentSignal` 列表。
- **主客户端没有注册入口**：`client/src/main/java/com/bong/client/BongClient.java:136-145`
  - 已注册 `SpiritNicheRevealBootstrap`、`SparringInviteScreenBootstrap`、`TradeOfferScreenBootstrap`；
  - 没有 `SilentSignal...Bootstrap`、`SilentSignalHud`、`SilentSignalOverlay` 之类入口。
- **单测是唯一调用方**：`client/src/test/java/com/bong/client/social/SilentSignalSystemTest.java:13-118`
  - 所有有效输入都由测试手工构造 `ActionSnapshot`；
  - 这证明检测逻辑存在，但也反向证明 runtime 输入源尚未接上。
- **额外 grep 结论（本轮静态取证）**
  - `rg -n "SilentSignalSystem\\.detect\\(|new SilentSignalSystem\\.ActionSnapshot\\(|SignalKind\\." client/src/main/java -g '!**/test/**'`
  - 结果只命中 `SilentSignalSystem.java` 自身定义，无任何主代码消费者。

## 影响面

- `plan-pvp-encounter-v1` 的 P1“沉默博弈”核心体验未真正上线，匿名遭遇缺少设计中的试探层。
- 玩家无法把“切火把 / 丢骨币 / 双蹲 / 指向 / 打坐”稳定识别成系统化社交信号，只能当普通动作或偶然行为去猜。
- 文档与实现出现“已归档完成，但 runtime 无接线”的认知落差，后续若继续在此基础上叠加 betrayal / rumor / NPC 目击推断，容易建立在假前提上。

## 修复建议

1. 新增 client runtime bootstrap，把 silent-signal 采样接到 `ClientTickEvents` 或等价观察循环。
2. 为每个 nearby remote player 生成真实 `ActionSnapshot`：
   - 距离；
   - 当前手持物；
   - 最近掉落物事件；
   - 面向关系；
   - 双蹲/指向/打坐的本地时序状态。
3. 增加 client-side store / dedupe / timeout，避免信号每 tick 抖动或永久残留。
4. 增加 HUD/overlay 消费层，至少满足归档 plan 已写死的“对方 HUD 显示火把 icon”等最小展示要求。
5. 补一条集成级回归：
   - 不是只测 `detect()` 纯函数；
   - 而是测“runtime 采样 → detect → store/HUD”整链有真实 callsite。

## 反方裁决

> 当前会话未提供可用的 subagent / delegate 能力；本轮按**退化处理**执行两轮反方裁决，由当前 agent 独立完成并把反方论点与驳回理由写死在 skeleton 中。

### 反方裁决第 1 轮

- **反方论点**：这也许不是 bug；plan 只是要求“不要给规则解释文字”，并不一定要求真的做额外 UI。
- **驳回理由**：不加“规则解释文字”不等于“完全没有运行时可见层”。`docs/finished_plans/plan-pvp-encounter-v1:120` 明确写了“对方 HUD 显示对方手持火把 icon”，而 `:143-144` 还要求双客户端手动验收。当前实现连 detector 入口都没接，更不是“故意极简 UI”，而是**功能根本没上线**。

### 反方裁决第 2 轮

- **反方论点**：也许别的模块通过隐藏 callsite、反射、生成代码或 render hook 间接用了 `SilentSignalSystem`，只是 grep 没那么直观。
- **驳回理由**：主代码树 grep 已把 `detect(...)`、`ActionSnapshot`、`SignalKind` 消费者都扫过；结果除了 `SilentSignalSystem.java` 自身定义外，没有任何 non-test 命中。`BongClient.java:136-145` 的显式 bootstrap 清单里也没有 silent-signal 入口。这不是“调用藏得深”，而是**运行时桥接缺席**。

## 审计备注

- 本条为 **report-only bughunt skeleton**；按任务要求，本轮**不修代码**。
- 本轮只新增这一份 `docs/plans-skeleton/plan-*.md`，不改源码，不改其他 docs。
