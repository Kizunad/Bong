# plan-woliu-dying-master-runtime-gap-v1（骨架）

> **骨架（草案）**。一句话主题：`plan-woliu-v4` 宣称已落地的 **“垂死大能” NPC sidepath** 在当前 `main`/本分支代码里只有纯函数、单测与一条 despawn tick，**没有任何生产 spawn / 遭遇 / 交互 / 掉落 / 战斗接线**，因此该 sidepath 实际上 **永远不会发生**。

> 立项动机：这不是“文档没更新”的轻微漂移，而是一个对实际玩法可感知的 **零运行态** 缺口。`docs/finished_plans/plan-woliu-v4.md` 仍把它写成已完成 P2，甚至列了 `e2e_dying_master_ambush` / `e2e_dying_master_patience`；但代码树里找不到任何能把玩家带进该遭遇的生产入口。

## 阶段总览

| 阶段 | 主题 | 路由 | 状态 |
|------|------|------|------|
| P0 | Woliu v4 垂死大能 sidepath 零运行态（不刷 / 不可交互 / 不掉卷） | fix_pr | ⬜ |

## P0 — Woliu v4 垂死大能 sidepath 零运行态

- **#1 major（fix_pr）**：`docs/finished_plans/plan-woliu-v4.md` 明确把“垂死大能”写成已交付玩法链：
  - `§P2.1`（line 342 起）要求在 `server/src/npc/dying_master.rs` 落一个 **特殊 NPC entity + 对话树 + 30s 倒计时 despawn**，触发条件是“玩家进入负灵域洞穴 chunk + 0.5% 判定”。
  - `Finish Evidence`（line 702）进一步写死“**垂死大能 encounter、30s 自然死亡、给丹 50/50 分流与战斗触发在 `server/src/npc/dying_master.rs`**”。
  - `E2E`（line 647-650）还列出 `e2e_dying_master_ambush` / `e2e_dying_master_patience` 两条实机路径。
- 但实际代码链路只剩壳：
  - `server/src/npc/dying_master.rs` 仅定义 `DyingMaster` component、`DyingMasterEncounterEvent`、`should_spawn_in_negative_zone`、`path_a_aid_outcome`、`path_c_earth_scroll_drop`、`seize_body_triggers_combat`，以及 `dying_master_despawn_tick`。**全文件没有 spawn system、没有 Commands::spawn、没有 EventWriter、没有掉落、没有战斗、没有 inventory/丹药交互。**
  - 全仓 `rg` 结果表明：`DyingMasterEncounterEvent` 只在 `server/src/npc/dying_master.rs` 自身定义，外加 `server/src/npc/mod.rs:107` 注册事件；**零生产 sender / zero consumer**。
  - 全仓 `rg` 结果表明：`DyingMaster` component 只在 `server/src/npc/dying_master.rs` 自身定义和单测里构造；**没有任何 `insert(DyingMaster)` / `spawn(DyingMaster)` 生产写入点**。
  - `server/src/npc/mod.rs:107-110` 对该模块的运行时接线只有：
    - `app.add_event::<dying_master::DyingMasterEncounterEvent>();`
    - `app.add_systems(Update, dying_master::dying_master_despawn_tick);`
    - `dying_master::log_dying_master_contract();`
    - 也就是说运行态只剩“注册一个永远没人发的 event + 给一个永远没人挂载的 component 跑 despawn tick + 打 debug log”。
  - 更进一步，`server/src/npc/lifecycle.rs` 的 `NpcArchetype` **根本没有 `DyingMaster` 变体**；当前真正存在的是后来的 `DyingElder`。这意味着即使有人想走既有 NPC inspect / metadata / lifecycle / archetype 分流，`dying_master` 也没有统一身份可接。
- **实机复现路径**：
  1. 按 `plan-woliu-v4` 描述进入负灵域洞穴区块，反复触发 chunk 首次进入条件。
  2. 期望：出现“救……给我回元丹……”的特殊 NPC，随后玩家可选给丹 / 拒绝 / 拖延。
  3. 实际：代码里不存在任何调用 `should_spawn_in_negative_zone()` 的生产系统，也不存在任何 spawn `DyingMaster` 实体的入口，所以**永远不会出现该 NPC**。
  4. 连锁结果：A 路线的“给丹 50/50 传功/夺舍”、C 路线的“等 30s 死亡后掉地阶残卷”、以及文档承诺的两条 e2e 路径都不可能在运行时发生。
- **根因链路**：
  1. `plan-woliu-v4` 的 P2 以 `server/src/npc/dying_master.rs` 为交付落点；
  2. 最终代码只留下纯函数与单测，未把 chunk-load gate、entity spawn、交互 UI/请求、奖励、战斗接入 runtime；
  3. 后续仓库引入了 `fauna/dying_elder.rs` 这条更完整的“垂死大能”体系，但没有回填 Woliu v4 文档，也没有把 Woliu sidepath 迁到新系统；
  4. 结果是：**文档宣称已完成，runtime 实际为零。**

## 这个 bug 对实际游玩体验的影响

- 玩家按 `plan-woliu-v4` / 已合并内容去探索负灵域洞穴时，**永远遇不到**这条高风险高回报的 NPC sidepath。
- Woliu v4 失去了一条本应非常有辨识度的获取路线：没有“给丹赌传功”、没有“拖时间等其自毙舔地阶残卷”、也没有“翻脸夺舍”的陷阱体验。
- 体感上就是：文档、数值说明、叙事预期都在暗示“这里会发生一件坏事”，但游戏里什么都不会发生；负灵域洞穴探索因此少了一整段设计好的惊险桥段。

## 修复建议

- **方案 A（推荐）**：把 Woliu v4 的 sidepath 真正接活。
  - 新增生产 spawn system，在“玩家进入负灵域洞穴 chunk”时调用 `should_spawn_in_negative_zone()`。
  - 真实生成带 `NpcMarker` / `Position` / 生命周期 / 可交互状态的特殊实体；若不新增 archetype，至少明确复用 `DyingElder` 还是独立 `DyingMaster`。
  - 接上交互入口：给丹、拒绝、拖延、30s 自然死亡、A 路线 50/50 分流、掉卷与 PvE 战斗。
  - 把文档里列出的 `e2e_dying_master_ambush` / `e2e_dying_master_patience` 真的补成可跑测试。
- **方案 B（降级）**：若产品决策是“Woliu v4 旧 sidepath 已被 `fauna/dying_elder.rs` 取代”，那就必须：
  - 删除/改写 `plan-woliu-v4.md` 的 Finish Evidence 与 E2E 承诺；
  - 移除 `npc/dying_master.rs` 这套半残 runtime 壳，避免继续误导后续开发与 bug-hunt。

## 反方裁决

- **第 1 轮反方论点**：这可能不是 bug，而是后来被 `fauna/dying_elder.rs` 正式替代的旧设计残骸。
  - **驳回**：若确实已替代，`plan-woliu-v4.md` 不应仍把 `server/src/npc/dying_master.rs` 写成已交付落点，更不应保留 `e2e_dying_master_*` 承诺；同时 `npc/mod.rs` 仍在注册该模块 runtime 壳，说明仓库当前对外状态不是“纯历史遗留已下线”，而是“半接线、误报已完成”。
- **第 2 轮反方论点**：即便这条 sidepath 不工作，Woliu 地阶残卷还能从别处掉落，不算真实玩法 bug。
  - **驳回**：本 bug 针对的是 **sidepath 本身的零运行态**，不是“是否唯一阻塞 Woliu 获取”。文档承诺的遭遇、倒计时、拖延解法、夺舍翻脸、负灵域洞穴惊险感全部消失，属于独立且可感知的玩法缺失。

## 反方裁决执行说明

- 当前会话**没有可用 subagent / delegate_task 工具链**来再开两轮外部怀疑者代理；本次改为主代理基于全仓静态证据执行 **两轮退化版反方裁决**，并在上文显式记录反方论点与驳回理由。

## 审计来源

- bug-hunt 定点轮（NPC sidepath，避开 dormant 失忆 / trade gate / silent signal runtime bridge / social anonymity live refresh / war group reputation gap）。
- 证据来自：
  - `docs/finished_plans/plan-woliu-v4.md`
  - `server/src/npc/dying_master.rs`
  - `server/src/npc/mod.rs`
  - 全仓 `rg` 对 `DyingMasterEncounterEvent` / `DyingMaster` / `e2e_dying_master_*` / `mentor_checks_realm` 的调用面审计。
