# plan-dandao-mutation-gameplay-v1（骨架）

> **骨架（草案）**。一句话主题：丹道变异当前是**“会长、会同步、会显示，但大多不会生效”**的 display-only bug。`server/src/dandao/mutation.rs` 已为 12 种异化定义玩法效果与衍生招式，client HUD 也会把这些效果文案展示给玩家；但主注册路径只接了阶段推进、视觉同步、agent 叙事，绝大多数功能性效果没有任何 runtime consumer，3 个玩家侧变异招式也没有注册进技能表。结果是玩家在实际游玩里承受了丹毒和外观异化，却拿不到承诺中的额外手臂、排毒加成、天然护甲、体型增益或异化招式。

> **立项动机**：本轮 bughunt 扫 `server/src/dandao/` 主路径时，先从 `MutationKind.effect()` 逆查所有 consumer，再对照 `register()` / `register_skills()` / client inspect/HUD 链路。结果发现：效果定义与 UI 文案都存在，但 runtime 基本不消费；已有 `docs/plans-skeleton/plan-module-wiring-gaps-v2.md:33-42` 把它列为 T1 高优先级决策主题，仓库里却还没有独立 skeleton 落位。该 bug 可达、玩家可感知，且不与本轮已占坑的 craft close pause / trap equip / surface stash / botany full inventory loss 重复。

## 阶段总览

| 阶段 | 主题 | 路由 | 状态 |
|------|------|------|------|
| P0 | 变异功能效果 dead data（7 类 effect 无 consumer） | plan_skeleton | ⬜ |
| P1 | 变异招式 dead skill（bone_slam / horn_charge / tail_strike 不可用） | plan_skeleton | ⬜ |
| P2 | inspect / 他人异化观察链路半成品 | plan_skeleton | ⬜ |

## P0 — 变异功能效果 dead data

- `server/src/dandao/mutation.rs:117-212` 把变异写成明确的**功能性效果**而不是纯 flavor：金瞳给视野强化，硬甲指给徒手伤害，糙皮给排毒加成，前臂鳞/背甲给天然护甲，脊突给减伤，多臂给 `ExtraHandSlots { count: 2 }`，体型膨胀给 `hp_pct: 0.50 + hitbox_scale: 1.5`，兽面给 `IntimidateAura`。
- 但 `server/src/dandao/mod.rs:34-57` 的运行时注册只接了 `track_pill_intake_system`、`mutation_advance_system`、`mutation_visual_emit_system`、`mutation_event_publish_system` 和暴龙王 BOSS；没有任何“按变异效果修改战斗/库存/属性/感知”的 consumer。
- `server/src/cultivation/contamination.rs:88-129` 的 `contamination_tick` 只根据 `Alchemy` 技艺等级算 `purge_rate_bonus`，Query 里没有 `MutationState` / `StatusEffects` / `MutationEffect`；因此糙皮的 `PurgeBoost { contamination_purge_pct: 0.10 }` 永远不生效。
- 全仓 `rg` 结果表明 `MutationEffect` 各变体只出现在 `mutation.rs` 定义和测试、以及 `visual_sync.rs` 的 HUD 文案拼接处；看不到任何战斗、库存、属性、渲染感知系统把这些效果落地。
- **这个 bug 对实际游玩体验的影响**：玩家正常走丹道时，丹毒累计跨线后会真的长角、长尾、多臂、兽化，也会背上更高经脉惩罚和 NPC 负面观感；但 promised buff 基本没有兑现。最直观的是“多臂”不会给玩家多出的持械收益，“糙皮”不会改善排毒，“背甲/鳞甲”不会减轻受伤，“体型膨胀”不会带来更肉的实战体验，形成“风险真实存在、奖励基本是假”的断层。

## P1 — 变异招式 dead skill

- `server/src/dandao/mutation.rs:128-146` 把骨脊、双角、尾分别定义为 `dandao.bone_slam`、`dandao.horn_charge`、`dandao.tail_strike` 三类衍生能力。
- 但 `server/src/dandao/mod.rs:60-64` 的 `register_skills()` 只注册了 `dandao.pill_rush`、`dandao.pill_bomb`、`dandao.pill_mist` 三个基础丹道招式。
- 全仓 `rg` 里 `dandao.bone_slam` / `dandao.tail_strike` 只命中 `mutation.rs`；`dandao.horn_charge` 除了 `mutation.rs` 之外，只在 `boss_ai.rs` / `boss_spawn.rs` 作为**暴龙王 BOSS 动作名**出现，和玩家变异技能无关。
- 这意味着玩家即使已经获得对应异化部位，也没有可施放的技能入口、技能注册或数值解析链路。
- **这个 bug 对实际游玩体验的影响**：变异在 UI 文案里承诺了“长出骨脊/双角/尾巴会解锁新手段”，但玩家实战中依然只能用基础丹道三招；高风险的走火入魔路线没有带来预期的战斗分化，直接削弱了丹道 build 的可玩性和成长反馈。

## P2 — inspect / 他人异化观察链路半成品

- `client/src/main/java/com/bong/client/dandao/MutationInspectLabel.java:9-79` 明确写了“查看他人异化状态”的规则与标签生成。
- 但全仓 `rg` 显示 `buildLabels()` 没有生产调用点；`client/src/main/java/com/bong/client/dandao/MutationVisualState.java:8-33` 只有单份静态本地状态，`update()` 只适合存“自己收到的 mutation_visual”。
- `server/src/dandao/visual_sync.rs:126-133` 还会把 `effect_desc: format!("{:?}", s.kind.effect())` 塞进 HUD 面板，继续向客户端强化“这些效果已经存在”的观感。
- **这个 bug 对实际游玩体验的影响**：玩家不仅自己拿不到变异效果，连观察别人时也没有完整 inspect 支撑。丹道最核心的“他人能看出你已显变/重变/兽化、并据此判断威胁”的社交和战斗识别面，也停在半成品。

## 两轮反方裁决摘要

1. **反方第 1 轮**：这些 `MutationEffect` 可能只是未来占位，不算 bug。
   **裁决**：不成立。`mutation.rs` 把它们写成带明确数值的功能效果，`visual_sync.rs:126-133` 还把 `effect_desc` 直接展示给玩家；这已经不是纯注释占位，而是“产品面对玩家声称存在某效果”。玩家侧承诺存在、runtime 却没有 consumer，属于高置信 gameplay bug。
2. **反方第 2 轮**：部分效果也许被别处隐式消费了，例如多臂已经有 extra hand 槽、角冲撞已经在代码里。
   **裁决**：仍不成立。extra hand 槽是库存系统的通用能力，仓库里找不到任何“由 `MutationState::ExtraArms` 触发授予”的接线；`dandao.horn_charge` 只被 BOSS AI 当成动作名使用，`bone_slam` / `tail_strike` 更是零消费。`contamination_tick` 也完全不读 `MutationState`，直接证伪了糙皮排毒加成的“隐式生效”假说。

## 开放问题

1. 是按 `MutationEffect` 逐类挂到既有系统，还是先补一个统一的“mutation_effect_consumer” 汇总层，再把战斗/库存/感知改造拆到子 PR。
2. 多臂是否直接复用现成 `extra_hand_0/1` 槽，还是要做“未获得 ExtraArms 前禁用、获得后解锁”的动态门控。
3. 体型膨胀、兽面光环、天然护甲都牵涉战斗平衡和碰撞/感知边界，适合单切数值 PR 还是并入一份完整 `dandao` gameplay plan。

## 审计来源

bughunt 线程 N（worktree: `.worktree/bughunt-loop-20260705-n`，branch: `bughunt-loop-20260705-n-alchemy-processing`）。主线读码后做两轮反方证伪：先排除“纯占位文案”的可能，再排除“被别处隐式消费”的可能；结论均失败，最终保留为高置信 skeleton。相关前情也与 `docs/plans-skeleton/plan-module-wiring-gaps-v2.md:33-42` 的 T1 决策主题互相印证。
