# plan-bot-e2e-timing-flaky-v1

> **Finished（已验收）**。一句话主题：治理 `bot-e2e(1)` 中
> `cultivation_qi_color_inspect` 与 `network_request_unknown_type` 的时序抖动，
> 以真实事件/状态锚点替代无依据的 wall-clock 等待，同时保持原有断言语义。

## 阶段总览

| 阶段 | 主题 | 路由 | 状态 |
|------|------|------|------|
| P0 | 在同一 HEAD 重现并定位等待窗口、锚点或服务端时序根因 | bughunt | ✅ 2026-09-06 |
| P1 | 以最小 Bot harness 修复确定性等待；若根因在服务端则停下转交 | fix_pr | ✅ 2026-09-06 |
| P2 | 连续稳定性证据、完整受影响门禁、fresh validator 与 PR review | coverage | ✅ 2026-09-06 |

## Integration Preflight（返工补证据，2026-09-06）

- **`docs/CLAUDE.md`**：已重读防孤岛调研的 §一至§四，按要求复查正典、归档
  plan、active plan、skeleton 与 `reminder.md`；本次是已归档 plan 的 review 返工，
  只原地补 Finish Evidence，不重复 promotion 或归档。
- **`docs/worldview.md`**：对 `qi_color_inspect`、
  `network_request_unknown_type`、`bot-e2e`、`cooldown_until_ms`、`tpdim`、
  `tpzone` 执行全文检索，均无命中；本次仍是 Bot/CI 观察层修复，不新增玩法、
  境界、区域或经济锚点。
- **`docs/finished_plans/`**：当前 369 份归档；精确检索命中一个既有
  `qi_color_inspect` 协议相关的 `plan-style-vector-integration-v1`，未发现
  `network_request_unknown_type` 或本 plan 同名归属；其余相邻 Bot 覆盖 plan
  不拥有这两个场景的时序锚点，因此不并入其它 plan。
- **当前 active `docs/plan-*.md`**：当前 87 份；精确检索的
  `qi_color_inspect` 命中 `plan-refactor-c2s-gate-v1` 与父 plan
  `plan-test-layout-refactor-v1`，未命中另一个时序修复 plan；已检查
  `plan-bot-e2e-coverage-v1`、`plan-ci-redis-pull-resilience-v1` 等相邻 owner，
  均不拥有本次两个场景的等待实现。
- **`docs/plans-skeleton/` 与 `reminder.md`**：当前 159 份 skeleton；对两个场景名
  与本 plan basename 均无命中，`reminder.md` 亦无匹配待办。结论是本返工只更新
  已归档 plan 的证据，不新立 skeleton、不改其它 plan。

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

- 在基线 `origin/main=4dd7bc17e` 的真实 server 上复现：低 TPS（约 4.4 TPS）下，
  崩拳/吸灵口的固定 wall-clock 间隔会先越过 `skillbar_config.cooldown_until_ms`
  的预测时间，但服务端实际 tick 冷却尚未清除，后续 cast 被静默丢弃；实际接受的
  崩拳 tick 为 `53/123/193/263`，吸灵口为 `272/433`。
- 同一链路的空间复现确认 `/tpzone` chat 是命令确认，不是 Position 提交；同坐标
  no-op 不发新的 `pos_look`。`/tpdim` 的 `Respawn` 后还会依次发 0.001 格 pulse
  与 restore 位置帧，若只等第一帧就发远端 `/tpzone`，restore 会覆盖新位置。
- `network_request_unknown_type` 的窗口副作用来自独立 heartbeat 的五类已知
  `world_omen` VFX，而非未知请求被处理；未知同前缀 event 仍必须视为副作用。
- 结论：根因是 Bot 观察锚点/状态等待错误，不是 server gameplay 或协议语义缺陷；
  server 生产事件与位置状态均按既有契约发出，进入 P1 保持服务端不变。

## P1 — 确定性 Bot 等待修复

- `cultivation_qi_color_inspect` 每次 cast 只发送一次 `skill_bar_cast`，等待本次
  请求后的真实接受反馈与新的 `skillbar_config`；cooldown 的 Unix deadline 只作
  休眠提示，最终通过同值 `skill_bar_bind` 刷新到服务端实际下发的 `cooldown=0`。
  不重复 cast、不把拒绝伪装成成功，原有 Heavy/Intricate、全量/脱敏/静默、维度与
  距离断言均保留。
- 空间等待复用 `_zone_loot_helpers.teleport_to_zone` 与 `wait_zone_info`；最新
  `zone_info` 已表明同 zone 时保留 no-op 只等规范 chat，否则等待本次真实 zone
  transition。跨维先记录 `/tpdim` 前的权威坐标，再分别校验目标 X（tsy 为旧 X
  `+0.25`、overworld 为旧 X `-0.25`）、pulse X（目标 X `+0.001`）及 Y/Z，最后
  精确校验 restore，避免无关位置帧满足等待。未知 `world_omen` 只加入 heartbeat
  已登记的五个精确 VFX ID，未知同前缀仍判红。
- cooldown 状态刷新仍以 server 下发的非零/零 `skillbar_config` 为完成依据；
  非零返回在过期 Unix hint 后至少经过一个 `CAST_READY_POLL_INTERVAL` 退避，
  不把 wall-clock hint 当成就绪，也不向 server 洪泛同值 bind；只要连接仍活着，
  等待持续到权威状态报告 `cooldown_until_ms == 0`，适配低 TPS 的真实 tick 进度。
- 使用现有 `Bot.wait_for`、`Bot.position`、typed `server_data` 和现有同值绑定接口，
  未新增 server seam，未改 server/gameplay、client、agent、schema 或其它 plan。
- `scripts/bot/test_protocol.py` 新增 9 条回归覆盖：单次 cast、cooldown 新状态、
  resolver skill、冷却刷新与过期 hint 后退避、低 TPS 下持续跟随权威状态、规范
  zone helper/no-op、精确 Respawn 后 pulse/restore 几何及缺失初始坐标拒绝。

## P2 — 稳定性验收与收口

- 已完成六轮真实双场景连续验证：R5A 轮中两场景均 PASS（122.9s/23.0s），
  R5B 轮中两场景均 PASS（47.6s/22.3s），R5C 轮中两场景均 PASS（124.4s/23.3s），
  R5D 轮（合并 `origin/main=51fcda26e` 后）中两场景均 PASS（119.3s/22.9s）；
  此前 FZ2/FZ3 也均输出 `total=2 pass=2 skip=0 fail=0`，每轮 runner 返回码均为 0。
  R5A/R5B/R5C/R5D 的 runner 均输出 `total=2 pass=2 skip=0 fail=0` 且返回码均为 0；
  最小真实序列还核验了两次跨维的
  `Respawn → pulse pos_look → restore pos_look → 目标 zone pos_look` 顺序。
- 已通过场景等待契约回归、完整协议测试 542/542、
  `python3 -m py_compile scripts/bot/scenarios/*.py`、`bash -n scripts/bot-e2e.sh`、
  `git diff --check`；scripts contract 为 `test_all_contract` 95/95、
  `e2e_redis_hang_guard_contract_test` 5/5、`smoke_owned_artifacts_test` PASS。
- 合并主线后的返工 HEAD `bc8fb1625` 已通过无上下文 read-only
  validator（validator 模型 `gpt-5.6-luna`），结论绑定该 SHA；此前 `4dfece34e9dc0937c19821f3c49338fc73d207f4` 的
  validator 证据属于返工前 HEAD，不作为本轮结论；`ba2e5e03f` 的返工 validator
  结论也不替代合并后新 SHA 的复验；提交 push、PR 的
  CI/e2e 与 Kody 主动 review 是 PR 阶段事项。

## 非目标

- 不修改 server gameplay、事件发射顺序、网络协议、schema、client、agent、
  `scripts/e2e-redis.sh` 或 `scripts/smoke-test-e2e.sh` 的 timeout 收口。
- 不把两个 flaky 场景改成跳过、宽松负断言、固定 sleep、后台 runner、吞掉退出码
  或只观察聊天文本的假阳性测试。
- 不修改其他 plan、`R7`、生产配置或不相关 Bot 场景。

## §8.1 决议

1. 完成锚点是本次请求后的 typed 接受反馈与权威 `skillbar_config`，以及根据
   `/tpdim` 前权威坐标计算出的目标/pulse/restore `pos_look`；固定 sleep 与仅
   chat/仅 Respawn、任意 `pos_look` 均不是完成信号。未知请求的观察锚是排空 join
   突发后的静默窗口，已知 heartbeat VFX 才可按精确白名单豁免。
2. 失败来自三类 Bot 观察问题：低 TPS 下固定冷却间隔误判 server 状态并可能在
   过期 hint 后洪泛刷新、`/tpzone` 自行解析配置造成契约分叉，以及
   `/tpdim` 的无关位置帧没有按目标几何过滤；不是靠增大业务断言窗口或修改
   server 玩法解决。
3. 现有 Bot API 已足够：`wait_for` 可按事件时间水位、typed payload 和坐标过滤，
   `Bot.position` 可复用既有权威镜像，同值 bind 是公开请求接口；无需新增 seam。
4. `network_request_unknown_type` **没有放宽等待预算**：`settle_s=2.0` 与
   `wait_keepalive_after` 默认 `25.0` 均未改；修复是 join 排空后取发送水位、等待
   两个新 keepalive 并进入 quiescence，且只豁免 heartbeat 已登记的五个精确
   `world_omen` ID，未知同前缀仍判副作用。原失败约 22.1s 是已知 heartbeat VFX
   被错误归因，不是把阈值从 22.1s 放宽到 23.5s；23.5/23.6s 是同一等待链路的
   实际完成耗时。
5. 稳定性证据由 R5A/R5B/R5C/R5D 四轮真实 runner 全绿（并保留 FZ2/FZ3 历史全绿记录）、
   每轮精确 `total=2 pass=2 skip=0 fail=0`、最小 transfer 事件序列，以及回归 fake 覆盖旧
   帧/no-op/缺失状态仍失败共同构成，非一次偶然重跑。

## Finish Evidence

- **落地清单**：
  - `scripts/bot/scenarios/cultivation_qi_color_inspect.py`：以本次请求后的
    `skillbar_config` 权威冷却、规范 zone helper、按旧坐标计算的目标/pulse/restore
    几何状态替代固定间隔、重复 zone 解析与任意位置水位；保留原有 6 次真实施放及
    全部 qi-color 正/负向断言。
  - `scripts/bot/scenarios/_rejection_helpers.py`：仅登记 heartbeat 的五个精确
    `bong:world_omen_*` VFX 为 ambient，未知同前缀仍判定为玩法副作用。
  - `scripts/bot/test_protocol.py`：新增等待契约回归测试，覆盖单次 cast、冷却状态、
    同值刷新退避与低 TPS 持续等待、规范 zone helper/no-op、精确位置几何、缺失
    坐标错误分支与未知 omen 白名单边界。
  - 未修改 server gameplay、client、agent、schema、Cargo、R7 或其它 plan。
- **关键 commit**：
  - `fcc078226`（2026-09-06）：创建 `plan-bot-e2e-timing-flaky-v1` 骨架。
  - `c574b8426`（2026-09-06）：将骨架 promotion 为 active plan。
  - `0bc6effc1`（2026-09-06）：修复 Bot 场景冷却、位置与环境 VFX 等待锚点。
  - `4dfece34e`（2026-09-06）：记录 P0/P1 决议与 P2 稳定性证据。
  - `ba2e5e03f`（2026-09-07）：按 review 补强权威冷却、zone 契约与跨维几何等待。
  - `bc8fb1625`（2026-09-07）：合并最新 `origin/main=51fcda26e` 并复验受影响栈。
- **测试结果**：
  - 真实 runner R5A：`cultivation_qi_color_inspect` PASS 122.9s，
    `network_request_unknown_type` PASS 23.0s，`total=2 pass=2 skip=0 fail=0`，rc=0；
    evidence 位于 `.sisyphus/evidence/bot-e2e/session.5oCEI5o0Nc/run.znaT7Jcits/`。
  - 真实 runner R5B：`cultivation_qi_color_inspect` PASS 47.6s，
    `network_request_unknown_type` PASS 22.3s，`total=2 pass=2 skip=0 fail=0`，rc=0；
    evidence 位于 `.sisyphus/evidence/bot-e2e/session.Az9n7OwvQb/run.EMGu6HJwCe/`。
  - 真实 runner R5C：`cultivation_qi_color_inspect` PASS 124.4s，
    `network_request_unknown_type` PASS 23.3s，`total=2 pass=2 skip=0 fail=0`，rc=0；
    evidence 位于 `.sisyphus/evidence/bot-e2e/session.frfd9pAAMb/run.GXPIm2dogz/`。
  - 真实 runner R5D（合并主线后）：`cultivation_qi_color_inspect` PASS 119.3s，
    `network_request_unknown_type` PASS 22.9s，`total=2 pass=2 skip=0 fail=0`，rc=0；
    evidence 位于 `.sisyphus/evidence/bot-e2e/session.wrdhCpJAIo/run.ALEmXT68dO/`。
  - 此前真实 runner FZ2/FZ3 均为双场景 PASS、`total=2 pass=2 skip=0 fail=0`、rc=0。
  - `python3 scripts/bot/test_protocol.py`：542 tests OK；场景等待契约回归已覆盖
    非零状态退避、低 TPS 持续轮询、规范 zone helper 与精确 pulse/restore 几何。
  - `python3 -m py_compile scripts/bot/scenarios/*.py`、`bash -n scripts/bot-e2e.sh`、
    `git diff --check`：全部通过。
  - `bash scripts/tests/test_all_contract_test.sh`：95 passed / 0 failed；
    `python3 scripts/tests/e2e_redis_hang_guard_contract_test.py`：5 tests OK；
    `bash scripts/tests/smoke_owned_artifacts_test.sh`：PASS。
- **跨仓库核验**：server 侧命中既有 `handle_tpzone`、`DimensionTransferRequest`、
  `skillbar_config` 与 `bong:server_data` 权威事件/状态；client、agent、schema
  均无本次变更命中，协议观察仍经现有 MC/typed payload 链路。
- **遗留 / 后续**：无 gameplay 或协议修复遗留；PR 阶段继续等待 CI/e2e 与 Kody
  对当前 HEAD 的主动结论，若 review 无意见则由调度会话按任务卡收口。
