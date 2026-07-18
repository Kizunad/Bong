# plan-dazuo-v1 — 打坐/主动修炼底盘：静坐吸灵 + 练功会话接活

> **一句话主题**：修炼当前是**纯被动挂机**——`cultivation/tick.rs` 文件头自注「P1 简化：无『静坐/行动』区分，全部按被动小系数回；静坐/打坐在 P1 末加客户端指令时再接入」，这笔欠账一直没还。本 plan 落地**玩家主动打坐**（长按触发姿态 + 吸灵提速 + 移动/受击打断），并顺带接活 `practice_session` 练功孤儿模块（reminder.md 登记的专门 plan 待办，本 plan 认领）——把修仙题材的核心动词从"等"变成"做"。
>
> 来源：2026-07-18 早期玩法诊断（三根因之一：前 30 分钟无主动修炼动作）+ journey §L:752 明写的设计意图「10:00 第一次右键长按打坐，真元缓涨——"要等的、不能一直跑"」+ worldview §三 修炼主流程「静坐攒真元」。

**状态**：骨架（skeleton）。升 active 前按 docs/CLAUDE.md §五收口 §8。

| 阶段 | 主题 | 状态 |
|------|------|------|
| P0 | server 打坐状态机 + 守恒吸灵提速 + 教程钩子归位 | ⬜ |
| P1 | client 触发手势 + 盘坐姿态动画 + 吸灵 A/V + HUD | ⬜ |
| P2 | practice_session 练功会话接活（熟练度 + qi 守恒还账） | ⬜ |
| P3 | 数值收口 + bot e2e + 与加速器 Clamp 联动核验 | ⬜ |

## 现状证据（2026-07-18 Explore 实证）

- `cultivation/tick.rs:1-8`：吸灵与 zone 扣减合并 system 保零和（✅ 守恒地基好），但顶注明写「P1 简化」，无静坐/行动区分。
- `world/spawn_tutorial.rs:693`：`TutorialHook::FirstSitMeditate` 在 `qi_current > 0` 时**自动触发**——钩子名承诺的"第一次打坐"动作根本不存在，名不副实。
- `cultivation/practice_session.rs:68-101`：`practice_session_tick` / `check_practice_session_exit` 均 `#[allow(dead_code)]`，mod.rs 仅 `pub mod` 无 ECS 注册；且 `:78` `*current_qi -= cost` **无 zone credit、不走 ledger**（守恒律红旗，reminder.md 2026-06-10 已登记）。
- 蒲团先例：`plan-furniture-buff-v1` 已用守恒安全的 `CultivationAcceleration` 兜住 +20% 修炼速度——打坐倍率与其叠加规则须一并定义。

## 接入面（docs/CLAUDE.md §二 checklist）

- **进料**：`cultivation::tick`（吸灵主循环，**只调系数不另起吸灵路径**）；`ZoneRegistry.spirit_qi`；`practice_session.rs` 全部既有类型（`PracticeSession`/`PracticeSessionStarted/Ended`/`PracticeSessionTracker`，只接线不重造）；`technique_proficiency::{practice_session_gain, practice_session_qi_cost_per_tick, should_exit_practice_session}`（公式已备）；`CultivationAcceleration`（蒲团）。
- **出料**：打坐期吸灵倍率进 tick 主循环（同一 `regen_from_zone` + `QiTransfer` 路径）；`TutorialHook::FirstSitMeditate` 改由真打坐触发（喂 [[plan-newbie-30min-hooks-audit-v1]] 的 10:00 钩子）；练功产出熟练度进 `technique_proficiency`；打坐/练功状态进 narration + HUD。
- **共享类型 / event**：新增仅限 `DazuoState` component + C2S `ClientRequestV1::DazuoToggle`（或 Start/Stop 对，§8 #2）；练功侧**零新类型**（全在 practice_session.rs 现成）。
- **跨仓库契约**：server `schema/client_request.rs` 新 variant + proto + samples 正反对拍；client `DazuoIntentHandler` + 姿态动画 + HUD 标记；agent 不参与。
- **worldview 锚点**：§三 修炼主流程 L76 起（静坐攒真元是主循环第一环）；journey §L:752（10:00 钩子）；journey K 红线 O.13（打坐无 UI 教学，靠姿态与粒子自明）。
- **qi_physics 锚点**（最高优先级约束）：
  - 打坐吸灵**不新增任何吸收路径**——只对 `cultivation::tick` 既有 `regen_from_zone` 系数乘打坐倍率，零和结构原样保留；倍率是玩法参数（const 在 `cultivation` 常量区），**不是物理常数**，不进 qi_physics。
  - practice_session 接活必须还守恒账（reminder.md 原文约束，一字不让）：`*current_qi -= cost` 改走 `qi_release_to_zone` 守恒释放 + 补 `qi_physics::ledger` 断言（zone 增量 = 玩家消耗）；不允许照抄现状 dead_code 直接注册。
  - 新常数红旗自查：本 plan 无 `*_DECAY*`/`*_DRAIN*` 类常数需求；若数值收口时出现"打坐时 qi 逸散减免"类需求 → 必查 qi_physics 先。

## P0 — server 打坐状态机 + 守恒吸灵 + 教程钩子归位 ⬜

- `DazuoState` component（`entered_at_tick`）；进入条件：静止 + 非战斗 + 非濒死；退出条件：移动 / 受击 / 施放 / 显式 Stop（复用 `should_exit_practice_session` 的判定形状）。
- tick 主循环：持 `DazuoState` 的玩家吸灵系数 × `DAZUO_ABSORB_MULT`（初值 3.0，§8 #1 拍板；行动基线系数不动——现状被动速率语义变更为"行动态"）。
- `TutorialHook::FirstSitMeditate` 触发条件改为「首次进入 `DazuoState` 且持续 ≥ 60 tick」——修正 `spawn_tutorial.rs:693` 的误触发。
- narration（player scope / perception style，进入首次 + 长坐节点各一）：「气往下沉，杂念浮上来，又散掉。」/「地脉里那点稀薄的东西，顺着呼吸往里爬。」/ 被打断：「心口一跳，气散了。」
- 测试：进入/退出全分支（移动/受击/施放/Stop/濒死拒入）、倍率生效对拍（打坐 vs 非打坐同 zone 同时长吸灵差）、守恒断言（玩家增量 = zone 减量，取 const 引用不写字面）、教程钩子只触发一次。

## P1 — client 触发 + 姿态 + A/V + HUD ⬜

- 触发手势：潜行 + 空手右键长按 ≥ 10 tick 进入（journey §L「右键长按打坐」），任意移动键退出（§8 #2 定稿）；`DazuoIntentHandler` 走 IntentHandler 模式发 C2S。
- **姿态动画**：PlayerAnimator JSON `dazuo_sit.json`（生成脚本 `client/tools/gen_dazuo_sit.py`）——盘坐：`body` y 位移 -0.55、双 `leg` pitch ≈ 35°（红线内）+ `bend` 后折承担盘叠、`torso` pitch 8°、双臂前置手腕搭膝；`isLooped=true`，**每个用到的 axis 在 endTick 补同值关键帧**（循环单帧衰减库坑）；呼吸起伏用 `torso` pitch ±1.5° 周期 60 tick。三视图用 `client/tools/render_animation.py` 预览定稿。
- **吸灵粒子**：`BongLineParticle`，continuous，每 4 tick 2 条，从半径 2.5 格随机点汇向玩家胸口，lifetime 12 tick，速度向心 0.12，颜色按 zone 浓度插值（贫 `#6B7B6B` → 丰 `#8FD9A8`），复用既有 qi_wisp 贴图；`bong:vfx_event` 新 ID `dazuo_absorb`，`VfxBootstrap` 注册（防孤岛）。
- **SFX**：audio_recipe `dazuo_loop.json`——layer1 `block.beacon.ambient` pitch 0.6 vol 0.2 loop；进入瞬间 `dazuo_enter.json`：`entity.player.breath` pitch 0.5 vol 0.4 + `block.deepslate.place` pitch 0.4 vol 0.25 delay 3 tick；attenuation MELEE 半径。
- **HUD**：真元竖条旁 8×8「入定」小标记，仅打坐时渲染（未打坐零渲染——HUD Conditional 约定）；打断时标记闪红 6 tick 后消失。
- 测试：intent 往返、动画 JSON 关键帧完整性（axis endTick 检查）、VFX 事件注册断言、HUD 条件渲染分支。

## P2 — practice_session 练功接活 ⬜

> 认领 reminder.md「`practice_session_tick` 接活留专门 plan」条目（plan-furniture-buff-v1 §8.1 #6 剥离），其约束全文迁入本节，reminder 对应条目随本骨架 PR 删除。

- 包装 system 注册：`track_casts_for_practice_entry`（连续 3 次同招 cast 进入练功态）→ `practice_session_tick` 每 tick 熟练度累积 + qi 消耗 → `check_practice_session_exit` 退出并发 `PracticeSessionEnded` + narration（`practice_narration_text` 三档文案已备）。
- **守恒还账**（进 P2 的硬门）：`practice_session_tick` 的 `*current_qi -= cost` 改经 `qi_release_to_zone` 释放回当前 zone + ledger 断言；练功与打坐互斥（同为"静态修行"，二态并存时 practice 优先，§8 #4）。
- 测试：进入阈值（2 次不进/3 次进）、退出全分支（移动/攻击/qi 枯竭/换招）、熟练度增益公式对拍、**守恒断言（zone 增量 = 练功消耗累计）**、narration 三档边界（<200 / 200-1000 / >1000 tick）。

## P3 — 数值收口 + bot e2e + Clamp 联动 ⬜

- 数值表（初值，const 声明在 `cultivation` 常量区，测试引用 const）：`DAZUO_ABSORB_MULT=3.0`、`DAZUO_ENTER_HOLD_TICKS=10`、`FIRST_SIT_HOOK_TICKS=60`。
- **与加速器 Clamp 关系拍板落文档**：打坐是**基线行为**不是加速器——journey §F「加速器 30% Clamp」只约束丹药/蒲团/灵田类外源加速；打坐倍率不进 clamp 池，但「打坐 × 蒲团 × 丹药」总吞吐要在 P3 实测一轮防灵气抽干（zone 恢复速率 vs 满倍率吸灵的平衡表）。
- bot 场景 `cultivation_dazuo_absorb.py`：dazuo intent → 对比窗口期 qi 增速 ≥ 基线 ×2.5 → 移动 → 断言退出 + 增速回落；练功场景 `cultivation_practice_session.py`（3 连 cast → proficiency payload 上涨 → zone qi 增量断言）。
- 30min 钩子矩阵联动：本 plan P0+P1 落地后，[[plan-newbie-30min-hooks-audit-v1]] 的 10:00 钩子从 SKIP 转真断言。

## §8 开放问题（升 active / P0 决策门前收口）

1. **倍率与基线语义**：行动态保持现系数、打坐 ×3（推荐——存量数值零迁移）vs 重定基线（打坐=标准速率、行动=折减，语义更贴 worldview 但牵连全部已校准的修炼时长曲线与 journey §F 时间预算）。
2. **触发手势定稿**：潜行+空手右键长按（推荐，零新键位）vs 独立键位 vs InspectScreen 按钮（违背沉默引导的"身体动作感"，不推荐）。
3. **打坐期脆弱性**：被击打断是否附加短暂硬直/伤害易伤（worldview「静坐是暴露」的张力）——推荐 v1 只打断不惩罚，惩罚留数值 plan。
4. **打坐 vs 练功互斥细节**：练功（动态挥招）与打坐（静态吸灵）显然互斥，但退出练功瞬间能否直接转打坐（冷却 0 vs 短冷却防状态抖动）。
5. **NPC 是否共用**：dormant NPC 吸灵已有独立路径（`apply_dormant_regen_with_multiplier`）——本 plan 推荐玩家 only，NPC 打坐姿态留 NPC 行为 plan。
