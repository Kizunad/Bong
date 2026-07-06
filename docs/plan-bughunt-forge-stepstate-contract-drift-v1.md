# plan-bughunt-forge-stepstate-contract-drift-v1

> 结论一句话：`forge_session.step_state` 的 server→schema→client 契约把 **tempering pattern / inscription 槽位上限 / consecration min_realm** 这三类步骤元数据丢掉或写错，导致炼器 UI 在已实现链路里出现“淬炼空轨道、双槽铭文自锁、低境界误导注入烧真元”的复合真 bug。主题明确避开已知 `forge/lingtian processing deadpath`。

## 目标 bug

- **高置信 bug（report-only）**：`server/src/network/forge_snapshot_emit.rs` 生成 `forge_session` 快照时，没有把 blueprint 里的步骤级真相源正确带到 client。
- 这不是“功能还没做”，而是**现有 server-data 契约自身漂移**：
  - `tempering.pattern` 在 schema 中存在，但 emit 时被硬编码成空数组。
  - `inscription.max_slots` 在 schema 中存在，但 emit 时被错误写成 `filled_slots`。
  - `consecration.min_realm` 在 blueprint / server 结算逻辑 / client UI 判定中都被使用，但 **Rust/TypeBox forge schema 都没定义**，client 仍在读这个字段。

## 复现路径

### 路径 A：淬炼轨道永远显示“已完成”

1. 让任一带 tempering 步的图谱进入锻造会话，例如 `qing_feng_v0`（`server/assets/forge/blueprints/qing_feng_v0.json:18-25`）或 `ling_feng_v0`（`.../ling_feng_v0.json:21-27`）。
2. server 经 `send_forge_snapshots_to_player -> build_session_data -> build_step_state` 组 `forge_session`。
3. `build_step_state` 在 `Tempering` 分支把 `pattern` 直接写成 `vec![]`（`server/src/network/forge_snapshot_emit.rs:155-164`）。
4. client `TemperingTrackComponent.renderStateFrom()` 只从 `pattern_remaining` 或 `pattern` 读节拍（`client/.../TemperingTrackComponent.java:43-53`），读到空数组后在 `drawTrack()` 直接落到“淬炼节拍已完成”（`...:81-84`）。

### 路径 B：双槽铭文在第一张残卷后自锁

1. 进入 `ling_feng_v0` 的 inscription 步；该图谱明确要求 `slots=2` 且 `required_scroll_count=2`（`server/assets/forge/blueprints/ling_feng_v0.json:29-35`）。
2. server `build_step_state` 在 `Inscription` 分支发送 `max_slots: state.filled_slots`（`server/src/network/forge_snapshot_emit.rs:166-169`）。
3. 第一张铭文残卷提交后，server 状态 `filled_slots=1`，下一帧 client 收到的 `max_slots` 也变成 1。
4. client `InscriptionPanelComponent` 用 `max_slots` 作为投放闸门；当 `filledCount() >= maxSlots()` 时直接拒绝第二张残卷（`client/.../InscriptionPanelComponent.java:70-85,106-112`）。
5. 结果：需要 2 槽的图谱在 UI 上被第一张残卷永久锁死，只能停在 partial / flawed 路径。

### 路径 C：低境界玩家会被误导去开光并真实烧掉真元

1. 进入 `ling_feng_v0` 的 consecration 步；该图谱要求 `min_realm = Spirit`（`server/assets/forge/blueprints/ling_feng_v0.json:37-42`）。
2. client `ConsecrationPanelComponent` 会读取 `min_realm` 决定按钮是否可按（`client/.../ConsecrationPanelComponent.java:57-67,247-255`）。
3. 但 Rust/TypeBox forge schema 的 `Consecration` 状态只有 `qi_injected / qi_required / color_imprint`，根本没有 `min_realm`（`server/src/schema/forge.rs:102-128`；`agent/packages/schema/src/forge.ts:99-107`）。
4. `min_realm` 为空时，client `realmAllowed()` 默认返回 `true`（`client/.../ConsecrationPanelComponent.java:247-252`），按钮保持可点。
5. server `handle_consecration_injects()` 会真实从 `Cultivation.qi_current` 扣真元并记入 zone ledger（`server/src/forge/mod.rs:429-535`），但最终 `resolve_consecration()` 仍会按 blueprint 的 `profile.min_realm` 判失败（`server/src/forge/steps.rs:230-243`）。
6. 结果：玩家在 UI 被误导为“可注入”，实际却把真元烧进一个注定失败的开光过程。

## 根因链路

1. `ForgeSession.step_state` 只存**运行中累积态**，不存 blueprint 的步骤静态元数据。
2. `forge_snapshot_emit::build_step_state()` 又没有把 blueprint profile 一并带入，因此在组包时：
   - `Tempering` 失去 `pattern`，直接塞空数组。
   - `Inscription` 拿不到 `slots/required_scroll_count`，错误地用 `filled_slots` 冒充 `max_slots`。
   - `Consecration` 拿不到 `min_realm`，而 schema 也没有这个字段。
3. client 三个组件都把这些字段当作**权威 UI 输入**，于是 contract drift 直接升级为玩家可见错误。

## 这个 bug 对实际游玩体验的影响

- 玩家做青锋/灵锋这类多步炼器时，会看到淬炼轨道一开始就显示“已完成”，失去按拍依据，体感像 UI 坏了而不是自己操作失误。
- 灵锋这类双槽铭文图谱会在第一张残卷后卡死，玩家被迫落入 partial/flawed，误以为“第二张卷轴没反应”或“背包拖放坏了”。
- 低于 `Spirit` 的玩家会被开光界面放行并真实损失真元，最后却只能得到失败结算；这是**误导性资源损耗**，体感最差。

## 影响面

- server：`server/src/network/forge_snapshot_emit.rs`
- schema：`server/src/schema/forge.rs`、`agent/packages/schema/src/forge.ts`
- client：`TemperingTrackComponent`、`InscriptionPanelComponent`、`ConsecrationPanelComponent`、`ForgeSessionHandler`
- content 命中面：
  - 所有有 tempering 步的图谱：`qing_feng_v0`、`ling_feng_v0`
  - 所有多槽 inscription 图谱：当前已知 `ling_feng_v0`
  - 所有有 consecration realm gate 的图谱：当前已知 `ling_feng_v0`

## 修复建议

1. 把 `build_step_state()` 改成可见 blueprint profile 的版本，而不是只吃 `ForgeSession`：
   - `Tempering` 回填完整 `pattern`
   - `Inscription` 回填真实 `max_slots`（建议直接用 `profile.slots`）
   - `Consecration` 回填 `min_realm`
2. 同步扩 Rust + TypeBox forge schema，把 client 实际依赖的 `min_realm` 纳入 source-of-truth，避免再出现“client 读、schema 没定义”的漂移。
3. 补 pin 测试：
   - `forge_snapshot_emit` 针对 `qing_feng_v0` 断言 tempering pattern 非空
   - 针对 `ling_feng_v0` 断言 inscription `max_slots == 2`
   - 针对 `ling_feng_v0` 断言 consecration payload 含 `min_realm = Spirit`

## 反方裁决（退化处理）

> 当前会话没有可用 subagent / delegate tool；以下两轮反方裁决为**同会话退化执行**，但分别按“不是 bug / 影响没那么大”两个方向独立复核。

### Round 1

- **反方论点**：`send_forge_snapshots_to_player()` 目前可能没被 runtime 真调用，这些字段就算错了也只是死代码，不算真实 bug。
- **驳回理由**：
  - `forge_session` / `forge_blueprint_book` / `forge_station` 全套 schema、router、store、handler、UI 都已落地，`docs/plans-progress.yaml` 还把 forge client 总结为“已实现”；该 helper 明显是既定正式桥，而不是 throwaway test stub。
  - 即便调用点稀薄，这仍是**已提交主干契约**的错误实现；一旦任何 open-screen / hydrate 路径接通，bug 立即对玩家可见，不属于纯 hypothetic。

### Round 2

- **反方论点**：这些只是 UI 信息不完整；server 结算本身还是对的，最多算 polish 问题。
- **驳回理由**：
  - tempering 不是装饰信息，`pattern` 是玩家按拍的核心输入；空轨道直接破坏玩法。
  - inscription `max_slots` 错误会实打实阻止第二张卷轴提交，是功能阻断，不是展示瑕疵。
  - consecration `min_realm` 缺失会让 client 放行注入，而 server 会真实扣 `qi_current`；这已经是**误导性资源损耗**，不是单纯 polish。

## 建议后续

- 以 `fix_pr` 处理，优先级建议 `major`：
  - 单点修 `forge_snapshot_emit + forge schema`
  - 顺手补 3 条 snapshot contract pin 测试
  - 不扩面触碰 `forge/lingtian processing` 主题
