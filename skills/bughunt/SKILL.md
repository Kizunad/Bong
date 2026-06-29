---
name: bughunt
description: 自治代码库 bug 狩猎一轮——4 sonnet finder 并发扫可达 gameplay bug → sonnet skeptic 对抗证伪 → worktree 隔离 fix+测试 → opus 对抗 verify。产出 fix 分支+verdict+findings。用法：/bughunt [round-id]，或"跑一轮 bughunt"。长跑用 /loop 自驱并盯 5h。
argument-hint: [round-id 如 20260630-r1]
allowed-tools: Bash Read Edit Write Workflow Grep Glob
---

# Bong 自治 bug-hunt

一轮 = 一次 Workflow（`wf-bughunt-round.mjs`，本目录），四阶段 pipeline：

| 阶段 | 模型 | 干什么 |
|------|------|--------|
| **Find** | sonnet ×4 并发 | 4 维度（qi_sweep / skill_gate / npc_combat / craft_econ）扫**可达** gameplay bug，每维≤3 个，必带 `file:line`+读码证据+fix sketch |
| **Debate** | sonnet skeptic | 对抗证伪：真 bug？周围代码已处理？玩家正常游玩可达（非 dev-only）？默认怀疑 |
| **Fix** | sonnet（worktree 隔离） | 断点最小修复 + 锁行为测试 + push 分支 |
| **Verify** | **opus**（≤3 并发） | 代替 Pi review，只读 diff 对抗验证，verdict = ship/needs_work/reject |

设计哲学：**真清洁的当场修+PR；涉设计抉择/跨模块/孤岛接线的 → report-only**（写进 findings 交人工/立 plan）。一个 PR 只收能稳稳锁住的修复。

---

## 跑一轮（标准流程）

### 0. 跑前两查（MANDATORY，"注意 5h" 的硬闸门）

```bash
bash ~/.claude/quota.sh        # 看 5h%（别用 ccusage，错的）
df -h /                        # 盘 >90% 先清 worktree build 缓存
```

- **5h governor**：一轮 ~8-12 个 subagent（含 worktree 冷构建）≈ **吃 15-50% 的 5h 窗口**。
  - 5h **< 75%** → 跑。
  - 5h **≥ 75%** → **停**，别开新轮；ScheduleWakeup 到 5h reset 时刻后再续（reset 时刻见 quota.sh 输出）。
  - 接近 **95% 硬停**。用户令"只看 5h，不管 7d"——但 7d 满（~100%）会强制全锁，撞到也得停。
- **磁盘**：盘 >90% 先 `rm -rf .worktree/*/server/target`（纯 build 缓存可再生，无源码损失）。⚠️ 别盲删带未提交工作的 worktree（外部 orchestrator 可能有活，先 `ls .worktree/` + `git -C <wt> status` 看）。

### 1. 起 Workflow

round-id 用 `YYYYMMDD-rN`（避免和老 `bughunt-r10-*` 分支撞）。脚本读 `args.round`（带 args-string 守卫）：

```
Workflow({ scriptPath: "<repo>/skills/bughunt/wf-bughunt-round.mjs", args: { round: "20260630-r1" } })
```

后台跑，完成时发 `<task-notification>`。**发完顺手 ScheduleWakeup ~1500s 当 fallback 心跳**（真正唤醒信号是 workflow 完成通知；防长等待掉 cache/session 冷）。

### 2. 收口 triage（workflow 返回后，主循环亲自做——别盲信 verdict）

workflow 返回 `{ round, found, confirmed, fixes:[{id,branch,verify,...}], skipped }`。逐条：

- **verify.verdict == ship 且 regression_risk ∈ {none,low} 且有真测试** → 候选 PR。但**落地前铁律**（见下）必须先过。
- **verdict == needs_work / reject** → 不开 PR，把 finding 写进 findings 日志（report-only）。
- **skipped / 设计抉择 / 孤岛需定 consumer 语义 / worldview 漂移** → report-only，附 `file:line`。

### 3. 落地铁律（opus verify 会漏，这几关不能省）

1. **fix-now 必主循环亲自读码复核全链路**——workflow synthesis 读不够深会误判方向（藏设计抉择/Bevy 16 参数上限/孤岛）。守恒/scorer 类尤其。
2. **本地必跑 `cargo test --lib`（全量）+ 连跑 3× 验 flaky**——绝不只信 opus verdict。`cargo test -- A B C`（多 filter 要 `--`，否则 "unexpected argument"）。
3. **守恒类 fix 必查 `mod.rs` 注册**——多次发现整 fix 是生产 no-op 死代码（system 没注册，单测 add_systems 掩盖孤岛）。
4. **合并前必查 CodeRabbit actionable**——它多次抓出 opus 全漏的 Critical（`.max(0.0)` 负灵域守恒、over-credit qi_current<damage）。每 PR 评论 `/review` 触发 Pi，等 CodeRabbit + Pi 都无阻塞再合（CodeRabbit summary-only "fail"=无 actionable，非阻塞）。
5. 自己开的 PR 自己盯到 merge，别甩回用户。

### 4. 推进下一轮

- 把本轮已修模块**追加进脚本的 `ALREADY_FIXED_QI` / 新 dedup**，让下轮 finder 不复报。
- round-id 递增（`-r2`、`-r3`…）。
- **NOT_REAL 比例升高 = 浅层枯竭信号** → 换角度回血：subsystem-sweep → mechanism（race/NaN/panic/clock-restart）→ semantic/authorization/worldview-drift。改脚本 `DIMENSIONS[].focus` 即可。

---

## 模型路由（[[feedback_workflow_model_routing]]）

- finder / debate / fix = **sonnet**；verify = **opus**。
- **opus 并发 ≤3**（用户硬令）——脚本 Verify 阶段 toFix 已 slice 到 3，满足。别擅自把 finder/fix 提到 opus。

## 工作流经济学（实证）

- haiku skeptic 是废过滤（refuted 0）→ 用 sonnet+强反驳指令。
- sonnet adjudicator 橡皮章（confirm 97%）；opus 才有鉴别力（~50%）但贵会爆 5h。
- ~8-finder 一轮 ≈ 吃 50% 5h 窗 → **一个 5h 窗只能跑 1-2 轮**，跑完等 reset。
- **worktree 堆爆盘**：~90 worktree 各带 server/target 累计 504G 撑满 `/` 卡死 workflow。每轮跑前 `df -h`，每轮跑后清自己的 worktree。
- 529 Overloaded：finder 全挂、0 token、误报 "no bug"——非真无 bug，等几十分钟同 scriptPath 重跑。

## 反复命中的系统性主题（dedup 已含，别复报）

写入端齐全消费端断裂（modifier-orphan）· VFX emit-orphan · forge 未接 qi_physics · 经脉门 declare 不统一 · **死系统**（组件从不 insert）· qi 守恒泄漏（扣 qi_current 不归还 zone，修法=直接写 `zone.spirit_qi`，`QiTransfer` 是 audit-only 孤岛）· proto enum 前缀/flat-array world_pos 漂移 · persistence 关服不 flush。

## 方法论铁律

- **必以 fresh origin/main worktree 为 ROOT**——本地 main 常 stale，绝不扫主仓 stale 工作目录（曾把已修 bug 当新发现）。
- 每轮 DEDUP 累积之前全部已确认+已剔除项，否则 finder 重报。
- qi-zone-credit 测试坑（脚本 Fix prompt 已内置）：fallback spawn zone 近满→拆 overflow；测试实体缺 `CurrentDimension`→credit 走 Overflow 非 zone；credit 用**实际扣减量**非请求量；clamp 边界别用 float 相等硬编码整数。

## 关联记忆

[[feedback_bug_hunt_workflow]] [[project_bughunt_findings]] [[project_bughunt_critical_fixes]] [[project_bughunt_r1_20260619]] [[project_bughunt_r2_20260623]] [[project_bughunt_qi_conservation]] [[reference_ccusage_5h_governor]] [[feedback_workflow_model_routing]] [[feedback_workflow_opus_concurrency_cap]] [[feedback_workflow_launch_wakeup]] [[feedback_wait_coderabbit_approve]] [[feedback_own_pr_watch_to_merge]]
