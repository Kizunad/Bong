# plan-bot-economy-eval-v1 — 参考无头客户端挂机收益测量与经济评估

> 一句话主题：在 V 轨 `plan-bot-e2e-coverage-v1` 完成、`scripts/bot/` 升格为正式支持的参考无头客户端后，测量不同场景与并发下的每小时骨币、物品、真元收益和消耗，给匮乏经济提供可复核数据；本 plan 不预先拍规则数值，也不改玩法代码。
>
> 所属总纲：`docs/plans-skeleton/plan-refactor-master-v1.md` §9.9（RF-45，2026-09-30 已决议）。这是评估骨架，不是经济平衡实施 plan。

**状态**：骨架（skeleton）

**执行**：—

**PR**：—

## 接入面

- **前置**：V 轨 `docs/plan-bot-e2e-coverage-v1.md` 必须先完成 P5 的组队渡劫和 Agent 回流完成门；未完成时本 plan 只能做测量协议设计，不能宣称收益结论。
- **进料**：`scripts/bot/run_scenarios.py::ScenarioEnv.new_bot`（`29-43`）、`scripts/bot/bot.py::Bot` 的动作/断言、`scripts/bot/mc_protocol.py::login`（`253-275`）、V 轨已认证的生产 wire 场景、服务器 `bong:server_data` 快照和现有经济 Redis channel `bong:bone_coin_tick` / `bong:price_index`（`agent/packages/schema/src/channels.ts:30-34`）。
- **出料**：可重复的原始事件/快照样本、按场景和时间窗口聚合的收益/消耗报告、服务器成本与配额影响报告，以及不带预设数值的后续规则建议输入；不直接写入生产经济状态。
- **复用类型 / event / schema**：复用 `inventory_snapshot`、`player_state`、`bong:bone_coin_tick`、`bong:price_index`、现有 Bot scenario registry 和 CI Bot e2e stage；不新增一份货币或真元表示，不把聊天文案当经济事件。
- **跨仓库契约**：server 继续产生现有经济与库存/玩家状态；agent 仅消费已有 `PRICE_INDEX`/经济快照，不为测量新增 gameplay channel；client/参考 bot 按 `bong:server_data` 与 MC 763 wire 观察同一结果；报告脚本只读 CI/服务器采样。
- **worldview 锚点**：`worldview.md §九 L844-L892`（骨币是真元封存的、会贬值的唯一硬通货）、`worldview.md §十 L870-L910`（资源匮乏与搜打撤循环），以及 `worldview.md §一 L26-L46` 的匮乏/风险背景。测量不能把骨币当作稳定堆叠数字，也不能把真元收益与物品掉落重复计数。
- **qi_physics 锚点**：本 plan 不定义公式或常数；真元相关指标只读取现有 `qi_physics`/ledger 可观察快照，并以服务器启动时的 `qi_physics::constants::DEFAULT_SPIRIT_QI_TOTAL` 或显式总量配置对拍守恒，不在测量脚本里修改 `qi_current`、zone 灵气或账户。

## 现状证据与不重复范围

- V 轨现有 `scripts/bot/mc_protocol.py`、`bot.py`、`run_scenarios.py`、`test_protocol.py` 和 `.github/workflows/e2e.yml` 已提供协议级测试基础，但 P5 仍有组队渡劫/Agent 回流未完成；本 plan 不复制 V 的覆盖场景或 decoder。
- `plan-economy-v1` 已定义骨币 `spirit_quality`、`bong:bone_coin_tick` / `bong:price_index` 和价格指数语义；本 plan 只读取其结果，不能重写价格公式、骨币半衰或货币模型。
- 既有 bot 场景可能使用 dev 命令作为铺垫；收益评估必须把铺垫消耗、生产动作、掉落、丢弃和回收分别标记，不能把测试赠品算入每小时收益。
- 服务器成本和配额（并发连接、请求预算、loot/discard 上限、Redis/SQLite 写入）是观测维度，不是本 plan 新增的限流策略。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | 测量协议、指标字典、前置核验与可重复 fixture | ⬜ |
| P1 | 只读采样与场景计时/标签接入 | ⬜ |
| P2 | 单 bot、多 bot、不同场景和时长的测量矩阵 | ⬜ |
| P3 | 收益/消耗/成本报告与数据质量审计 | ⬜ |
| P4 | 规则决策输入、边界建议与后续 owner 交接 | ⬜ |

## P0 — 测量协议与指标字典 ⬜

- 冻结每个观测窗口的 run id、服务器 commit、bot 客户端版本、协议版本 763、世界/zone、起始角色状态、场景序列、bot 数、并发策略、持续时间、退出原因和 CI 环境。
- 指标至少包括：每小时骨币面值与 `spirit_quality` 变化、物品实例/模板及数量变化、真元余额/zone 变化、消耗品与材料消耗、失败/拒绝/丢弃/拾回数量、loot/discard 配额使用、请求速率、Redis/SQLite 写入量、CPU/内存/网络/磁盘成本。
- 对收益、消耗、持有量、转移和凭空生成分别建字段；同一物品实例跨背包/地面/容器只计一次，骨币真元按 `spirit_quality` 口径，不按枚数替代。
- 明确观测边界：dev 命令只能出现在 fixture/setup，生产动作从第一条可计时请求开始；断线、重连、服务器重启、超时和异常退出单独标记，不静默拼接成成功样本。
- 先用已有 V 轨场景做可观测性探针，确认 `bong:server_data`、经济 channel 和 SQLite/服务器指标能对拍；缺字段时登记 owner，不在此 plan 伪造估算。

## P1 — 只读采样与标注接入 ⬜

- 在 Bot harness/CI 侧记录每个场景的动作时间线和 typed payload，捕获 `inventory_snapshot`、`player_state`、`bong:bone_coin_tick`、`bong:price_index` 与服务器成本计数；采样不得改变请求节奏或玩法状态。
- 给每个事件加统一 run/scenario/window 标签和单调 tick/时间戳，支持按请求、物品 instance、账号 principal（若认证 plan 已落地）回溯；不把 bearer、密码或内部数据库秘密写进报告。
- 对连接断开、未知 payload、未解码事件、漏掉末页、跨维切换和重复事件 fail-closed 标记为无效样本；报告必须区分“没有收益”和“没有可观测数据”。
- 任何真元统计只读 ledger/既有快照并执行守恒对拍；评估工具不得调用 `/qi set`、`/give` 或其它 dev 命令作为计时内动作。

## P2 — 测量矩阵 ⬜

- 至少覆盖：单 bot 低风险搜打撤、锻造/制作等非冻结生产场景、战斗/掉落、库存满/丢弃边界、重连恢复，以及 2/4/8 bot 并发；场景集合以 V 轨最终已合入清单为准。灵田、经脉、功法/各流派招式和身体部位冻结期间，不新增或扩展这些域的测量场景。
- 每种场景分别运行短窗口验证接线、中窗口观察稳定速率、长窗口观察骨币半衰/资源匮乏与成本；重复运行并保存原始样本、seed、commit 和环境。
- 对不同起始境界、库存容量、zone 灵气、装备/材料、连接抖动和服务器负载建立分层样本；冻结域不作为新增或扩展的实验变量。
- 明确挂机行为定义（持续连接、动作间隔、失败重试、休息/回城策略）；这些是测量条件，不是生产规则建议。

## P3 — 报告与数据质量 ⬜

- 输出每场景每小时收益/消耗、净持有变化、真元变化、丢弃/拾回、失败率、请求速率和服务器成本；同时给出样本数、有效窗口比例、median/p95 或区间，不只给单个平均数。
- 把骨币封存真元、普通物品、消耗品、地面掉落、容器存量和服务器资源分开报；对重复事件、断线窗口、跨窗口持有物和 setup 赠品做审计表。
- 用 V 轨 bot e2e 与现有经济 schema/Redis channel 做数据完整性 pin；schema 漂移、未知字段、经济快照缺失或守恒不闭合时报告失败，不自动补零。
- 报告同时回答“单 bot 是否能稳定收益”“并发是否放大收益/成本”“长时间持有是否因半衰改变收益结构”“discard/loot 配额是否成为瓶颈”，不直接回答该不该封禁或该定多少。

## P4 — 规则决策输入与交接 ⬜

- 将结果交给经济 owner 和总纲 §9.9 决策门：先给事实区间、敏感性和测量局限，再由人工/后续 plan 决定挂机规则、配额或收益调整。
- 不在本 plan 中预先设每小时骨币上限、真元上限、物品掉落率、并发数、封禁阈值或 bot 专属价格；任何规则必须另立 owner plan 并引用本报告版本。
- 若测量显示现有玩法/账本违反守恒或物品所有权，另开 bugfix/refactor 任务；本 plan 只记录证据，不借评估 PR 偷渡玩法修复。

## 验收与边界

- 验收要求：前置 V 轨完成可核验；同一 commit/seed/fixture 可重放；所有窗口有完整标签和原始样本；收益与消耗逐类可追溯；真元总量对拍 `qi_physics::constants::DEFAULT_SPIRIT_QI_TOTAL` 或显式总量配置；坏包/未知事件不会让统计脚本崩溃或吞样本；报告明确不确定性和无效窗口。
- 本 plan 不改 `server/`、`client/`、`agent/` gameplay 或经济规则，不新增认证、货币、真元公式、限流信号、bot-only 旁路，也不修改 `docs/worldview.md` 或 `docs/library/`。

## §8 开放问题（升 active / P0 决策门前收口）

1. V 轨 P5 完成后的正式场景基线和可计时入口；由 V owner 在完成门证据中冻结，不由本 plan 复制一份清单。
2. 服务器成本指标的权威采样来源（CI runner telemetry、server process metrics、Redis/SQLite counters）及保留期限。
3. 长窗口是否需要固定世界时钟/时代、zone 灵气和 bot 角色起始状态；若需要，必须把 fixture 与报告版本化。
4. 统计区间和最小有效样本数；本 plan 只要求报告不确定性，不先拍经济阈值。

## §10 实施工作流

### §10.1 顺序

1. V 轨完成并在 origin/main 可核验后，PR-1 落 P0/P1 测量协议与只读采样。
2. PR-2 落 P2 场景矩阵与 CI 采样接入；只使用已支持的参考 bot。
3. PR-3 落 P3 报告和数据质量审计；PR-4 交付 P4 评估报告与后续 owner handoff。

### §10.2 验证

- Bot/Python 测试、V 轨协议 pin、经济 channel/schema 对拍和报告重放测试按实际改动栈运行；不以“脚本跑完”代替原始样本完整性。
- 从最新 `origin/main` fetch+merge 后复验受影响栈；任何 server/client/agent 变更都超出本 docs/measurement skeleton 的默认范围，需另立实现 plan。

### §10.3 review 与归档

- 每个实现 PR 用独立上下文、中文 commit、`Model:` trailer，PR body 写明 docs/code 范围、可读性自查和数据局限；按当前 HEAD 对拍 Kody。
- 只有 P0-P4 全部完成、报告和 raw sample 归档路径可复核，且后续规则 owner 接手后，才迁入 `docs/finished_plans/`。

### §10.4 PR 实施上下文

- 每个 PR 使用独立实施 subagent；prompt 必须包含本 PR 的采样范围、V 轨已完成证据、数据完整性约束和“不制定经济规则”边界，并先核对现有生产采样接口。
- 实施 subagent 使用 `subagent_type: "claude"`、`model: "opus"`，prompt 末尾包含 `ultrathink`；它负责实现、对应栈门禁、push 和开 PR，不等待 review，也不 merge。
- 返工由新的独立 subagent 接手同一 PR；保留原始采样与报告版本，不能为得到预期结果改写或筛除失败窗口。

### §10.5 Kody review 等待

- PR 创建和每次 push 后按 `docs/CLAUDE.md §6.5` 等待 Kody；当前 HEAD 以 `gh pr view --json headRefOid` 为准，行内意见用 `original_commit_id` 对拍并通过 `gh api .../comments --paginate` 读取完整列表。
- Kody 反馈缺失时遵守 §6.5 等待轮数；当前 HEAD 有成立意见必须先处理，再等待当前 HEAD 的明确 clean。若意见要求预先拍定收益阈值、处罚规则或 bot 专属价格，停止并交人工决策。

### §10.6 单次 consume-plan 到 merge

- 一次 `/consume-plan` 只推进依赖序列中当前最前一个未完成 PR；各 PR 经采样质量门、最新主线复验、Kody 和 CI 收敛后，orchestrator 在既有授权边界内 squash merge。无 merge 授权则停在可合并状态。
- 用户无需手工拆分 PR；全部阶段完成、Finish Evidence 和数据报告齐全后，由最后阶段 PR 归档本 plan。

### §10.7 单次消费边界

- `/consume-plan` 只推进一个阶段；没有 V 轨完成证据、数据源不完整或需要拍新的经济数值时停下交人工，不在测量 agent 内自行定规则。

## Finish Evidence

> 迁入 `docs/finished_plans/` 前填写：V 轨前置 commit、测量 fixture/场景清单、raw sample 与报告路径、每小时收益/消耗/成本结果、守恒与数据质量 pin、规则 owner 及不在本 plan 范围的后续事项。
