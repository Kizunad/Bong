# plan-bughunt-cl-ningmai-meridian-target-drop-v1（骨架）

> **骨架（草案）**。一句话主题：`ningmai_powder` 的“外敷（选经脉）”真实消费链把 `ApplyPillTargetV1::Meridian` 整段丢弃，导致**选中的经脉完全不生效**；更严重的是实际 `MeridianHeal` 会对**所有裂痕经脉同时推进 healing_progress**，一包药变成全图群疗。

## 总览

| 阶段 | 主题 | 路由 | 状态 |
|------|------|------|------|
| P0 | `ningmai_powder` 选经脉目标丢失，外敷 UI 与 runtime 语义分叉 | fix_pr | ⬜ |

## P0 — `ningmai_powder` 选经脉目标丢失，且运行时变成全经脉群疗

- **#1 major（fix_pr）**：client 明确把 `ningmai_powder` 走“外敷（选经脉）”链路：`InspectScreen.dispatchApplyPillMeridianToChannel()` 会把用户点选的经脉编码进 `sendApplyPill(instanceId, MeridianTarget(...))`，并且右键菜单文案就是“外敷（选经脉）”（`client/src/main/java/com/bong/client/inventory/InspectScreen.java:2904-2940`）。schema 也为此专门定义了 `ApplyPillTargetV1::Meridian { meridian_id }`，`ClientRequestV1::ApplyPill` 把 `target` 设为必填（`server/src/schema/client_request.rs:24-31,391-395`）。
- 但 server 入口 `handle_apply_pill` 把参数直接命名成 `_target`，随后仅按 `instance_id -> template_id` 转发到 `handle_alchemy_take_pill(...)`，**没有任何地方读取用户选中的 meridian_id**（`server/src/network/client_request_handler.rs:11208-11244`）。
- 最终真正执行 `ItemEffect::MeridianHeal` 的 `apply_item_effect()` 也再次把模板自带的 `target` 丢成 `target: _`，并在注释里写明“**不区分 target = any_meridian vs 具体 meridian id**”；代码实际遍历 `for m in meridians.iter_mut()` 再遍历每条经脉上的全部 `cracks`，对每条裂痕都加一次 `magnitude`（`server/src/network/cast_emit.rs:457-499`）。因此现网行为不是“外敷选中的一条经脉”，而是：
  - ① 用户在 UI 里点哪条经脉都没区别；
  - ② 只要角色有多条经脉带裂痕，一次 `ningmai_powder` 会同时推进所有裂痕的 `healing_progress`；
  - ③ `ningmai_powder` 物品文案“外敷经脉”与模板语义“meridian_heal + any_meridian”（`server/assets/items/pills.toml:25-35`）被 runtime 放大成“全经脉 AoE 治疗”，实际资源效率高于 UI/世界观承诺。

## 复现路径

1. 准备至少 1 个 `ningmai_powder`，并让角色同时拥有两条以上带裂痕的经脉（例如 A、B 两条都未愈合）。
2. 打开 `InspectScreen`，右键 `ningmai_powder`，选择“外敷（选经脉）”。
3. 在经脉层明确点选 A，经 client 发出 `apply_pill { instance_id, target:{ kind:\"meridian\", meridian_id:A } }`。
4. 观察 server 结果：A 并不会被单独治疗；`apply_item_effect(ItemEffect::MeridianHeal)` 会遍历全部经脉裂痕，把 A/B 等所有未愈合裂痕都推进同样的 healing_progress。
5. 若把第 3 步改成点 B，结果与点 A 相同，证明 UI 目标选择对 runtime 无效。

## 根因链路

1. **协议层有目标**：`ApplyPillTargetV1`/`ClientRequestV1::ApplyPill` 已承载 `meridian_id`。
2. **客户端真发目标**：`dispatchApplyPillMeridianToChannel()` 把选中的 `MeridianChannel` 编码进 `MeridianTarget`。
3. **network handler 丢目标**：`handle_apply_pill(..., _target, ...)` 未透传也未缓存目标。
4. **runtime 再次忽略目标**：`apply_item_effect(ItemEffect::MeridianHeal { target:_ })` 对“模板 target”与“请求 target”都不分流。
5. **算法错误扩大影响**：实现不是“命中一条经脉/一条裂痕”，而是“遍历所有 meridian + 所有 cracks”，把一次外敷放大成全经脉治疗。

## 这个 bug 对实际游玩体验的影响

- 玩家会被 UI 明确误导：界面要求“选经脉”，但实际点哪条都一样，决策完全是假动作。
- `ningmai_powder` 在多裂痕场景下会超规格生效，一次消耗覆盖全部受损经脉，直接破坏经脉修复的资源权衡。
- 后续若围绕“定点修某条关键经脉”设计 build / 教学 / 经济定价，当前实现会让这些内容全部失真。

## 修复建议

1. `handle_apply_pill` 不再丢弃 `target`，把 `ApplyPillTargetV1` 明确透传到消费侧。
2. `MeridianHeal` 运行时新增“目标经脉解析”路径：
   - `SelfTarget`/无目标：要么拒绝 `ningmai_powder`，要么按明确 fallback 语义处理；
   - `Meridian { meridian_id }`：只允许命中该经脉；
   - 模板 `target = "any_meridian"` 仅表示“该物品支持任意经脉作为合法目标”，**不是**“自动治疗全部经脉”。
3. 回归测试必须锁住“指定 A 不会治疗 B”“切换目标会改变结果”“单次消耗只推进一条目标经脉”的行为。

## 验收抓手

- server 单测：
  - `apply_pill` 传 `Meridian { Ren }` 时，仅 `Ren` 裂痕变化，其他经脉不变。
  - 同一角色存在多条裂痕时，单次 `ningmai_powder` 不得同时推进两条经脉。
  - `SelfTarget` 对 `ningmai_powder` 的行为必须被明确 pin（拒绝或 fallback，但不能静默群疗）。
- client/协议：
  - `InspectScreen` 右键“外敷（选经脉）”后，切换不同目标经脉应产生不同 server 结果。
  - `ApplyPillTargetV1::Meridian` 端到端 round-trip 保持 `meridian_id` 不丢。
- 手测：
  - 两条经脉各留 1 条裂痕，连续两次分别点 A / B；应看到每次只治疗被点中的那一条。

## 去重检查

- 已检索 `docs/plans-skeleton` 与 `docs/finished_plans`，未发现现成题目命中 `ningmai_powder` / `ApplyPillTargetV1` / “外敷（选经脉）被忽略”。
- 与用户明确排除的既有题（多臂副手、NPC 交易 bundle 数量桥、战斗丹毒门禁、orphan `pack_<id>` 脏档）均无重叠。

## 反方裁决（退化处理：本会话未开 subagent，改为同会话两轮反方审查并如实记录）

### Round 1

- **反方论点**：这可能只是老注释/旧 UI 残留；`ningmai_powder` 也许本来就设计成“任意选一条作为交互占位，实际治疗全部经脉”。
- **驳回理由**：
  - client 不是仅做装饰文案，而是实际发送 `MeridianTarget(...)`；
  - schema 把 `target` 设成 `ApplyPill` 必填字段，说明协议设计就是要把目标交给 server；
  - server 代码注释直接承认“具体 meridian id 后续再细化”，这不是既定玩法，而是未接完的 runtime 缺口。

### Round 2

- **反方论点**：即便忽略用户目标，也许当前实现只是“治疗第一条裂痕”，影响有限，不足以立 major。
- **驳回理由**：
  - 真实代码不是“第一条裂痕”，而是双重循环遍历全部经脉与全部裂痕，对每条裂痕都加 healing_progress；
  - 因此问题不只是“目标被忽略”，还是“单次 consumable 被错误放大成全经脉群疗”，会直接改变资源效率与修复节奏，达到 major。

## 审计来源

- bughunt 线程 CL，本地静态核查。
- 受限说明：本轮未再开 subagent，按用户允许的退化方案，在 skeleton 内记录两轮反方裁决、反方论点与驳回理由。
