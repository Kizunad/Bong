# Bong · plan-bughunt-entity-interact-range-desync-v1

> 一句话主题：统一实体交互链（准星命中 / `G` 键 / C2S 请求）里，**client 侧候选距离比 server 真实可交互距离更宽，而且判定形状也不一致**，导致工作台 / 货箱 / 死信箱 / 物资棺出现稳定的“**看起来能交互，按下去却被 server 拒绝**”假交互带。范围避开已知的 `G` 键 nearest hijack、NPC trade gate、NPC dormant、movement interaction 近期题，只落在 server entity interaction + client intent/router 当前活跃链路。

## 阶段总览

| 阶段 | 主题 | 路由 | 状态 |
|------|------|------|------|
| P0 | 实体交互 client/server 距离判定漂移，形成 phantom interact 假交互带 | fix_pr | ✅ 2026-07-12 |

## P0 — 实体交互 client/server 距离判定漂移

- **结论**：`InteractKeyRouter` 驱动的实体交互链里，client 统一用 `5.0` 格欧氏半径放行候选，而 server 分别用 `4.5` 格欧氏半径（容器/物资棺）和 `3.0` 格 Chebyshev 半径（工作台）做最终授权。结果是**client 会稳定挑中并 dispatch 一个“可交互”候选，但 server 随后稳定拒绝**。这不是单点 typo，而是同一套路由上的系统性 drift。

### 复现路径

1. **工作台**
   - 对着工作台 marker，站到 **server 判定刚出 3 格、但离 marker 仍远小于 5 格** 的位置，比如 `dx≈3.1, dy≈0, dz≈0`。
   - 按 `G`。
   - client 侧 `WorkbenchInteractIntentHandler` 仍会生成 candidate 并发 `workbench_open` C2S；server `handle_workbench_interact` 用 `is_within_workbench_range`（Chebyshev ≤ 3）拒绝，UI 不会打开。
   - 证据：`client/src/main/java/com/bong/client/craft/WorkbenchInteractIntentHandler.java:23-46`、`server/src/craft/workbench.rs:73-81,129-137`。
2. **货箱 / 死信箱**
   - 对着 `TRADE_CRATE` / `HERB_CRATE_PLACED` / `DEAD_DROP_BOX`，站到 **4.5~5.0 格** 之间。
   - 按 `G`。
   - client 侧 `ContainerOpenIntentSupport` 仍 dispatch `container_open`；server `handle_container_open` 因 `dist > 4.5` 拒绝，并给出“`[容器] 离得太远。`”。
   - 证据：`client/src/main/java/com/bong/client/inventory/ContainerOpenIntentSupport.java:15-45,48-58`、`server/src/world/container_open.rs:26-27,95-102`。
3. **物资棺**
   - 对着供应棺 marker，站到 **4.5~5.0 格** 之间。
   - 按 `G`。
   - client 侧 `SupplyCoffinInteractIntentHandler` 仍 dispatch `supply_coffin_open`；server `handle_supply_coffin_interact` 因 `dist > 4.5` 拒绝。此路径默认**无 chat 反馈，只在日志里记 reject**，体感比普通容器更像“按键没反应”。
   - 证据：`client/src/main/java/com/bong/client/inventory/SupplyCoffinInteractIntentHandler.java:25-52,55-66`、`server/src/supply_coffin/interact.rs:37-38,94-101`。

### 根因链路

1. `InteractionKeybindings` 每次按 `G` 都走 `InteractKeyRouter.global().route(client)`，由各 `IntentHandler` 本地先做 candidate 预判。
   - 证据：`client/src/main/java/com/bong/client/input/InteractionKeybindings.java:29-38`、`client/src/main/java/com/bong/client/input/InteractKeyRouter.java:34-59`。
2. 三条实体交互 handler 各自硬编码了 client gate：
   - 工作台：`MAX_INTERACT_DISTANCE_SQ = 5.0 * 5.0`
   - 世界容器：`MAX_INTERACT_DISTANCE_SQ = 5.0 * 5.0`
   - 物资棺：`MAX_INTERACT_DISTANCE_SQ = 5.0 * 5.0`
3. server 再次授权时却分别使用了**不同常数、不同距离形状**：
   - 工作台：Chebyshev `<= 3.0`
   - 世界容器：欧氏 `<= 4.0 + 0.5`
   - 物资棺：欧氏 `<= 4.0 + 0.5`
4. 因 client 没有复用 server 侧同一 helper，也没有一条 pin 测试要求“client candidate 边界必须与 server accept 边界一致”，所以 drift 长期未被拦住。
   - 反证材料：
   - server 对工作台边界已经有真测试，明确 `3.1 blocks on x axis should be out of range`：`server/src/craft/workbench.rs:388-397`。
   - 但 client/worktree 现有测试只覆盖 `kind` / label parse / `client==null`，没有任何“3.1 格必须不出 candidate”的边界 pin：`client/src/test/java/com/bong/client/craft/WorkbenchInteractIntentHandlerTest.java:15-71`、`client/src/test/java/com/bong/client/inventory/WorldContainerInteractIntentHandlerTest.java:13-88`。

### 影响面

- **工作台**
  - `client`：`WorkbenchInteractIntentHandler`
  - `server`：`client_request_handler` → `WorkbenchOpenRequest` → `handle_workbench_interact`
- **世界容器**
  - `client`：`StorageCrateInteractIntentHandler` / `DeadDropInteractIntentHandler` / `ContainerOpenIntentSupport`
  - `server`：`client_request_handler` → `ContainerOpenRequest` → `handle_container_open`
- **物资棺**
  - `client`：`SupplyCoffinInteractIntentHandler`
  - `server`：`client_request_handler` → `SupplyCoffinOpenRequest` → `handle_supply_coffin_interact`
- **不在本骨架范围**
  - NPC 交互当前 server 上限是 `6.0`，本轮未看到同类“client 5 / server 6”反向拒绝症状，因此不把 NPC 一并打包。

## 这个 bug 对实际游玩体验的影响

- 玩家会在**准星已经明确命中模型**时，反复按 `G` 却只得到“没反应”或“离得太远”的结果。
- 体感上这不是“站位差一点”，而是**交互系统本身不可信**：模型、准星、按键提示和 server 真正接受的范围彼此不一致。
- 对工作台和物资棺尤其糟，因为 reject 默认没有客户端显式提示，玩家只能靠试错继续往前蹭，像在和 invisible wall / ghost range 打架。
- 这会直接恶化 Bong 当前强调的 Marker + C2S 实体交互范式；用户学到的不是“对准并按键”，而是“必须贴得比视觉上合理的距离更近，且有些对象会静默吞键”。

## 修复建议

1. **统一 source of truth**
   - 把实体交互距离 gate 收敛成共享 helper / 共享常数，不允许 client 再手写 `5.0 * 5.0` 魔法数。
2. **先对齐 server，再决定 client 体验**
   - 工作台 client 候选必须对齐 `is_within_workbench_range`（Chebyshev 3）。
   - 世界容器 / 物资棺 client 候选必须对齐 `4.5` 欧氏距离，或把 server 改到同一视觉半径，但两端必须完全一致。
3. **补双端 pin 测试**
   - client：边界内出 candidate，边界外不出 candidate。
   - server：边界内 accept，边界外 reject。
   - 联动：对同一组采样点断言“client 可交互 <=> server 会接受”。
4. **顺手补反馈**
   - 即使保留 server authoritative reject，工作台/物资棺路径也应给出玩家可见反馈，避免静默吞键。

## 两轮反方裁决

> **退化说明**：本会话未提供可用 subagent / delegate 通道，无法按理想流程外包两轮对抗审查。本骨架以下两轮为**同会话本地反方裁决**，结论和驳回理由如实记录。

### 反方裁决 Round 1

- **反方论点**：这不算 bug，只是正常的 server authoritative 校验。client 先放宽一点候选范围，让 server 拒绝即可。
- **驳回理由**：
  - 这里不是“client 无条件发包，server 兜底”，而是 **client 自己已经在做 candidate gate**。既然前端预判存在，它就应与 server 授权一致，否则预判本身就是错的。
  - 相关 finished plan 已把“range 外不触发”写成目标，而不是“range 外也可触发，交给 server 拒绝”。工作台计划还明确钉了 `3.1` 格拒绝边界，说明设计目标是**前后端一致的交互边界**，不是“前端宽松，后端兜底”。
  - 工作台 / 物资棺 reject 还是**静默失败**，因此这不是健康的“权威拒绝”，而是明确的 player-facing 假交互。

### 反方裁决 Round 2

- **反方论点**：这可能只是 marker 实体中心点与方块坐标中心不完全一致的微小 epsilon；玩家真实游玩不会感知。
- **驳回理由**：
  - 工作台不是 epsilon 级偏差，而是**距离形状都变了**：client 用 5 格欧氏球，server 用 3 格 Chebyshev 立方。`dx=3.1, dy=0, dz=0` 这种 server 明确拒绝点，在 client 里仍深处可交互区，不是边缘浮动。
  - 世界容器 / 物资棺也有**稳定 0.5 格带宽**的 false-positive 区间，不是单帧采样误差。
  - 三条不同交互链同时出现同方向漂移，说明是**复制粘贴的常数设计问题**，不是单一实体 pivot 偏了半格。

## 审计来源

bug-hunt 单轮（实体交互 / 准星命中 / C2S 请求链路聚焦）。本轮只做搜索、证据收集和 skeleton，**未改源码**。高置信 real bug：统一交互路由上的 client/server 距离 gate 漂移，已给出三条可复现链路、具体 file:line、影响面和修复建议。

## Finish Evidence

### 验证结论

第一性原理复核：不信 skeleton 结论，直接读 `origin/main`（claim SHA `f3a2709ab57f5a3714a393fe64658e89cb9df605`）代码逐条核对三条链路，**全部确认为真 bug，且在认领时仍未被其他 PR 修复**：

- 工作台：client `WorkbenchInteractIntentHandler.MAX_INTERACT_DISTANCE_SQ = 5.0*5.0`（欧氏球）vs server `WORKBENCH_INTERACT_RANGE = 3.0`（Chebyshev 立方，`is_within_workbench_range`，`server/src/craft/workbench.rs:59,76-82`）。
- 世界容器：client `ContainerOpenIntentSupport.MAX_INTERACT_DISTANCE_SQ = 5.0*5.0` vs server `OPEN_RANGE_BLOCKS(4.0) + OPEN_RANGE_TOLERANCE(0.5) = 4.5`（`server/src/world/container_open.rs:26-27,99-102`）。
- 物资棺：client `SupplyCoffinInteractIntentHandler.MAX_INTERACT_DISTANCE_SQ = 5.0*5.0` vs server 同款 `4.0+0.5=4.5`（`server/src/supply_coffin/interact.rs:37-38,94-99`）。

### 落地清单

- `client/src/main/java/com/bong/client/craft/WorkbenchInteractIntentHandler.java`：新增包内静态 `isWithinInteractRange(playerX,playerY,playerZ,entityX,entityY,entityZ)`，先 floor 实体世界坐标还原方块坐标，再做 Chebyshev（各轴绝对差取最大值）比较 `WORKBENCH_INTERACT_RANGE_BLOCKS = 3.0`，替换掉原来欧氏球判定；候选优先级仍用欧氏 `distSq` 打分（不影响排序语义）。
- `client/src/main/java/com/bong/client/inventory/ContainerOpenIntentSupport.java`：常数从 `5.0*5.0` 收紧为 `4.5*4.5`（`MAX_INTERACT_DISTANCE_BLOCKS = 4.5`），新增 `isWithinInteractRange(double distSq)` 便于测试直接钉边界。
- `client/src/main/java/com/bong/client/inventory/SupplyCoffinInteractIntentHandler.java`：同款 `4.5` 收紧 + `isWithinInteractRange(double distSq)`；`candidateEntityId` 从 `private` 改为包内可见，供测试复用。
- 测试：`client/src/test/java/com/bong/client/craft/WorkbenchInteractIntentHandlerTest.java`（新增 8 条边界 pin，含镜像 server `3.1 blocks out of range` 用例 + entity +0.5 偏移真实案例）、`client/src/test/java/com/bong/client/inventory/WorldContainerInteractIntentHandlerTest.java`（新增 4 条 4.5/4.51/4.8/0.0 边界 pin）、新建 `client/src/test/java/com/bong/client/inventory/SupplyCoffinInteractIntentHandlerTest.java`（9 条用例，含同款边界 pin + `candidateEntityId` 全套）。

### 关键 commit

- `27519be3`（2026-07-12）迁移 skeleton 为 active plan。
- `278eb582`（2026-07-12）`fix(client): 实体交互 client/server 距离判定对齐，消除假交互带` —— 三处常数/判定形状对齐 + 饱和边界 pin 测试。

### 测试结果

- `cd client && ./gradlew test --tests "com.bong.client.craft.WorkbenchInteractIntentHandlerTest" --tests "com.bong.client.inventory.WorldContainerInteractIntentHandlerTest" --tests "com.bong.client.inventory.SupplyCoffinInteractIntentHandlerTest"` → 35+10+9 = 54 个测试全绿，0 failures/errors/skips。
- `cd client && ./gradlew test build` → BUILD SUCCESSFUL（全量单测 + assemble + check）。

### 跨仓库核验

- **client**：`WorkbenchInteractIntentHandler.isWithinInteractRange` / `ContainerOpenIntentSupport.isWithinInteractRange` / `SupplyCoffinInteractIntentHandler.isWithinInteractRange` 三个新 gate helper，均已被对应 `candidate()` 方法接线消费（非孤岛）。
- **server**：本轮未改 server 代码——server 侧判定（`is_within_workbench_range` / `OPEN_RANGE_BLOCKS+OPEN_RANGE_TOLERANCE`）本就正确，只是 client 候选预判与其失配，修复只在 client 收紧对齐。
- 对抗验证：无上下文 read-only validator（Explore agent）对 worktree HEAD `278eb5828c517a0f6ad1b9a652341d9af4817c47` 独立复核 Chebyshev+floor 数学、常数对齐、测试非重言式、无残留孤岛常数、架构合规（Marker+C2S，未引入 vanilla InteractEntityEvent），结论 `PASS 278eb5828c517a0f6ad1b9a652341d9af4817c47`。

### 遗留 / 后续

- `CoffinEnterIntentHandler`（延寿棺“躺入”交互，`6.0` 半径）与 `TsyContainerSearchIntentHandler`（`5.0` 半径，末法残土搜索机制）本轮明确排除在外——前者对应 server NPC 同款 `6.0` 上限本轮未见反向拒绝症状，后者是独立子系统，均按 skeleton 原始范围界定不纳入本 plan。
- 物资棺 reject 目前仍无 client 可见反馈（server 只记日志），skeleton「修复建议 #4」的“顺手补反馈”未在本轮实施——范围收窄到纯距离判定对齐（P0 标题即“距离判定漂移”），反馈补齐建议留给后续单独 plan/PR，避免本 PR 混入 UX 新增功能扩大 review 面。
