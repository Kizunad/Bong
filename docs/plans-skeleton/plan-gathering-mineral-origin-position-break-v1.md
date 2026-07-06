# plan-gathering-mineral-origin-position-break-v1（骨架）

> **骨架（草案）**。一句话主题：`mineral -> gathering` 桥接把矿块中心坐标写进 `GatheringSession.origin_position`，而 `gathering::enforce_gathering_session_constraints` 又把这个字段当成“玩家起手位置”做移动打断；结果是矿脉采集只要进入正式约束 tick，就会把**站在矿前正常挖掘**误判成“已移动过远”并中断 session。影响是：**矿石通常还能掉出来，但挖矿进度环、循环采集动画、tick 粒子/音效、品质提示会在玩家没动时被系统自己掐掉，采集手感明显断裂**。

> 立项动机：这是 `server/src/mineral/break_handler.rs` 与 `server/src/gathering/session.rs` 的主路径错接，不是边角测试桩。`plan-gathering-ux-v1` 明写“移动 / 被攻击 / 松开右键 → 进度归零”，语义是**玩家离开起手点**才打断；当前矿脉桥却把“矿块中心”塞给同一个字段，导致约束系统稳定拿错参照物。

## 阶段总览

| 阶段 | 主题 | 路由 | 状态 |
|------|------|------|------|
| P0 | 矿脉采集 `origin_position` 误接导致移动打断恒成立 | fix_pr | ⬜ |

## P0 — 矿脉采集 `origin_position` 误接导致移动打断恒成立

- **现象**：`server/src/mineral/break_handler.rs:99-118` 的 `mining_gathering_session()` 创建 `GatheringSessionStart` 时，把 `origin_position` 设成 `mining_origin_position(context.pos)`；`mining_origin_position()` 本身就是矿块中心 `[x+0.5, y+0.5, z+0.5]`（`break_handler.rs:85-87`）。但 `server/src/gathering/session.rs:165-170` 的 `moved_too_far()` 会把 `origin_position` 当成“玩家不应离开的参考点”，直接和 `Query<&Position, With<Client>>` 取到的**玩家当前位置**做距离比较；阈值还只有 `0.5 * 0.5`（`session.rs:18`）。
- **为什么这是 bug，不是设计**：`docs/finished_plans/plan-gathering-ux-v1.md:90-112` 对 `GatheringSession` 的契约写得很清楚：打断条件是“移动 / 被攻击 / 松开右键”，验收手动项也是“移动 → 中断”。这里的“移动”只能是**玩家离开起手采集位置**，不可能是“玩家和目标矿块中心必须重合在 0.5 格内”，否则矿石/树木这类天然隔着一格采集的目标根本不可能稳定维持 session。
- **主证据链**：
  - `gathering::enforce_gathering_session_constraints()`（`server/src/gathering/session.rs:220-249`）每个 Produce tick 都会扫 `store.iter()`，命中 `session.moved_too_far(position)` 就移除 session，并发一帧 `interrupted=true` 的 `GatheringProgressFrame`。
  - 矿脉桥创建 session 时没有记录玩家起手位置，只记录了矿块中心（`server/src/mineral/break_handler.rs:108-115`）。
  - `server/src/gathering/session.rs:351-362` 的单测 pin 住了当前阈值：偏移到 `[0.6, 64.0, 0.0]` 就算“移动过远”。这意味着只要玩家脚下坐标与目标矿中心差出大约半格以上，就会被判中断；而正常挖矿站位通常至少与矿块隔着一格，稳定超过阈值。
- **为什么玩家仍能拿到矿，但体验仍然坏**：`server/src/mineral/break_handler.rs:121-136` 的 `mining_completion()` 会在 Stop 时尝试 `store.remove(player)`；如果前面的错误约束已经把 session 删掉，它会**伪造一个“已经挖满 total_ticks”的 fallback session** 来继续发完成帧和掉落。所以结果不是“完全挖不到矿”，而是**矿会掉、但持续进度 session 已经提前被误中断**。这会把 HUD / 动画 / tick VFX/SFX / 品质提示从真实挖掘动作上撕开。
- **对实际游玩体验的影响**：玩家站在矿前长按挖掘时，会看到 0% 开始帧发出后，进度环和采集动作很快被系统自己判成“中断”；如果继续正常挖完，矿仍会掉出来，于是体感变成“明明没动，HUD 和动作先断了，最后矿又出来了”。这会让矿脉采集显得不可信，尤其伤到 `plan-gathering-ux-v1` 专门做的进度环、循环动作、tick 粒子/音效、品质 hint 这整套主观反馈。
- **建议修复范围 / 模块**：优先收口 `server/src/mineral/break_handler.rs` 与 `server/src/gathering/session.rs`。方向应二选一并统一语义：要么把 `GatheringSession.origin_position` 改成明确的“玩家起手位置”，矿脉桥/灵木桥/草药桥都传同一种坐标；要么把“目标展示/VFX 原点”和“移动打断参考点”拆成两个字段，避免继续一字段双重含义。无论选哪条，**都不能再让移动约束复用矿块中心**。
- **验收抓手**：
  1. 矿脉 Survival Start 后，玩家在原地持续挖掘到 Stop 之前，`GatheringSessionStore` 不应因为静止站桩而被 `moved_too_far()` 提前清掉。
  2. 同一条挖矿链路中，`gathering_session` / `mining_progress` / `GatheringProgressHud` 应持续显示到完成或真实打断，而不是“先 interrupted、后 completed”自相矛盾。
  3. 真实移动离开起手点、受击、Abort 三种场景仍必须触发中断，不能因为修正参照物而放松掉原本的打断语义。
  4. 矿脉进度环、循环动画、tick 粒子/音效、品质 hint 的回归要按“站桩可持续、移动才中断”验收。

## 反方裁决摘要

1. **Round 1（怀疑“这只是 VFX 原点，不影响真实 session”）**：反方认为 `origin_position` 也许只是给粒子/音效/动画找落点，误用矿块中心不一定构成玩法 bug。复核后驳回：`server/src/gathering/session.rs:165-170` 明确把同一字段用于 `moved_too_far()`，而 `enforce_gathering_session_constraints()` 又在正式 Update 主链里按这个判断删 session；它不是单纯表现层字段。
2. **Round 2（怀疑“即便参照物错了，也许矿工站位仍落在阈值内，或系统顺序能绕过”）**：反方怀疑玩家 Position 与矿块中心可能足够接近，或 break/start 与约束系统顺序让 session 实际能活到完成。复核后驳回：阈值被单测 pin 在 0.5 格半径（`session.rs:351-362`），而正常挖矿是对着相邻方块操作，脚下坐标与矿心天然拉开；即便调度顺序让 Start 当帧侥幸存活，下一次约束 tick 仍会对着同一个错误参考点重判。`mining_completion()` 的 fallback 还能解释“矿照掉、session 已死”的表象，因此这不是理论边缘案，而是稳定可达的主路径错接。

## 开放问题

1. `GatheringSession.origin_position` 是否已经在草药 / 灵木 / 其他采集桥里同时承担了“表现原点”和“移动打断原点”两种语义？若是，修复 PR 最好顺手把字段语义拆开，避免矿脉修完后别的采集路径继续带雷。
2. 既然 `mining_completion()` 允许 session 缺失时 fallback 完成，是否还需要补一个 pin，专门防“静止站桩时先 interrupted 后 completed”的矛盾事件序列再次出现？

## 审计来源

bug-hunt 线程 S，范围限定 `server/src/economy/`、`server/src/gathering/`、`server/src/mineral/` 与必要 client gathering/mineral 消费链；候选经主代理代码级复核后保留。当前结论是 **report-only**：先提交 skeleton plan 固化玩家影响、根因路径、两轮反方裁决和验收面，再由后续 fix PR 单独修正 `origin_position` 语义与矿脉桥接。
