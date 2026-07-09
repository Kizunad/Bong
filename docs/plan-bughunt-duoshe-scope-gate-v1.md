# BugHunt: 夺舍请求缺少服务端距离与维度门禁

> server-gameplay bughunt r10。只记录问题与修复计划；本 PR 不改代码。

## 一句话

`duo_she_request` 是公开 C2S 协议，但服务端收到 `target_id` 后只按身份字符串解析目标并直接执行夺舍，缺少宿主与目标的同维、同层、距离、视线或交互态校验；能发包的客户端可以远程甚至跨维夺舍任意符合资格的 NPC / 玩家肉身。

## 实际游玩体验影响

夺舍是寿元系统里的高代价换壳玩法，本应是近距离、高风险、可被目标或旁观者感知的互动。当前服务端权威路径没有 scope gate 后，任何能发送公开 `bong:client_request` 的客户端或 bot，只要知道可夺舍目标的 `character_id` / `lifecycle.character_id` / `canonical_npc_id`，就可能从安全地点直接换到目标身体，并继承目标坐标、维度和 layer。

玩家体验上，这会把“寻找凡人/醒灵目标并冒险接近”的玩法压扁成远程菜单/脚本操作：目标可能在 TSY、远处聚落或另一个维度，宿主仍可绕过空间风险完成夺舍。受害目标会被标记为 `PossessedVictim + Despawned`，宿主瞬移到目标位置，属于真实角色状态变化，不是 UI 显示问题。

普通 Java UI 入口目前只找到 `ClientRequestSender.sendDuoSheRequest(...)` / `ClientRequestProtocol.encodeDuoSheRequest(...)`，尚未找到生产 screen 按钮调用；因此本 bug 的最小触发面是公开协议/自定义客户端/bot。但 AGENTS.md §15 要求 server 最大化宽容并防御任意 `bong:client_request` 输入，新 C2S 输入面不能依赖客户端自律。

## 证据

1. C2S schema 公开暴露 `DuoSheRequest { target_id }`，没有坐标、目标 entity、交互 session 或 scope token：`server/src/schema/client_request.rs:414-417`。

2. 网络层只把 `target_id` 原样转成领域事件，没有在入口解析目标、校验距离/维度/同层：
   - `server/src/network/client_request_handler.rs:2007-2013`
   - `DuoSheRequestEvent { host: ev.client, target_id }`

3. 领域处理只先查冷却，再调用 `resolve_target_snapshot(...)`：
   - `server/src/cultivation/possession.rs:133-157`
   - 没有读取 host position/dimension 后与 target 对比的逻辑。

4. `resolve_target_snapshot(...)` 只按身份匹配：
   - `life_record.character_id == target_id`
   - `lifecycle.character_id == target_id`
   - `canonical_npc_id(entity) == target_id`
   - 见 `server/src/cultivation/possession.rs:350-397`

5. 成功后 `inherit_host_runtime_body(...)` 会把宿主改到目标位置、维度和 layer：
   - `position.set(target_position)`：`server/src/cultivation/possession.rs:419-427`
   - `current_dimension.0 = inherited_dimension`：`server/src/cultivation/possession.rs:435-437`
   - layer / visible layer 切换：`server/src/cultivation/possession.rs:439-454`

6. 现有测试还把跨维继承锁成 happy path，而不是拒绝：`process_duo_she_inherits_target_position_and_dimension` 中 host 在 Overworld，target 在 TSY，更新后断言 host 变成 TSY，见 `server/src/cultivation/possession.rs:586-660`。

## 非重复性检查

- 不重复 #981 / 炼丹炉 scope gate：本题是寿元/夺舍 C2S。
- 不重复 #1014 / 玩家交易跨维：本题不是交易换货，而是角色换壳与目标 despawn。
- 不重复 #1007 / 掉落物跨维拾取、#1022 / 灵田 C2S、#1073 / 制作台、#1088 / 普通延寿棺、#1101 / 阵法布置：都是其他 gameplay 入口。
- `docs/plans-skeleton` 中仅检出与夺舍相关的 Woliu 垂死大能运行态、dying elder、life span 已完结计划；未见“duo_she_request 距离/维度门禁”同题 plan。

## 修复计划

- [ ] TODO(server): 在 `DuoSheRequestEvent` 处理前或 `process_duo_she_requests` 内加入 `host_scope` 校验：host 与 target 必须同 `CurrentDimension`、同 `EntityLayerId`，并在夺舍交互半径内。
- [ ] TODO(server): 对缺少 `Position` / `CurrentDimension` / `EntityLayerId` 的生产请求保守拒绝；仅允许单测通过显式 helper 构造无空间上下文的内部事件。
- [ ] TODO(server): 增加拒绝反馈事件或 narration，避免玩家发起后无声失败。
- [ ] TODO(test): pin 三类负例：远距离同维拒绝、同坐标跨维拒绝、缺空间组件拒绝。
- [ ] TODO(test): 保留合法近距同维 happy path，并确认 `qi_max` 截断释放守恒测试仍通过。
- [ ] TODO(bot-e2e): 新增 `duoshe_scope_gate` 场景，用 `bong:client_request` 直接发送跨维/远距 `duo_she_request`，断言不会换壳、不会 despawn 目标。

## 对抗结论

Round 1 反方先审出炼丹炉 scope gate 候选成立但重复 #981，因此弃用。

Round 2 反方继续确认炼丹炉候选与 `docs/plans-skeleton/plan-bughunt-alchemy-furnace-scope-gate.md` 精确重复。

Round 3 反方审查本候选：PASS。反方确认 `DuoSheRequest` 公开协议只带 `target_id`，server handler 直接 emit `DuoSheRequestEvent`，`resolve_target_snapshot` 只按身份匹配，成功后会改 host 位置/维度并标记 target `PossessedVictim + Despawned`。未发现 #969-#1101 或 `docs/plans-skeleton` 精确覆盖。

本候选的最强反驳是：普通 Java UI 暂未找到生产调用，`sendDuoSheRequest` 只是公开 sender；但 server 已注册 Rust/Java/agent schema 公开协议，且 AGENTS.md §15 明确要求新增 C2S 输入面必须防御 bot/坏 payload。服务端权威不能把“当前 UI 不发”当作空间门禁。
