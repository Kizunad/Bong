# BugHunt: 绝脉断链标记误显为增幅 HUD

**状态**：Skeleton Plan
**日期**：2026-07-07
**分区**：client-combat
**严重度**：Medium（HUD 误导；不改变 gameplay，不是 60s 窗口真的激活）

## 一句话

低境或其它 `grants_amplification=false` 的 `zhenmai.sever_chain` 主动断脉路径只应反馈“断脉无应”，但 client 把 server 的断脉标记 S2C 当作增幅窗口，短暂显示“断链增幅”和倒计时条。

## 实际游玩体验影响

玩家在通灵以下使用绝脉断链时，会付出断一条经脉和真元代价，但设计语义是“没有反震 x3 加成”。当前 HUD 会在约 2 秒内显示 `✕<经脉> 断链增幅` 和增幅条，玩家会误判自己已经拿到高境绝脉断链收益，战斗中可能错误选择硬吃攻击或继续进攻。

这不是服务端真的插入 `BackfireAmplification`，也不是 client 显示完整 60s 窗口；问题边界是短暂但关键的 combat HUD false positive。

## 复现路径

1. 准备一个通灵以下角色，例如 `Realm::Condense`，配置 `zhenmai.sever_chain` 的经脉与 `backfire_kind`，保证有 50 qi。
2. 施放 `zhenmai.sever_chain`。
3. server `resolve_sever_chain` 会断掉目标经脉并发送 `MeridianSeveredVoluntaryEvent { grants_amplification:false }`，但不会发送 `BackfireAmplificationActiveEvent`。
4. `publish_zhenmai_skill_events` 仍向 client 发送 `zhenmai_hud`：`skill_id="sever_chain"`, `k_drain=0`, `duration_ms=0`。
5. client `ZhenmaiHudServerDataHandler` 对 `sever_chain` 一律 `readDuration`，把 `duration_ms=0` 回退为 2000ms，并写入 `ZhenmaiHudStateStore.setSever`。
6. `ZhenmaiHudPlanner.appendSever` 固定渲染“断链增幅”和倒计时条，低境玩家看到错误收益反馈。

## 根因证据

- 设计语义：`docs/finished_plans/plan-zhenmai-v2.md:102-105` 写明通灵以下 cast 仍 SEVERED 但没有反震 x3，HUD 应为“断脉无应”；`docs/finished_plans/plan-zhenmai-v2.md:235-243` 的表格同样把低境收益写为“无加成，仅 SEVERED / HUD「断脉无应」”。
- 可达性：`server/src/combat/zhenmai_v2.rs:430-449` 的 `sever_chain_profile` 在非 `Spirit`/`Void` 分支返回 `grants_amplification:false`；`server/src/combat/zhenmai_v2.rs:831-856` 只在 `profile.grants_amplification` 为 true 时插入 `BackfireAmplification` 并发送 `BackfireAmplificationActiveEvent`。
- 现有单测：`server/src/combat/zhenmai_v2.rs:1987-2005` 已 pin 住低境 `resolve_sever_chain` 能 `Started`、真元扣到 0、但没有 `BackfireAmplification`。
- S2C 混淆点：`server/src/network/zhenmai_v2_event_bridge.rs:138-163` 对 voluntary sever 发送 `skill_id="sever_chain"`, `k_drain=0`, `duration_ms=0`，注释明确它是“断脉标记；增幅窗口在 amplification 事件携 k_drain+duration”；`server/src/network/zhenmai_v2_event_bridge.rs:166-196` 才是实际增幅窗口 S2C。
- client 误解释：`client/src/main/java/com/bong/client/combat/handler/ZhenmaiHudServerDataHandler.java:72-77` 对所有 `sever_chain` 都走 `setSever`；`client/src/main/java/com/bong/client/combat/handler/ZhenmaiHudServerDataHandler.java:85-92` 把 `duration_ms<=0` 回退到 `DEFAULT_DURATION_MS=2000`。
- HUD 文案固定：`client/src/main/java/com/bong/client/hud/ZhenmaiHudPlanner.java:115-128` 无视 `k_drain`/outcome，固定显示 `断链增幅` 和倒计时条。
- 协议注意点：`server/src/schema/server_data.rs:841-850` 当前把 `duration_ms=0` 描述为 client 使用默认 duration。因此本 bug 不应泛化为“所有 0 duration 错误”，而应定位为 `sever_chain` 缺少 outcome 区分，导致 marker 被渲染成 amplification。

## 已避开重复主题

已检查 `gh pr list --state all --limit 250 --json number,title,headRefName,url`。昨天 #969-#1046 的 client-combat 主题包括 #987 技能配置拒绝缺少施法同步、#997 毒蛊 v2 HUD 招式区分断链、#1012 `vfx_event slash` 契约漂移、#1018 蜕壳主动施放视听双源、#1027 技能栏施法源漂移、#1033 爆脉 v3 视听双源发射、#1038 涡流共振循环姿态衰减、#1045 丹道三基础招技能栏断链；均不覆盖 `zhenmai.sever_chain` runtime sever marker 与 amplification HUD 混淆。已有 `docs/plans-skeleton/plan-bughunt-skillconfig-castsync.md` 是缺配置拒绝时 CastSync 不纠正，也不是本问题。

## 修复计划骨架

- [ ] 在 `ZhenmaiHudV1` 增加 `grants_amplification` 或更明确的 `sever_outcome` 字段，区分 `marker_no_amplification` 与 `amplification_active`。若选择最小修复，client 至少要对 `sever_chain && k_drain<=0 && duration_ms<=0` 特判为 no-amplification marker，不得走增幅 slot 的 duration fallback。
- [ ] server bridge 对 `MeridianSeveredVoluntaryEvent { grants_amplification:false }` 发送低境结果语义；对 `grants_amplification:true` 的前置 marker 要避免短暂覆盖后续 amplification HUD。
- [ ] client handler 分出 no-amplification 状态：显示“断脉无应”或等价断脉确认，不显示“断链增幅”和增幅倒计时条。
- [ ] 保持真实 amplification 路径：`k_drain>0,duration_ms=60000` 仍进入增幅 slot，并显示“断链增幅”与倒计时条。
- [ ] 不改 server gameplay：低境仍能断脉、仍扣代价、仍不插入 `BackfireAmplification`。

## 验证计划

- [ ] server：新增/补强 `Realm::Awaken` 或 `Realm::Condense` 的 `resolve_sever_chain` 事件断言，确认发送 `MeridianSeveredVoluntaryEvent { grants_amplification:false }` 且不发送 `BackfireAmplificationActiveEvent`。
- [ ] bridge：新增 `grants_amplification=false` 的 voluntary sever S2C 测试，断言 payload 为 `skill_id="sever_chain"`, `k_drain=0`, `duration_ms=0`，并明确这是断脉结果标记，不是增幅窗口。
- [ ] client handler：给 `ZhenmaiHudServerDataHandler` 加 pin 测试，接收 `k_drain=0,duration_ms=0` 后不得通过 `readDuration` fallback 进入 2000ms amplification slot。
- [ ] client planner：低境/no-amplification 状态显示“断脉无应”或等价反馈，不出现“断链增幅”和增幅倒计时条。
- [ ] 回归：`k_drain>0,duration_ms=60000` 的真实 amplification 仍显示“断链增幅”与倒计时条。
- [ ] 回归：高境 dual-emit 最终 HUD 是 amplification，不被前置 voluntary marker 覆盖成“断脉无应”。
- [ ] client gate：按 AGENTS 三栈命令矩阵，在 `client/` 跑 `./gradlew test build`；若涉及 schema/dist，再按实际修改栈补充对应命令。

## 对抗复核结论

**候选证据**：server 把 voluntary sever 标记和 amplification active 作为两类事件发送，低境 profile 可达 `grants_amplification=false`，client 却把所有 `sever_chain` 渲染成增幅 HUD。

**反方质疑第一轮**：`server_data.rs` 允许 `duration_ms=0` 回退默认时长，voluntary sever 的 2 秒展示可能只是成功确认；并且 client 只显示约 2 秒短条，不应夸成 60s 增幅窗口或 gameplay 激活。还需补证低境路径真实可达、且不与 #987 等现有 BugHunt 重复。

**修正/反驳第二轮**：接受严重性修正，定位为短暂 HUD false positive；不攻击通用 `duration_ms=0`，只指出 `sever_chain` 缺 outcome 区分。finished plan 明确低境 HUD 应为“断脉无应”，server profile 与单测证明低境断脉无增幅可达，现有 PR/skeleton 没覆盖该 runtime HUD 混淆。

**反方最终裁决**：通过。可作为 BugHunt skeleton PR；剩余风险是修复时不能让高境 true 路径先闪“断脉无应”，验证必须覆盖 handler 收到 `k_drain=0,duration_ms=0` 后不进入增幅层，以及真实 `k_drain>0,duration_ms=60000` amplification 不回归。
