# BugHunt: 普通延寿棺跨维裸坐标门禁

## 结论

普通延寿棺 `coffin` 的 `CoffinPlace` / `CoffinEnter` / `CoffinBreak` / `CoffinMenuReclaim` 链路只携带并校验裸 `BlockPos`，没有携带或校验玩家 `CurrentDimension`。玩家处在坍缩渊或其它非主世界维度时，只要数值坐标接近主世界某口棺，就可能通过 C2S 请求操作主世界 `CoffinRegistry` 与 `OverworldLayer` 上的棺。

这不是物资棺 `supply_coffin` 的跨维 session gate，也不是制作台 / 灵龛的同类门禁；这里的对象是普通延寿棺自身的注册表、marker、进棺、破坏与回收链路。

## 实际游玩体验影响

玩家在异维同坐标附近可以触发主世界延寿棺副作用：普通客户端手持 `mundane_coffin` 右键当前维度方块即可发送 `coffin_place`，服务端会消耗随身棺材物品却把棺注册并生成到主世界。`coffin_enter` / `coffin_break` / `coffin_menu_reclaim` 的正常客户端准星入口通常受 marker 可见性限制，但陈旧菜单、bot、调试包或伪造 `bong:client_request` 仍可钻入、破坏或回收主世界棺。表现上就是“人在坍缩渊，动到了主世界家里的棺”，会破坏玩家对维度隔离、领地资产和风险范围的直觉。

## 证据

1. 请求结构没有维度字段。

   `server/src/coffin/mod.rs:251-293` 定义 `CoffinPlaceRequest`、`CoffinEnterRequest`、`CoffinMenuReclaimRequest`、`CoffinBreakRequest`，字段只有 `player`、`pos`、`tick`、物品实例 id，没有 `CurrentDimension` 或 layer id。

2. 网络层只把 JSON 坐标转成 `BlockPos` 并发事件。

   `server/src/network/client_request_handler.rs:930-1061` 对 `coffin_place`、`coffin_enter`、`coffin_break`、`coffin_menu_reclaim` 只读取 `x/y/z`，构造 `BlockPos::new(x, y, z)` 后发送对应事件。这里没有读取玩家当前维度，也没有拒绝非主世界玩家。

3. 放置链路使用主世界 layer。

   `server/src/coffin/mod.rs:419-436` 的放置处理查询 `OverworldLayer`，玩家查询只取 `Position` 而不取 `CurrentDimension`。`server/src/coffin/mod.rs:449-495` 只做裸坐标近距与主世界方块空位检查；`server/src/coffin/mod.rs:497-531` 随后消费玩家物品、写入 registry，并在 `OverworldLayer` 生成 marker。

4. 进棺链路只查全局 registry 和裸坐标距离。

   `server/src/coffin/mod.rs:571-625` 先 `registry.lookup(event.pos)`，再用 `coffin_target_is_close(&position, event.pos)` 校验距离；成功后直接 `position.set(coffin_player_position(coffin.lower))` 并插入 `CoffinComponent`。玩家维度不参与校验。

5. 破坏与菜单回收链路同样只用裸坐标。

   `server/src/coffin/mod.rs:704-760` 的 `handle_coffin_breaks` 只校验玩家实体存在和 `coffin_target_is_close(position, event.pos)`，随后 `registry.remove_by_pos(event.pos)` 并 despawn marker。

   `server/src/coffin/mod.rs:833-889` 的 `handle_coffin_menu_reclaim` 先 `registry.lookup(event.pos)`，再按 `coffin.lower` 做裸坐标近距，随后 `registry.remove_by_pos(coffin.lower)` 并 despawn marker。

6. C2S 路径可达，且协议本身没有维度。

   `client/src/main/java/com/bong/client/mixin/MixinClientPlayerInteractionManagerAlchemy.java:118-123` 会在手持 `mundane_coffin` 右键方块时发送 `coffin_place`，这是普通客户端在异维也能自然触发的入口。同文件 `:47-54` 会从 marker 攻击发送 `coffin_break`；`client/src/main/java/com/bong/client/coffin/CoffinMenuScreen.java:77-87` 会发送 `coffin_enter` 与 `coffin_menu_reclaim`。后三者的正常 UI 入口可能受异维 marker 可见性收窄，但服务端授权仍不能依赖客户端可见性。`client/src/main/java/com/bong/client/network/ClientRequestProtocol.java:440-485` 四类 payload 只编码 `x/y/z` 与必要物品实例 id，没有维度。

## 复现草案

1. 在主世界坐标 `P` 放置一口普通延寿棺，确保 `CoffinRegistry` 中存在 `P` / upper half。
2. 让玩家进入坍缩渊或任意非主世界维度，站到与 `P` 数值距离足够近的位置。
3. 发送 `{"type":"coffin_enter","v":1,"x":P.x,"y":P.y,"z":P.z}`，或发送 `coffin_break` / `coffin_menu_reclaim`。
4. 预期：服务端应因玩家不在主世界而拒绝。
5. 实际：当前代码路径只用裸坐标距离与全局 registry，可能进入、破坏或回收主世界棺。

普通客户端直触路径可用 `coffin_place` 复现：玩家在异维手持 `mundane_coffin` 右键当前维度方块，客户端会发送当前维度裸坐标；服务端会按同一裸坐标检查 `OverworldLayer` 并写入主世界 registry。

## 去重

- 不是 #1060 / `docs/plans-skeleton/plan-bughunt-supply-coffin-cross-dimension-session-gate-v1.md`：那是物资棺 `supply_coffin` 外部容器 session gate；本报告是普通延寿棺 `coffin` 的注册表、marker、进棺、破坏、回收链路。
- 不是 #1073 或 #1004：那些是制作台跨维打开 / 误拆；本报告不涉及 crafting table。
- 不是 #973：那是坍缩渊灵龛放置维度门禁；本报告对象不是 spirit niche。
- 不是 #1048 / #1034 / #1039：本报告不是满包吞产物。
- 不属于 server-qi：没有新真元守恒公式或 ledger 缺口，只是 gameplay 交互门禁缺失。

## 修复方向

- [x] 在普通延寿棺 C2S 入口或事件消费端强制读取玩家 `CurrentDimension`。
      **落地**：选**事件消费端**（非网络层入口）——服务端本就持有权威维度，客户端上报的维度不可信；
      且消费端能统一覆盖非 UI 来源（bot / 伪造 payload / 陈旧菜单）。四个 handler 的玩家查询各补
      `Option<&CurrentDimension>`（`server/src/coffin/mod.rs` place/enter/break/menu_reclaim）。
- [x] 需要操作 `OverworldLayer` / `CoffinRegistry` 的请求必须要求玩家当前维度为主世界；否则拒绝并下发反馈。
      **落地**：`coffin_requires_overworld`（`server/src/coffin/mod.rs:1301-1303`）=
      `matches!(dimension, Some(CurrentDimension(DimensionKind::Overworld)))`，组件缺失落 `_` 分支
      → **fail-closed 拒绝**（不隐式当作主世界）。四条链路的校验均在**产生副作用之前**（place 在
      消费物品 / `registry.insert` 之前；enter 在 `set_occupied` 之前；break 在 `remove_by_pos` 之前；
      menu_reclaim 在 `lookup` 之前）。拒绝复用既有 `client.send_chat_message` 回执惯例，未另造通道。
      **未复用 `player/home_return.rs:96 fn is_overworld`**：那个是 fail-open（`unwrap_or_default()`
      缺失时当主世界）且模块私有，复用会把 fail-open 语义带进安全门禁。
- [x] 回归测试应覆盖 `coffin_place`、`coffin_enter`、`coffin_break`、`coffin_menu_reclaim` 四类请求在非主世界同裸坐标附近被拒绝。
      **落地**：四类各配**三侧**共 12 条——放行（主世界 + 近距）/ 跨维拒绝（Tsy + 同裸坐标）/
      维度组件缺失拒绝；拒绝侧一并断言 chat 回执。**只测拒绝不足以证明门禁正确**，故补齐放行侧。

## Finish Evidence

**落地清单**

- 维度门禁实现 + 四链路接线：`server/src/coffin/mod.rs`（`coffin_requires_overworld` +
  `handle_coffin_place_requests` / `handle_coffin_enter_requests` / `handle_coffin_breaks` /
  `handle_coffin_menu_reclaim` 的玩家查询与前置校验）
- 回归测试：`server/src/coffin/mod.rs` `mod tests` 内 12 条 `ecs_coffin_*`（place/enter/break/reclaim
  × allowed / rejected_in_tsy_same_numeric_pos / rejected_missing_dimension）

**关键 commit**

- `172d6614a`（2026-07-27）修复普通延寿棺跨维裸坐标门禁：四条 C2S 链路强制主世界校验

**测试结果**

- `cargo test --lib coffin` → **235 passed / 0 failed**（含既有 coffin 用例，无回归）
- 全量 `cargo test` → **11962 passed**；`cargo fmt --check` RC=0；`cargo clippy --all-targets -- -D warnings` RC=0
- **判别力自检**：把 `coffin_requires_overworld` 改为恒 `true`（门禁失效）后，**8 条拒绝用例全部撞红、
  8 条非拒绝用例保持绿**；还原后全绿。证明这批用例真正依赖门禁而非空转
- **对抗验证**：无上下文 read-only validator 对 HEAD `172d6614a` 复核 **PASS**，独立复现同一组 8 红/8 绿

**跨仓库核验**

- server：`coffin_requires_overworld` / `CurrentDimension` / `DimensionKind::Overworld` /
  `COFFIN_DIMENSION_REJECTION_MESSAGE`
- client / agent：**无改动**。本修复刻意不给 payload 增加维度字段——客户端上报的维度不可作为授权依据，
  服务端权威维度已足够；`ClientRequestProtocol.java:440-485` 四类 payload 维持只编码 `x/y/z`

**遗留 / 后续**

- 本 plan 只覆盖**普通延寿棺**。物资棺 `supply_coffin` 另有 `supply_coffin::authority` 的
  session/source 双维度比对逻辑（含 `SupplyCoffinAuthorityFailure::MissingPlayerDimension`），
  语义更复杂、不在本 plan 范围，未合并为同一实现。
- 修复期间发现的测试脆弱点（非本 plan 引入、已在本轮修掉）：`handle_coffin_place_requests` 的玩家
  查询要求 `&PlayerState`（非 Option，与 break/reclaim 的 `Option<&PlayerState>` 不一致），测试
  harness 漏插该组件会让整条 place 链路在取玩家那步早退——从而使「断言未注册」型的拒绝用例被喂成
  假绿。同类隐患已核查 enter/break/reclaim 三条（复用 `ScenarioSingleClient` 完整 ClientBundle，
  不存在）。四条链路对 `PlayerState` 必需性不一致本身可作为后续统一口径的候选。

## 对抗审查

- 第 1 轮：要求对抗 subagent 从“是否已有维度校验、是否客户端不可达、是否与 #1060/#973/#1004/#1073 重复”方向找反证。
- 第 2 轮：要求对抗 subagent 专门攻击“普通延寿棺 vs 物资棺 / 制作台 / 灵龛”的去重边界，以及客户端可见性是否能替代服务端授权。
