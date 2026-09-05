# plan-bot-e2e-timing-flaky-v1（骨架）

> **骨架（草案）**。一句话主题：治理 `bot-e2e(1)` 中
> `cultivation_qi_color_inspect` 与 `network_request_unknown_type` 的时序抖动，
> 以真实事件/状态锚点替代无依据的 wall-clock 等待，同时保持原有断言语义。

## 阶段总览

| 阶段 | 主题 | 路由 | 状态 |
|------|------|------|------|
| P0 | 在同一 HEAD 重现并定位等待窗口、锚点或服务端时序根因 | bughunt | ⬜ |
| P1 | 以最小 Bot harness 修复确定性等待；若根因在服务端则停下转交 | fix_pr | ⬜ |
| P2 | 连续稳定性证据、完整受影响门禁、fresh validator 与 PR review | coverage | ⬜ |

## 接入面 Checklist

- **进料**：`scripts/bot/scenarios/cultivation_qi_color_inspect.py`、
  `scripts/bot/scenarios/network_request_unknown_type.py`、`scripts/bot/bot.py` 的
  事件/状态等待原语、`scripts/bot/run_scenarios.py` 的真实 runner，以及
  `scripts/bot-e2e.sh` 的 bot-e2e(1) 调度配置。
- **出料**：两个场景继续驱动真实协议/Redis/server 链路，并以可观测事件或状态
  作为完成锚点；`scripts/bot/test_protocol.py` 或 scripts contract 测试锁住
  等待契约，CI 的场景集合与 timeout/cleanup 语义保持不变。
- **共享类型 / event**：只复用现有 `server_data` typed payload、聊天/Redis
  观察接口和 Bot wait/poll 原语；不新增 gameplay event、server seam 或旁路命令。
- **跨仓库契约**：server 仍是权威事件/状态来源；Bot 只通过现有 MC/Redis 协议
  观察；client、agent、schema 零改，除非第一性定位证明已有协议契约本身有问题，
  此时停止本 plan 并转交对应 owner。
- **worldview 锚点**：纯 CI/Bot 测试稳定性修复，不新增玩法或世界观锚点。
- **qi_physics 锚点**：不改真元/灵气物理；`cultivation_qi_color_inspect`
  只观察既有真元颜色/事件结果，不写账户或 ledger。

## P0 — 第一性定位

- 在 `origin/main` 固定 HEAD 上分别复跑两个场景，记录每次的事件时间、状态快照、
  wall-clock 等待与 server 日志锚点；不能把一次通过当作根因证据。
- 区分三类原因：窗口确实低于正常耗时分布、等待锚点选错/命中旧快照、或服务端
  异步事件与状态落地存在真实竞态。
- 对 `cultivation_qi_color_inspect` 锁定“空挥崩拳应收到
  `burst_meridian_event`”的原断言及其真实生产事件来源；对
  `network_request_unknown_type` 锁定心跳观察期后的无玩法副作用窗口及现有
  状态/事件来源。
- 若证据指向服务端生产时序或协议语义缺陷，停止改动并回报，不以 Bot 等待逻辑
  迁就 gameplay；若是测试观察锚点问题，进入 P1。

## P1 — 确定性 Bot 等待修复

- 优先使用已存在的事件/状态等待或新增最小、可复用且只读的 Bot harness 原语，
  poll 到明确锚点/状态达成，不以放大 timeout 数值掩盖竞态。
- 保留两个场景的成功、失败、负向副作用与边界断言；禁止删除断言、降低断言
  强度、扩大未知 type 的允许副作用范围，或改 server/gameplay 迁就测试。
- 若确需调整阈值，必须由多次正常耗时分布和明确余量证明，并在 evidence 中
  记录依据；不得使用后台进程、tail 或忽略失败制造假绿。
- 补充针对等待锚点的 contract/regression 覆盖，确保旧快照、缺失事件、状态未达成
  时仍然失败并保留诊断信息。

## P2 — 稳定性验收与收口

- 修改后的两个场景连续多次在同一真实入口运行并全绿，记录精确次数、每次
  `passed/failed/ignored` 与目标场景结果，以及事件/状态锚点证据；稳定性结论
  必须由等待机制的确定性解释支撑，不接受碰运气重跑。
- 运行 `bash -n`、相关 `scripts/bot/test_protocol.py` / scripts contract tests，
  以及受影响栈门禁；合并最新 `origin/main` 后对受影响内容复验。
- 对最终 HEAD 启动无上下文 read-only validator，等待当前 HEAD 的 Kody 主动
  “未发现问题”结论；PR 中文标题/body 均带完整 plan basename，body 末尾注明
  执行模型与 validator 模型。

## 非目标

- 不修改 server gameplay、事件发射顺序、网络协议、schema、client、agent、
  `scripts/e2e-redis.sh` 或 `scripts/smoke-test-e2e.sh` 的 timeout 收口。
- 不把两个 flaky 场景改成跳过、宽松负断言、固定 sleep、后台 runner、吞掉退出码
  或只观察聊天文本的假阳性测试。
- 不修改其他 plan、`R7`、生产配置或不相关 Bot 场景。

## §8 开放问题（P0 决策门前需收口）

1. 两个场景当前等待的完成/观察锚点分别是什么，是否是生产路径真正的完成信号？
2. 稳定失败是否来自 wall-clock 窗口不足，还是旧快照/事件落地顺序竞态？
3. 现有 Bot API 是否已有足够的事件/状态轮询能力，最小修复是否能完全留在
   `scripts/bot/` 场景/harness？
4. 如何用连续运行记录、锚点时间线和负向断言结果证明修复不是偶然变绿？

上述问题须在 promotion 后以代码、日志和可重复实验收口；在 §8.1 决议前不得
扩大到 server 修复或仅增加超时。
