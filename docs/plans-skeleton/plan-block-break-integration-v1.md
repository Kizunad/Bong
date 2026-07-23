# plan-block-break-integration-v1 — 统一方块破坏集成层（break reason + hook 管线 + 程序化破坏 API）

**主题**：把当前散落在 7 个模块里各自为政的 `EventReader<DiggingEvent>` 消费者收拢成一条**统一破坏管线**——引入 `BreakReason`（怎么破的）/ `BreakCause`（谁破的）语义、pre-break veto → apply → post-break effect 的有序 hook 注册机制、以及让服务端逻辑（爆炸 / 招式 / 陷阱 / 天道世界事件）复用同一条掉落/清理/索引链路的**程序化破坏请求 API**。终极目的：为「§858 盲盒死信箱『遭受非对应破坏』触发阵法」「§417 地师埋陷阱」这类**破坏原因驱动的机关玩法**打好基座。

> **形态**：这是一份**基础设施 / 框架整合** plan（纯 server 逻辑为主，视听规格仅 P4 破坏反馈差异化涉及）。它本身不是玩法，而是把破坏方块这件事从「7 处各判各的」变成「一处funnel + 注册式 hook」，让下游 trap / container / skill plan 接得上。

## 阶段总览

| 阶段 | 主题 | 状态 |
|------|------|------|
| P0 | 核心类型 + 事件总线：`BreakReason` / `BreakCause` / `BlockBreakEvent` / `BlockBreakRequest`，player `DiggingEvent` → 统一事件 adapter（行为不变） | ⬜ |
| P1 | Hook 注册管线：pre-break veto trait + post-break effect trait（有序），迁移现有 7 个消费者上 hook API | ⬜ |
| P2 | 程序化破坏 API：`BlockBreakRequest` 消费者跑同一条管线，爆炸 / 招式 block-destroy 改发 request 而非裸 `set_block` | ⬜ |
| P3 | 陷阱基座：`BreakTriggeredTrap` component + registry，`BreakReason` 判定「非对应破坏」→ 触发陷阱效果（参照 `zhenfa::BlastTrap`） | ⬜ |
| P4 | 破坏反馈差异化 + 饱和测试：不同 `BreakReason` 驱动差异化 VFX/SFX（陷阱破坏 ≠ 镐子破坏），端到端 e2e | ⬜ |

（骨架草案，验收日期待填。升 active 前必须先按 §8 收口开放问题。）

---

## 接入面（防孤岛 checklist）

- **进料**：
  - valence `DiggingEvent`（玩家挖掘，唯一现有入口）→ 经 P0 adapter 翻译成 `BlockBreakEvent`
  - `GameMode`（决定 Creative 秒破 / Survival 挖完破 / Adventure·Spectator 不破，现 `world/block_break.rs:27` 的门迁进 reason）
  - 服务端破坏来源：爆炸（现无统一原语）/ 招式 block-destroy / 陷阱触发 / 天道世界事件（伪灵脉塌缩等）→ 经 P2 `BlockBreakRequest`
  - 现有 registry：`MineralOreIndex` / `SpiritNicheRegistry` / `FurnitureRegistry` / 容器 registry（`container_block`）
- **出料**：
  - `BlockBreakEvent` → 有序 hook → `set_block(AIR)` + 各 registry 清理
  - 掉落经现有 `MineralDropEvent` / `block_drop` → `inventory`
  - 容器破坏 → `LootContainerCloseReasonV1::ContainerDestroyed`（`plan-placeable-container-blocks-v1` §858 死信箱依赖此项强制关闭 open-session）
  - 陷阱触发 → `zhenfa::BlastTrap`（owner 当 attacker 范式）/ StatusEffect（毒气雷）/ 真元地刺（走 `qi_physics` ledger）
  - 客户端破坏反馈差异化 → `bong:vfx_event`
- **共享类型 / event（不新造重名）**：
  - **复用** valence `DiggingEvent` 作为 player 输入 adapter，不替换协议
  - **复用** `MineralDropEvent` / `MineralOreIndex` / `FurnitureRegistry` / `SpiritNicheRegistry`——**严禁**再造一份平行 drop event
  - **复用** `LootContainerCloseReasonV1::ContainerDestroyed`（container-blocks plan 定义）
  - 新增 `BlockBreakEvent` / `BlockBreakRequest` / `BreakReason` / `BreakCause`：现无统一破坏事件，7 个消费者各自读 `DiggingEvent` 是碎片化根因（见下方证据），需要单一 funnel
- **跨仓库契约**：
  - server 主体：新 `world::block_break` 模块的 `BlockBreakEvent` / `BreakReason` / hook trait
  - agent（天道）：可 emit `BlockBreakRequest` 用于**天道陷阱**（worldview §38 伪灵脉）/ 世界事件塌缩——Redis `bong:agent_cmd` 新增破坏指令 variant（P2/P3 待定是否纳入本 plan 还是留给天道 plan）
  - client：`BreakReason` → 破坏 VFX/SFX 差异化，`bong:vfx_event` 事件 ID + 可能的 schema 字段（P4）
- **worldview 锚点**：
  - **§858 盲盒死信箱**——"箱子**遭受非对应破坏时**,内部阵法启动:物品化灰 + 原地引爆毒气雷"（break reason 检测的正典原型）
  - **§417-419 地师/阵法流**——"必经之路埋设陷阱,敌人踩中瞬间地下真元如地刺贯穿双腿"
  - **§482 阵法地雷**、**§1267 幽暗地穴禁制/陷阱**、**§38 天道陷阱(伪灵脉)**
  - **§860 游商傀儡**——抢劫傀儡自锁（破坏/交互驱动的触发同源）
- **qi_physics 锚点**：破坏方块本身**不动真元**（无 `*_DECAY*` 常数）。但 P3 陷阱**效果**若涉及真元（毒性真元 §482 / 地刺贯穿 / 毒气雷），真元流动必须走 `qi_physics::ledger::QiTransfer`——本 plan **只声明陷阱触发点 + reason 判定**，真元物理归 `qi_physics` + 下游具体陷阱 plan，本 plan 不写真元公式。

---

## 碎片化现状（立 plan 的证据）

当前 **7 个模块各自独立** `EventReader<DiggingEvent>`，每个自己判 `(DiggingState, GameMode)` 决定归不归它管，无统一 reason / 无顺序保证 / 无 veto 阶段：

| 模块 | 文件:行 | 职责 | 现有问题 |
|------|---------|------|----------|
| 默认破坏 apply | `world/block_break.rs:35` | chunk → AIR + furniture 清理 | 只认 player dig，服务端破坏（爆炸/陷阱）无法复用 |
| 掉落 | `world/block_drop.rs:162` | 方块掉落 | 与 mineral drop 两套并行逻辑 |
| 矿脉 | `mineral/break_handler.rs:172` | 镐品级门 + drop + karma + 采集进度 | 与默认 apply 靠"同帧消费同一事件"松耦合 |
| 灵龛保护 | `social/mod.rs:2131` | niche 位置退让 | 保护靠每个消费者**各自** check，脆弱（新消费者忘记 check 就绕过保护）|
| 灵木 | `spiritwood/mod.rs:85` | 灵木破坏 | 同上 |
| 工作台 | `craft/workbench.rs:165` | 工作台方块 | 跨维度破坏 bug（见 `plan-bughunt-workbench-cross-dimension-break-v1` 骨架）|
| 容器方块 | `world/container_block.rs:186` | 容器方块破坏 | open-session 强制关闭缺原语（container-blocks plan §858 需要）|

**碎片化后果**：① 保护逻辑（灵龛）靠每个消费者自觉 check → 漏一个就穿透；② 服务端破坏来源（爆炸/招式/陷阱/天道）没法走这条链 → 只能裸 `set_block(AIR)` 绕过所有掉落/清理/索引，产生"chunk 空了但 entity 还在"的鬼影（`mineral/break_handler.rs` 注释里已有自愈 warn）；③ bughunt 反复在散落消费者里抓 break 相关 bug（现有 2 份 break 相关 bughunt 骨架）；④ 没有 `BreakReason`，§858 死信箱"非对应破坏"这类玩法无从判定。

---

## P0 — 核心类型 + 事件总线 ⬜

**交付物**：
- 新增 `server/src/world/block_break/` 模块（或扩现有 `block_break.rs`）：
  - `enum BreakReason { PlayerDig { pickaxe_tier: u8 }, CreativeInstant, Explosion, Skill { skill_id: String }, Trap { trap_id: String }, WorldEvent, Support, Fire, Liquid }`（"检测 break reason" 的落地）
  - `enum BreakCause { Player(Entity), TrapOwner(Entity), Tiandao, Environment, None }`（谁/什么导致）
  - `struct BlockBreakEvent { pos: BlockPos, dimension: DimensionKind, block_state: BlockState, reason: BreakReason, cause: BreakCause }`
  - `struct BlockBreakRequest { pos, dimension, reason, cause }`（P2 用，P0 先定义）
- **player dig adapter**：`fn digging_to_break_event(DiggingEvent, GameMode) -> Option<BlockBreakEvent>`——把现 `should_apply_default_break` 的 `(state, mode)` 真值表映射成 `BreakReason::PlayerDig` / `CreativeInstant` / None，**行为逐条对齐现状**（8 组合真值表测试保留）
- 一个 `emit_block_break_events` system：读 `DiggingEvent` → 发 `BlockBreakEvent`（P0 阶段其余消费者仍读旧事件，保证不回归；P1 才切换）
- **测试**：`BreakReason` / `BreakCause` 每变体 pin；adapter 真值表（8 组合 + pickaxe_tier 透传）；`BlockBreakEvent` 字段 round-trip

**核验抓手**：`world::block_break::BreakReason` / `BlockBreakEvent` / `digging_to_break_event`；`block_break::tests` N 单测。

## P1 — Hook 注册管线 ⬜

**交付物**：
- `trait BreakVetoHook { fn veto(&self, event: &BlockBreakEvent, world: ...) -> Option<VetoReason>; }`——pre-break 否决（灵龛保护 / 容器锁 / 阵法禁制先迁这里）
- `trait BreakEffectHook { fn on_broken(&self, event: &BlockBreakEvent, commands, ...); }`——post-break 副作用（掉落 / 索引清理 / 容器关闭 / 陷阱触发）
- `BlockBreakPipeline` system：读 `BlockBreakEvent` → 跑所有 veto（任一否决即跳过，可选发否决反馈）→ `set_block(AIR)` + registry 清理 → 跑所有 effect hook（有序）
- **迁移现有 7 消费者上 hook**：mineral / spiritwood / social(veto) / block_drop / container(veto + effect) / workbench / furniture 从独立 `EventReader<DiggingEvent>` 改成注册 `BreakVetoHook` / `BreakEffectHook`。**契约不变**——外部可观察行为（掉落、index、niche 保护）逐个用现有测试锁死，只换接线
- **测试**：veto 短路（否决则不 set_block、不掉落）；hook 顺序确定性；灵龛保护现在是**集中** veto（新消费者不会绕过）；迁移后 mineral/spiritwood/container 现有测试全绿

**核验抓手**：`BreakVetoHook` / `BreakEffectHook` / `BlockBreakPipeline`；各模块 `register_*_break_hook`；迁移后 `mineral::break_handler` 不再 `EventReader<DiggingEvent>`。

## P2 — 程序化破坏 API ⬜

**交付物**：
- `BlockBreakRequest` 消费 system：读 request → 查 `block_state` → 构造 `BlockBreakEvent`（reason/cause 来自 request）→ **走 P1 同一条 pipeline**（含 veto！服务端破坏也受保护约束，除非 reason 明示 bypass）
- 改造一处现有裸 `set_block(AIR)` 服务端破坏为 request（挑一个安全示例，如某招式的 block-destroy 或新增 AoE 爆炸原语）
- **爆炸原语**（可选，或留 P3）：`emit_explosion(center, radius, reason, cause)` → 批量 `BlockBreakRequest`
- **测试**：request → pipeline 完整跑（掉落 + 清理 + veto 生效）；服务端破坏受 veto（灵龛内爆炸不破被保护块）；reason bypass 语义（如 `WorldEvent` 塌缩可跳保护，明示）

**核验抓手**：`BlockBreakRequest` 消费 system；`emit_explosion` / 某招式改发 request；`block_break::tests` request 集成用例。

## P3 — 陷阱基座 ⬜

**交付物**：
- `struct BreakTriggeredTrap { authorized_reason: Option<BreakReason>, trap_effect: TrapEffect, owner: Entity }` component + `BreakTrapIndex`（按坐标查）
- 注册一个 `BreakEffectHook`：破坏命中 trap 坐标时，比对 `event.reason` 与 `authorized_reason`——**非对应破坏**（reason 不匹配 / cause 非授权）→ 触发 `trap_effect`（参照 `zhenfa::BlastTrap`，owner 当 attacker）
- 落地 worldview §858 死信箱最小闭环：容器 `authorized_reason = 授权开启`，被镐子/爆炸破坏（`PlayerDig`/`Explosion` 非授权）→ 物品化灰 + 引爆毒气雷（复用 `zhenfa::BlastTrap` + StatusEffect，**不新增 Poison 变体**）
- **测试**：授权破坏不触发；非对应破坏触发；trap owner 记账；毒气雷 AoE 命中范围
- **视听规格**（本阶段涉及玩家可感知，按 docs/CLAUDE.md §四 内联）：陷阱触发的粒子/音效/HUD——待 §8 收口时按精度模板补全（毒气雷绿雾 `BongSpriteParticle` + audio_recipe + 屏幕 tint）

**核验抓手**：`BreakTriggeredTrap` / `BreakTrapIndex` / 死信箱 e2e 用例；引用 `zhenfa::BlastTrap`。

> **边界**：P3 是**基座 + 死信箱最小闭环**。地师流「必经之路真元地刺」「阵法地雷」等完整陷阱玩法留给下游专门 plan（`plan-dishi-formation-v*` 之类），本 plan 只保证它们能 hook 进破坏管线。

## P4 — 破坏反馈差异化 + 饱和测试 ⬜

**交付物**：
- `BreakReason` → 客户端差异化破坏反馈：镐子破坏（vanilla 碎块）vs 陷阱触发破坏（阵法崩解 VFX + 特殊 SFX）vs 爆炸（冲击波），经 `bong:vfx_event`
- 端到端 e2e：player dig → BlockBreakEvent → pipeline → hooks → 掉落到 inventory + 客户端收到破坏 VFX
- 饱和回归：所有 `BreakReason` 变体各一条 e2e；veto 路径 e2e；request 路径 e2e
- **视听规格**：按 docs/CLAUDE.md §四 精度模板写全（粒子基类/数量/lifetime/hex/vfx ID + audio_recipe + HUD），§8 收口时锁定

**核验抓手**：`e2e` 破坏链路用例；client `BreakReason` VFX 分派。

---

## §8 开放问题（升 active / P0 决策门前需收口）

1. **veto 优先级 / 顺序**：veto hook 之间冲突如何裁决？（灵龛保护 vs 天道世界事件强制塌缩谁赢）→ 需定 hook priority 机制 + `WorldEvent` 是否可 bypass veto。
2. **request 是否受 veto**：服务端 `BlockBreakRequest`（爆炸/招式）默认受 veto，还是每 reason 声明 bypass？（灵龛内玩家招式能不能炸被保护块）
3. **`DiggingEvent` 保留还是彻底封装**：P1 迁移后旧事件是否还有直接消费者，或全部 funnel 进 `BlockBreakEvent`？（clean code 取向：应彻底 funnel，无 dual-form）
4. **爆炸原语归属**：`emit_explosion` 放本 plan（P2）还是留给 combat/skill plan？本 plan 只提供 request API 是否足够？
5. **陷阱 P3 范围**：死信箱最小闭环 vs 完整地师陷阱玩法拆分线——哪些留本 plan，哪些划给下游 `plan-dishi-*`？
6. **agent 破坏指令**：天道 emit `BlockBreakRequest`（伪灵脉塌缩 §38）纳入本 plan 还是留天道 plan？涉及 Redis `bong:agent_cmd` schema 扩展。
7. **与 `plan-placeable-container-blocks-v1` 的依赖顺序**：本 plan 的 `LootContainerCloseReasonV1::ContainerDestroyed` funnel 是先于还是后于 container-blocks plan 落地？两者谁定义该 reason？需协调避免 PR 撞车。
8. **性能**：hook 管线每次破坏遍历所有注册 hook——大范围爆炸（batch request）是否需要空间索引预筛（只跑坐标命中的 trap hook）？

> 收口方式：按 docs/CLAUDE.md §5.1，升 active 前追加 `## §8.1 决议（pre-P0 收口，YYYY-MM-DD）`，每条靠 Explore agent 并行核查代码现状后落「文件:行号 + plan 章节」双锚点。

---

## §10 实施工作流（scope ≥ 4 PR，升 active 前补全）

本 plan 预计 5 阶段 ≥ 4 PR，升 active 前须按 docs/CLAUDE.md §六 补 §10：
- **PR 拆分**：P0 核心类型（纯 server）→ P1 hook 迁移（纯 server，逐模块）→ P2 request API → P3 陷阱基座 → P4 视听 + e2e
- P3/P4 涉及视听资产，按 §6.1 走多轮打磨 + `<PROMISE>`（毒气雷 VFX / 破坏差异化）
- subagent context 隔离 + CR 等待协议按 §6.4 / §6.5
