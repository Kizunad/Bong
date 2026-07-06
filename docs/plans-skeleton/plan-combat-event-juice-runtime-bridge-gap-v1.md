# plan-combat-event-juice-runtime-bridge-gap-v1（骨架）

> 一句话主题：`combat_event` 线上 runtime bridge 仍停留在“只够飘字”的最小 payload，导致 `plan-combat-gamefeel-v1` 已宣称接入的 `CombatJuiceSystem` 在线上拿不到实体 UUID / 流派 / 方向 / 本地击杀身份，**普通命中 hit-stop 实际不生效，战斗手感大量退化成 generic fallback**。

## 结论

- **结论**：这是一个真实 bug，不是文档噪音。`server/src/network/combat_event_emit.rs` 生产的 `CombatEventFloaterEntryV1` 只含 `kind/amount/text/x/y/z`，而 client `CombatEventHandler` 已按 `plan-combat-gamefeel-v1` 去消费 `attacker_uuid/target_uuid/local_player_uuid/victim_name/school/tier/direction_x/direction_z/kill/perfect/rare_drop` 等富字段。结果是 live payload 能出飘字、能触发部分 shake，但 **hit-stop / school profile / parry pushback / local kill slowmo 等关键 battle feedback 分支在真人游玩里拿不到必要上下文**。

## 复现路径

1. 玩家在实战里造成一次普通命中或盾格挡命中，server 进入 [`server/src/network/combat_event_emit.rs:15`](/home/kiz/Code/Bong/.worktree/bughunt-loop-20260705-bp/server/src/network/combat_event_emit.rs:15)。
2. emit 侧构造的 `CombatEventFloaterEntryV1` 只有 `kind/amount/text/x/y/z` 六个字段；且坐标还被硬编码为 `0.0/0.0/0.0`，见 [`combat_event_emit.rs:28-35`](/home/kiz/Code/Bong/.worktree/bughunt-loop-20260705-bp/server/src/network/combat_event_emit.rs:28)。
3. schema 也把 `CombatEventFloaterEntryV1` 锁死为这六个字段，见 [`server/src/schema/server_data.rs:649-664`](/home/kiz/Code/Bong/.worktree/bughunt-loop-20260705-bp/server/src/schema/server_data.rs:649)。
4. client `CombatEventHandler` 收包后会调用 `toJuiceEvent()`，主动尝试读取 `school/attacker_uuid/target_uuid/local_player_uuid/victim_name/direction_x/direction_z/kill/perfect/rare_drop`，见 [`client/.../CombatEventHandler.java:136-176`](/home/kiz/Code/Bong/.worktree/bughunt-loop-20260705-bp/client/src/main/java/com/bong/client/combat/handler/CombatEventHandler.java:136)。
5. 由于线上 payload 根本没有这些字段，`CombatJuiceEvent` 被构造成空 UUID、空本地身份、默认方向、`CombatSchool.GENERIC`。
6. `HitStopController.request()` 只会对**非空** attacker/target UUID 建冻结，见 [`HitStopController.java:14-26`](/home/kiz/Code/Bong/.worktree/bughunt-loop-20260705-bp/client/src/main/java/com/bong/client/combat/juice/HitStopController.java:14)；live 命中因此直接无 freeze。
7. `KillJuiceController.trigger()` 还要求 `local_player_uuid` 非空且等于 attacker，见 [`KillJuiceController.java:12-18`](/home/kiz/Code/Bong/.worktree/bughunt-loop-20260705-bp/client/src/main/java/com/bong/client/combat/juice/KillJuiceController.java:12)；当前 payload 家族无法满足。

## 根因链路

- `plan-combat-feedback-v1` 最早只规划了“飘字最小集”，样例里也只有 `kind/amount/x/y/z/text`，见 [`docs/finished_plans/plan-combat-feedback-v1.md:48-67`](/home/kiz/Code/Bong/.worktree/bughunt-loop-20260705-bp/docs/finished_plans/plan-combat-feedback-v1.md:48)。
- 后续 `plan-combat-gamefeel-v1` 已把 `CombatEventHandler` 升格为 `CombatJuiceSystem` 入口，明确宣称 client 会消费 `combat_event` 的 `hit/parry/dodge/kill/qi_collision/full_charge/overload` 可选字段，见 [`plan-combat-gamefeel-v1.md:271-286`](/home/kiz/Code/Bong/.worktree/bughunt-loop-20260705-bp/docs/finished_plans/plan-combat-gamefeel-v1.md:271)。
- 但 server emit / server schema / proto roundtrip 从未同步扩展；`plan-wire-format-bridge-v1` 只把 `combat_event.*` 富化簇登记为“scope 外遗留”，见 [`plan-wire-format-bridge-v1.md:101-107`](/home/kiz/Code/Bong/.worktree/bughunt-loop-20260705-bp/docs/finished_plans/plan-wire-format-bridge-v1.md:101) 与 [`:231-233`](/home/kiz/Code/Bong/.worktree/bughunt-loop-20260705-bp/docs/finished_plans/plan-wire-format-bridge-v1.md:231)。
- 于是形成断层：**client 代码已按富字段实现，server runtime bridge 仍只发最小飘字。**

## 影响面

- **普通命中 hit-stop 实际失效**：`CombatJuiceSystem` 会走 `HIT` 分支，但 `HitStopController` 因 UUID 为空而不建任何 freeze。
- **流派差异化退化为 generic**：`CombatSchool.fromWire("") -> GENERIC`，命中震屏/色调不再体现暴脉、毒蛊、阵法等 school profile。
- **招架/格挡 pushback 退化**：方向字段缺失时只能吃默认方向；UUID 缺失时 pushback 目标为空。
- **击杀慢动作无法按本地击杀门控触发**：`local_player_uuid` / `victim_name` / `rare_drop` 都无法从当前 payload 家族表达。
- **测试被手写 JSON 掩盖**：client 现有测试直接喂 `target_uuid` 等富字段，而不是复用真实 server emit 形状。

## 这个 bug 对实际游玩体验的影响

- 玩家会看到“有数字飘出来、镜头偶尔震一下”，但**最该有粘滞感的命中停顿没有发生**，不同流派的打击感也被压成近似模板化反馈；对近战格挡、连击确认、击杀收尾尤其明显，体感是“命中了，但没打实”。

## 证据

- live emit 只写六字段：[`server/src/network/combat_event_emit.rs:25-35`](/home/kiz/Code/Bong/.worktree/bughunt-loop-20260705-bp/server/src/network/combat_event_emit.rs:25)
- schema 也只允许六字段：[`server/src/schema/server_data.rs:649-664`](/home/kiz/Code/Bong/.worktree/bughunt-loop-20260705-bp/server/src/schema/server_data.rs:649)
- client 明确依赖富字段：[`client/src/main/java/com/bong/client/combat/handler/CombatEventHandler.java:136-176`](/home/kiz/Code/Bong/.worktree/bughunt-loop-20260705-bp/client/src/main/java/com/bong/client/combat/handler/CombatEventHandler.java:136)
- hit-stop 对空 UUID 直接 no-op：[`client/src/main/java/com/bong/client/combat/juice/HitStopController.java:14-26`](/home/kiz/Code/Bong/.worktree/bughunt-loop-20260705-bp/client/src/main/java/com/bong/client/combat/juice/HitStopController.java:14)
- kill slowmo 对空 `local_player_uuid` 直接 no-op：[`client/src/main/java/com/bong/client/combat/juice/KillJuiceController.java:12-18`](/home/kiz/Code/Bong/.worktree/bughunt-loop-20260705-bp/client/src/main/java/com/bong/client/combat/juice/KillJuiceController.java:12)
- client 测试使用“线上不会出现”的富字段 JSON：[`client/src/test/java/com/bong/client/network/ServerDataRouterCombatTest.java:41-55`](/home/kiz/Code/Bong/.worktree/bughunt-loop-20260705-bp/client/src/test/java/com/bong/client/network/ServerDataRouterCombatTest.java:41)、[`client/src/test/java/com/bong/client/combat/ShieldBlockJuiceTest.java:149-166`](/home/kiz/Code/Bong/.worktree/bughunt-loop-20260705-bp/client/src/test/java/com/bong/client/combat/ShieldBlockJuiceTest.java:149)

## 修复建议

1. 扩 `CombatEventFloaterEntryV1` / proto `CombatEventFloaterEntry`，至少补：`attacker_uuid`、`target_uuid`、`local_player_uuid`、`victim_name`、`school`、`tier`、`direction_x`、`direction_z`、`kill`、`perfect`、`rare_drop`。
2. `emit_combat_event_to_client` 在构包时填真实 `CombatEvent` 上下文；`local_player_uuid` 需要按接收方（attacker/target）分别写入。
3. 同步把 `x/y/z` 改成真实命中或目标位置，别再硬编码 `0.0`。
4. 增加**真端到端**测试：从 server emit / proto bridge 产出 payload，再进 `ServerDataRouter`，断言 live 形状下 `HitStopController.remainingTicks(target)>0`，而不是继续用手写富字段 JSON。

## 反方裁决摘要（退化处理）

- **退化说明**：当前会话没有可用 subagent / delegate 工具，无法再开独立怀疑者子代理；本轮改为主代理手工执行两轮反方裁决，并把反方论点与驳回理由明文记档。
- **反方裁决 round 1**
  - 反方论点：`combat_event.*` 富化簇早在 `plan-wire-format-bridge-v1` 被标成“scope 外”，这更像 backlog，不是 bug。
  - 驳回理由：scope 外只说明“那一单没修”，不等于“线上没问题”。`plan-combat-gamefeel-v1` 已把 `CombatEventHandler` 作为 shipped juice 入口写进 Finish Evidence；既然 live sender 永远不给它关键字段，这就是已上线功能的真实断桥。
- **反方裁决 round 2**
  - 反方论点：现在线上至少还有飘字、DamageTilt 和部分 shake，不能算 battle feedback 失效。
  - 驳回理由：本条 skeleton 指控的不是“完全无反馈”，而是**核心战斗手感层被静默降级**。`HitStopController` 对空 UUID 必然不建 freeze，这是代码级硬结论；现有测试之所以没暴露，只因它们喂了真实 server 永远不会发出的 `target_uuid` 富字段。

## 建议优先级

- **建议优先级：P1 / fix_pr**
- 原因：不改战斗数值，不牵涉世界观和守恒律，但直接影响每次命中体感；属于“线上一直在发生的表现层断桥”，修复面集中在 server schema / emit / client 端到端测试。
