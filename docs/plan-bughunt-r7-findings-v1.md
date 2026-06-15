# plan-bughunt-r7-findings-v1（active）

> **Active（已从 skeleton 升级，待逐项消费）**。一句话主题：代码库自检 bug-hunt **round7**（fresh origin/main worktree ROOT，转深角度：registry 注册健壮性 · 跨子系统多跳集成链 · Bevy 系统调度 · Redis IPC 往返 · client 输入/屏幕生命周期）确认的 **10 个新真 bug**——含**重磅系统发现：InsightModifiers 五字段全断链（顿悟投资的通用天赋 modifier 写入却无任何系统读取 → insight 投资全失效）**。已对 r1-r6 去重，全部 real-on-main。

> 立项动机：round7 转深角度，5 finder → 怀疑者对抗 → opus 逐条**实地 Read/Grep 全树**复核，15 候选 → **10 REAL / 5 NOT_REAL**。本轮最大收获是 **InsightModifiers integration-chain-break 簇**（5 字段写入端齐全、消费端从未接线，与 r4/r6 的 status-effect 孤岛同根因——"modifier 写入 → 消费系统读取"这一跳系统性缺失），外加 client UI 生命周期/输入残留 2 处 + registry/IPC/schema 文档漂移 3 处。

## 阶段总览（按主题分组，逐项独立可修）

| 阶段 | 主题 | 路由 | 状态 |
|------|------|------|------|
| P0 | 🔶 InsightModifiers 五字段全断链（顿悟投资全失效） | plan_skeleton | ⬜ |
| P1 | client UI 生命周期 / 输入残留（关屏泄漏 + 拖拽卡死） | fix_pr | ⬜ |
| P2 | registry/IPC/schema 文档漂移（死代码 + 注释错 + 联合缺项） | fix_pr | ⬜ |

## P0 — 🔶 InsightModifiers 五字段全断链（major 簇，顿悟投资全失效）

> `server/src/cultivation/insight_apply.rs` 把玩家选中的通用天赋（generic_talent.rs）写入 `InsightModifiers` 各字段，但**消费系统从不读取**——全树 grep 这些字段仅命中 struct 声明 / new() 初始化 / 该写入 / dev reset 断言，**零生产读取**。玩家投资顿悟点选这些天赋后**效果完全 no-op**。五字段同根因：「modifier 写入 → 消费系统读取」这一跳从未为它们建立（与 r4 QiCapPermMinus/QiRegenBoost、r6 ContaminationBoost 同类系统性缺口，但**不同字段**，非去重）。

- **#3 major**：`insight_apply.rs:127` 写 `modifiers.qi_regen_mul *= mul`；`tick.rs` query 不含 InsightModifiers，回气乘法链无此因子。`QiRegenFactor` 真实可选（`generic_talent.rs:330` / `cultivation_insight_offer_emit.rs:432` mul:1.1）→ 选后每 tick 丢弃。
- **#4 major**：`insight_apply.rs:159` 写 `next_breakthrough_bonus += add`；`breakthrough.rs` material_bonus 仅 `req.material_bonus + BreakthroughBoost`，无此字段。`NextBreakthroughBonus` 真实可选（`generic_talent.rs:332` / offer_emit add:0.05）→ 突破成功率加成无效。
- **#5 major**：`insight_apply.rs:178` 写 `vortex_backfire_resist_mul *= mul`；`woliu.rs` `check_backfire_resistance` 仅看 AntiSpiritPressurePill 状态。`VortexBackfireResist` 真实可选（offer_emit mul:0.9）→ 反噬抗性无效，仍受全额反噬。
- **#6 major**：`insight_apply.rs:184` 写 `vortex_delta_bonus_add += add`；`woliu.rs` 涡旋 delta 仅取 `vortex_delta_for_realm(realm)` 境界固定值。`VortexDeltaBonus` 真实可选（offer_emit add:0.05；孤立度最高，连 dev reset 断言都无）→ 涡旋强度与未投资者相同。
- **#7 major**：`insight_apply.rs:190` 写 `vortex_flow_speed_mul *= mul`；全仓无任何系统读此字段。`VortexFlowSpeed` 真实可选（offer_emit mul:1.1）→ 涡旋周期加速期望落空，完全 no-op。
- 修：建立 `InsightModifiers` → 各消费系统（tick.rs 回气 / breakthrough.rs 突破 / woliu.rs 反噬·delta·flow）的读取接线；**建议与 r4/r6 status-effect 孤岛簇统一为一个"modifier/effect 消费层接入" plan**（消除"写入端齐全、消费端断裂"的系统性模式）。**需设计统一接入层。**

## P1 — client UI 生命周期 / 输入残留

- **#9 major（fix_pr）**：`client/.../agentui/AgentUiScreen.java` `close()`（232-242，ESC/按钮）与 `closeWithoutResponse()`（247-255，本地超时/server 信号）均只 `AgentUiVfxStore.clear()` + super.close()，**都不调 `AgentUiStore.setActive(null)`**；AgentUiStore 仅在 `receiveClose`（server 端 requestId 匹配，38-43）和 `clear`（断连，46-48）置 null。`AgentUiBootstrap.onEndClientTick`（30-38）每 tick 对 `getActive()` 调 `tickLocalTimeout` → **本地 ESC 关闭后陈旧 closed 屏仍被 store 持有、每 tick 被 tick**；一旦 `currentTick>=localExpireTick`，`tickLocalTimeout` **每 tick 调 closeWithoutResponse() → 每 tick 跑 AgentUiVfxStore.clear()** 无限持续（持续抹除 VFX 可能干扰其他系统 VFX），直到新请求替换引用。closed 守卫只防重复 send 不停 VfxStore.clear。修：close()/closeWithoutResponse() 持有活跃引用时清空 store。**局部明确。**
- **#10 minor（fix_pr）**：`client/.../mixin/MixinMouse.java:100-103` 左键处理 `currentScreen!=null` 时 early return → 拖拽中打开屏幕时该次 LEFT RELEASE 不调 `onLeftButton(0,...)`，`BotanyDragState.dragging`（静态单例）**滞留 true**。reset 路径（maybeResetForSession 按 sessionId / resetForNewSession）屏幕开关均不重置 → 关屏后下个左 PRESS 离 panel 不消费但 dragging 仍 true，随之 RELEASE → `onLeftButton(0)` 命中 `action==0 && dragging` → dragging=false 返 true → 该 RELEASE 被 mixin cancel（117），**吞掉关屏后首个左键 RELEASE**（影响 release 敏感控件/拖动）。前置条件较窄但物理可达。修：开屏/`currentScreen!=null` 时清 dragging 或补发 onLeftButton(0)。**局部明确。**

## P2 — registry/IPC/schema 文档漂移

- **#2 minor（fix_pr）**：`server/src/cultivation/skill_registry.rs` `jiemai::register_skills` 注册的 `zhenmai.parry`（`jiemai.rs:21` → resolve_zhenmai_parry_skill）被 `init_registry`（skill_registry.rs:66 jiemai → 67 zhenmai_v2）下一行 `zhenmai_v2`（`zhenmai_v2.rs:303` resolve_parry）用 `HashMap::insert`（53）**静默覆盖** → jiemai 版**永不被 lookup 命中**（死代码）。jiemai 版还跳过 spend_qi/check_static_meridian_dependencies/record_practice/emit_skill_feedback（zhenmai_v2 版四者俱全），NPC parry 走 DefenseIntent 不经 registry。外部仅引用 jiemai 的 qi_cost/apply_effects（仍活跃），唯独 register_skills+resolve_zhenmai_parry_skill 死。修：删 line 66 jiemai register 调用及随之失活的函数（安全清理）。**局部明确。**
- **#7chat minor（fix_pr）**：`agent/packages/schema/src/channels.ts:9` 注释标 PLAYER_CHAT「(Redis List, RPUSH/BLPOP)」，但全 agent 仓 grep BLPOP **仅此注释**，实际消费 `redis-ipc.ts:891` `drainListAtomically` → `multi().lrange().ltrim()`（批量 drain）。channels.ts 是 IPC 单一事实源，BLPOP（阻塞单消费者 pop）标注会**误导未来第二消费者按文档接 BLPOP 从生产 drain 偷消息**。修：注释改 LRANGE/LTRIM 批量 drain（一行）。**局部明确。**
- **#8 minor（fix_pr）**：`agent/packages/schema/src/payloads/agent-ui.ts:91-94` `AgentUiErrorReasonV1` 联合仅声明 3 种（realm_gate_rejected/invalid_button_id/player_offline），但 server `agent_ui.rs` 实发 **5 种**——漏 `invalid_command`（:353）与 `xml_sanitize_failed`（:422），两者经 `bong:agent_ui_response` 过线。消费者 `uiResponseConsumer.ts:243-262` 有兜底 else（无崩溃），但这是便利联合相对 server 输出的**完整性缺口**。修：联合补 2 字面量（连 sample 对拍，保持与 server 一致）。**局部明确。**

## §N 开放问题

1. **#3-#7 InsightModifiers 簇是否与 r4（QiCapPermMinus/QiRegenBoost）+ r6（ContaminationBoost）合并成一个"modifier/effect 消费层接入"总 plan**——五个 insight 字段 + 三个 status-effect 共 8 处"写入端齐全消费端断裂"，同根因，统一接入层一次修比逐个补更彻底（强烈建议合并，避免再漏）。
2. #9 AgentUiStore 清空时机：close() 内清 vs receiveClose/clear 集中清——需确认本地关闭与 server 关闭两路都覆盖且不竞态。
3. #2 jiemai.parry 删除范围：确认 jiemai::register_skills 是否还注册了其他仍活的 skill（只删 parry 注册 vs 整个 register_skills）。
4. P2 三条文档/schema 漂移（#2/#7chat/#8）可合一个机械 fix PR（与 r1/r3/r4 机械 fix 同性质）。

## 审计来源

bug-hunt round7（workflow，5 深角度 finder + 怀疑者对抗 + opus 逐条全树复核，15 候选）。**ROOT = fresh origin/main worktree**（方法论修正后第五轮）。已对 r1-r6 去重。**report-only**：InsightModifiers 五字段断链是本轮最大系统发现（顿悟投资全失效），建议与 r4/r6 effect 孤岛合并统一接入层；#9 client UI 关屏泄漏次之；P2 三条文档漂移可合机械 fix。**深角度转向有效**：registry/集成链/client 生命周期挖出 10 个全新真 bug，证明浅层枯竭后转深仍有富矿。
