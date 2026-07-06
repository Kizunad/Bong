# plan-bughunt-omen-insight-offer-context-clobber-v1（Active）

> **Active（已从 skeleton 升级，待修复验证）**。一句话主题：`insight` 主路径当前同时跑着“本地 contextual fallback”与“agent 回包覆盖”两条 offer 生产链；后者在 `server/src/cultivation/insight_flow.rs:147-173` / `server/src/network/mod.rs:2430-2461` 中把 agent 已经裁好的三轨选项**一律降级为 `fallback_for(trigger_id)` 的无上下文版本**，并重新覆盖 `PendingInsightOffer` 与 S2C `InsightOffer`。影响是：**玩家正常触发顿悟时，先拿到按自身真元色谱/PracticeLog/Quota 生成的机缘，随后又被覆盖成“默认 Mellow + 空 PracticeLog + Realm::Induce”模板；顿悟从“看你是谁”退化成“大家都看同一套默认稿”**。

> 立项动机：这不是 r7 的 `InsightModifiers` 消费断链，也不是 client `InsightOfferScreen` 生命周期问题；它发生在**选项生成阶段**，直接破坏 `plan-insight-alignment-v1` 已定下的“按玩家当前真元向量动态生成三轨选项”主承诺（`docs/finished_plans/plan-insight-alignment-v1.md:1-3`）。因为顿悟是玩家长期 build 分岔口，错误选项会实质影响成长路线与代价判断，值得先立 skeleton-only plan 固化证据和修复边界。

## 阶段总览

| 阶段 | 主题 | 路由 | 状态 |
|------|------|------|------|
| P0 | insight offer contextual 三轨被 agent 回包静默覆盖降级 | fix_pr | ⬜ |

## P0 — insight offer contextual 三轨被 agent 回包静默覆盖降级

- **设计承诺**：`plan-insight-alignment-v1` 明确要求“按玩家当前真元向量动态生成三轨选项”（`docs/finished_plans/plan-insight-alignment-v1.md:1-3`）。server 侧真实实现也分成两层：
  - `process_insight_request` 用 `fallback_for_context(&req.trigger_id, qi_color, practice_log, quota, req.realm)` 读取**玩家当前** `QiColor` / `PracticeLog` / `InsightQuota` / `Realm` 生成选项（`server/src/cultivation/insight_flow.rs:176-217`）。
  - `select_aligned_choices` 会按 `qi_color.main` 与 `PracticeLog` 计算 converge / neutral / diverge，并把 diverge 目标色绑定到“当前最弱色”上（`server/src/cultivation/color_affinity.rs:47-118`）。
- **覆盖降级点**：agent 回包进入 `ingest_agent_insight_offer` 时，没有沿用当前玩家上下文，而是直接调用 `fallback_for(trigger_id)`（`server/src/cultivation/insight_flow.rs:147-173`）。`fallback_for` 的定义是**固定**拿 `QiColor::default()`、`PracticeLog::default()`、`InsightQuota::default()`、`Realm::Induce` 生成三轨（`server/src/cultivation/insight_fallback.rs:28-36`）；而 `QiColor::default().main == ColorKind::Mellow`（`server/src/cultivation/components.rs:343-351`）。
- **为什么这条覆盖链是 live 的，不是死代码**：server 当前同时注册了 `process_insight_request`（`server/src/cultivation/mod.rs:504-516`）和 `publish_insight_requests` / `emit_cultivation_insight_offers`（`server/src/network/mod.rs:514-557`）。`plan-exploration-probe-return-v1` 还明确把 `InsightOffer` producer 写成“两路 producer（fallback + agent-fed）共用同一事件队列”（`docs/finished_plans/plan-exploration-probe-return-v1.md:127-137`）。
- **实际覆盖动作**：Redis 收到 `InsightOffer` 后，`network/mod.rs:2445-2461` 先调 `ingest_agent_insight_offer(...)`，再把返回 choices **重新写回** `PendingInsightOffer` 并 `insight_offers.send(...)`。这意味着 player 已经看到或即将看到的 contextual 机缘，会被第二次 S2C 包替换成默认模板。
- **这不是“agent 失败才 fallback”的预期**：agent runtime 自己的契约是“只有 LLM 失败 / Arbiter 拒绝 / 空 choices 才交由 server fallback 池兜底”（`agent/packages/tiandao/src/insight-runtime.ts:1-11`，`skills/insight.md:44-52`）。正常路径里它会先按 `available_categories` / `global_caps` / alignment 唯一性裁掉非法项，再发布合法三选一（`agent/packages/tiandao/src/insight-runtime.ts:71-103`）。server 现在却**无差别忽略所有非空 agent 结果**，与 runtime 契约正面冲突。
- **玩家可感知后果**：
  - 1. 已修成锋锐/厚重/杂色/混元的角色，顿悟文案与 target color 本应跟随当前 build；现会在 agent 回包后退回“默认醇色模板”。测试已固定证明 `fallback_for_context` 在 `Sharp` 主色下会产出含“锋锐”的 flavor（`server/src/cultivation/insight_fallback.rs:195-209`），而覆盖路径改用 `fallback_for` 后这一 build-specific 信息消失。
  - 2. diverge 槽本应依据 `PracticeLog` 指向“当前最陌生的色”，帮助玩家转型；现因为覆盖路径喂的是空 log，转向目标退化为默认排序，不再反映玩家真实修炼史。
  - 3. 覆盖路径把 `Realm` 和 `InsightQuota` 也重置为 `Induce + default quota`，高境界/已接近 cap 的角色会看到与自身阶段不匹配的选项过滤结果。顿悟是长期成长分叉口，这种错配会直接影响玩家对“该继续专精还是转型”的判断。
- **建议修复范围 / 模块**：优先收口 `server/src/cultivation/insight_flow.rs` 与 `server/src/network/mod.rs`。方向上至少要保证 agent-fed 路径与本地 fallback 路径共享**同一份玩家上下文**：要么在 ingest 时 query 当前实体的 `QiColor/PracticeLog/InsightQuota/Realm` 并调用 `fallback_for_context`；要么扩 schema 把 effect 参数补齐，真正消费 agent choices，而不是把它们整体丢掉。
- **验收抓手**：至少补 4 组 pin。1) 同一 `InsightRequest`，本地 fallback 路径与 agent-fed 路径在相同上下文下产出的 alignment/target_color/flavor 主色词一致。2) `Sharp` / `Hunyuan` / `Chaotic` 三类角色走 agent-fed 回包后，不得退化成 `Mellow` 默认模板。3) agent runtime 发布合法 3 choices 时，server 不得无条件改写为 `fallback_for()` 结果。4) 双 producer 同帧时，最终 `PendingInsightOffer` 必须保持 contextual 语义，而不是默认上下文。

## 两轮对抗裁决

1. **Round 1（怀疑方）**：反方主张“schema 缺 `effect_params`，server 不能无损重建 agent choice，所以用 `fallback_for(trigger_id)` 是设计使然，不算 bug”。裁决：该辩护只能解释“为什么不直接应用 agent 的 `effect_kind`”，解释不了“为什么不沿用当前玩家上下文”。当前代码明明已有 `fallback_for_context`，且本地路径已经拿到了 `QiColor/PracticeLog/Quota/Realm`；问题不是“无损重建不了 effect”，而是**静默把 contextual fallback 降级回默认 fallback**。
2. **Round 2（反方）**：反方改口主张“也许 agent runtime 本来就只用于触发 server fallback，server 忽略 agent choices 属预期”。裁决：被 `insight-runtime.ts` / `skills/insight.md` 直接证伪。runtime 只在失败/空 choices 时才期望 server fallback（`insight-runtime.ts:10-11`，`skills/insight.md:51-52`）；正常路径先做 Arbiter，再发布合法 3 选项（`insight-runtime.ts:71-103`）。因此 server 现状是**违背上游契约**，不是“按设计兜底”。
3. **结论**：两轮对抗后，候选仍成立，且玩家影响明确、代码点位集中、与既有 bughunt 主题不重复。它属于 `insight` 主路径的 runtime contract / context clobber bug，不是文档漂移，也不是纯美术/占位缺口。

## 开放问题

1. 修复时是否只把 agent-fed fallback 从 `fallback_for` 改成 `fallback_for_context`，还是顺手推进 `InsightChoiceV1` 扩 schema，让 server 真正消费 agent 生成的 effect 参数？前者是局部止血，后者才是契约彻底闭环。
2. 双 producer 并存是否还需要保留“agent 回包后二次 S2C 覆盖”这一步，还是应在本地 fallback 与 agent runtime 之间明确主从/超时策略，避免玩家先后看到两版机缘面板？

## 审计来源

bughunt AP 定点轮（限定 `omen/insight` 主路径：`server/src/cultivation/*insight*`、`server/src/network/*insight*`、`client/src/main/java/com/bong/client/insight/` 周边契约）。结论由主代理实地 Read/Grep 复核形成；当前 harness 未暴露 `multi_agent_v1.spawn_agent`，且本机 `codex` 子会话 provider reachability 失败，故两轮对抗裁决以可审计代码证据链手工固化在本 skeleton 中，未伪造 subagent 结果。
