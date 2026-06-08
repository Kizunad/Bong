# Bong · plan-neg-domain-escape-v1 · 负灵域逃遁战术

**实装负灵域的战术价值**：worldview §二 明确定义两条战术路径，但当前代码两者均未实现。其一：**天道在负数区无法索敌**——通灵境修士可躲入负灵域逃避天劫（代价：境界跌落）；其二：**溺水暗杀术**——低境界修士将高境界修士引入负灵域，利用灵压差实现极限反杀。前者需在天道/agent calamity 层加负灵域免疫判定；后者是已实装物理规律（`negative_zone.rs` 按 qi_max 缩放抽吸量）的**叙事正典化**（narration + 感知 + 技巧传授）。

**世界观锚点**：
- `worldview.md §二:53-55` 负灵域战术价值——"通灵境修士可躲入负灵域逃避天劫（天道在负数区无法索敌），代价是跌落境界" / "低境界修士可将高境界修士诱入负灵域……实现极限反杀（**负压战术/溺水暗杀术**）"
- `worldview.md §八` 天道行为准则（"静观手段/等塌回收"）——天道不追入负灵域是因为负灵域本身就是天道的"自然陷阱"

**前置依赖**：
- `plan-qi-physics-v1` ✅ — `ZoneRegistry` + `siphon_amount`（负灵域物理已实装）
- `plan-tribulation-v1` ✅ — `AscensionQuotaStore` + tribulation 触发机制（本 plan P0 在此基础上加负灵域豁免判定）
- `plan-neg-domain-fauna-v1` ⬜（本 plan P1 正典化逃遁叙事时，负灵域内的危害密度已实装）

**反向被依赖**：
- `plan-tribulation-balance-v1` ⬜（逃遁使用频率是平衡矩阵的一个变量，需本 plan 提供 telemetry 数据）

---

## 接入面 Checklist

- **进料**：`ZoneRegistry`（玩家当前 zone 的 spirit_qi 读取）/ `AscensionQuotaStore`（玩家是否有 pending 天劫）/ agent `calamity.md` skill（天劫触发逻辑）/ `cultivation::realm`（通灵境判定）/ `qi_physics::siphon_amount`（溺水物理已有，无需改）
- **出料**：P0——天劫豁免检查（负灵域内挂起 tribulation 计时器，出域后恢复）+ Narration "天道失去你的踪迹" / "天道重新锁定" + telemetry 计数器；P1——溺水暗杀术 Narration 模板 + 感知提示（对方真元骤降时有提示）
- **共享类型**：无新增 IPC schema；`calamity.md` 新增 `neg_domain_exempt` 判定分支
- **跨仓库契约**：
  - agent：`packages/tiandao/src/skills/calamity.md` + `calamity.ts` 加负灵域豁免判定（查 `world_state.players[*].zone.spirit_qi`）
  - server：无新文件；在 tribulation schema event 上加 `pending_in_neg_domain: bool` 字段（可选，用于 P0 telemetry）
  - client：HUD 微妙提示"灵压庇护"（可选 P1）
- **worldview 锚点**：§二 负灵域战术价值 + §八 天道行为准则
- **qi_physics 锚点**：本 plan 不引入新的 QiTransfer；溺水物理已由 `negative_zone.rs` 守恒实装；天劫豁免不改变 qi 流动

---

## §0 设计轴心

- [ ] **天道豁免是战术工具，不是安全港**：躲进负灵域确实能挂起天劫——但 `negative_zone.rs` + `plan-neg-domain-fauna-v1` 的诡影/噬灵藓让待在负灵域本身也是在扣命。真正的选择是"被天劫劈死 vs 被抽干真元降境"
- [ ] **溺水暗杀术是既有物理的叙事正典化**：`siphon_amount ∝ qi_max` 已经实现了高境界在负灵域更危险的物理。本 plan 的工作是让玩家能**感知到**这条规律（narration + 感知到对方真元骤降），而不是再造新的物理
- [ ] **降境代价是设计中轴**：逃进负灵域后 qi_max 无法维持 → 经脉萎缩 → 境界跌落，这条链路由 cultivation 系统已有规则驱动，本 plan 不新建
- [ ] **不破守恒律**：天劫豁免 = 天道暂不主动发起，不是 qi 不流动；玩家在负灵域内真元照样被 zone 抽走

---

## §1 阶段总览

| 阶段 | 内容 | 状态 | 验收 |
|------|------|------|------|
| **P0** | agent calamity 加负灵域豁免判定：玩家 zone spirit_qi < 0 时不触发/挂起天劫；出域后恢复 | ✅ 2026-06-08 | ≥ 8 单测：mock player in neg zone → calamity skill 不发天劫事件 / 玩家移到正 zone → 重新评估触发 / pending 后条件变化不误触发 / 仅通灵境豁免（固元/凝脉不豁免）/ 离线重连 pending 保留 |
| **P1** | Narration 正典化：天道"失去锁定" / "重新锁定"提示 + 溺水暗杀术 narration（感知对方真元骤降）| ✅ 2026-06-08 | ≥ 6 单测：通灵境玩家入负灵域时广播"灵压庇护"类 narration / 出域广播"重新锁定" / 对方 qi_current 5 tick 内降超 25%×qi_max 触发溺水 hint / 境界差不足两级不触发 / 私信 A + 公开 B 气息溃散 scope 正确 |
| **P2** | telemetry + 平衡数据：逃遁使用频率 / 逃遁后境界跌落率 / 实际成功规避天劫次数 | ✅ 2026-06-08 | ≥ 4 单测：三类计数器累积正确 + 累积到 plan-tribulation-balance-v1 的数据管道 |

---

## §2 P0：agent calamity 豁免判定

### 修改点

`agent/packages/tiandao/src/skills/calamity.md`（calamity skill 指令文档）：

```text
### 负灵域豁免规则（worldview §二 战术价值）
- 执行天劫触发前，先检查目标玩家所在 zone 的 spirit_qi
- 若 spirit_qi < 0（负灵域定义）：
  → 不触发天劫；在内部 pending_tribulation 队列记录该玩家
  → 天道 narration："[玩家名]遁入负灵域，灵压异常，天道视线受阻"（scope: broadcast）
- 若玩家离开负灵域（spirit_qi ≥ 0）：
  → 重新评估 pending 队列；若仍满足天劫条件则触发
  → 天道 narration："[玩家名]离开负灵域庇护，天道重新锁定"
- 每次 pending check 时仍按正常 calamity 条件重新判断（避免 pending 后条件变化导致错误触发）
```

对应 TypeScript：`calamity.ts` 加 `isInNegDomain(player)` helper（查 `world_state`），在 `shouldTrigger` 分支前添加豁免检查。

---

## §3 P1：溺水暗杀术 Narration 模板

**场景**：玩家 A（低境界）将玩家 B（高境界）引入负灵域，B 的真元因 qi_max 大而急速被抽干。

**narration 触发条件**：
- B 的 `qi_current` 在 5 tick 内下降超过 `qi_max × 0.25`（25% 骤降，排除正常消耗）
- B 当前在负灵域（spirit_qi < 0）
- B 的境界比 A 高至少两级

**narration 模板**（天道视角）：
- "高处不胜寒。[B名] 的真元如倒悬之泉，灵压差已超出其所能承受。"
- "[A名] 选择了溺水之地——此地天地之法，不偏向强者。"

**感知提示**（给 A 的私信）：
- "你感知到对方的气息在急剧溃散——这是你的机会。"

---

## §4 开放问题

- [ ] 天劫 pending 状态在玩家下线/重连后是否保留？——倾向：保留（写入 persistence plan 的玩家存档）
- [ ] 通灵境以外的境界（固元/凝脉）进入负灵域是否也豁免天劫？——倾向：只豁免通灵境（worldview §二 原文明确"通灵境修士"）
- [ ] 溺水暗杀术 narration 是否公开广播还是私信？——倾向：私信 A（给 A 战术信息），公开 B 的气息溃散（戏剧性）
- [ ] 天劫豁免期间 quota 是否锁定？——倾向：不锁定（pending 不算已在渡劫，quota slot 仍开放）

## §5 进度日志

- 2026-05-31：骨架创建。worldview §二 战术价值审计，agent calamity 代码无负灵域豁免判定；negative_zone.rs 的物理已足够支撑溺水物理，缺的是逻辑判定和叙事正典化。

## Finish Evidence

### 落地清单

- **P0 负灵域天劫豁免 gate**：`agent/packages/tiandao/src/neg-domain-escape.ts` 增加 `applyNegDomainTribulationGate`，`agent/packages/tiandao/src/runtime.ts` 在天道 tick 后处理 targeted `thunder_tribulation`，`agent/packages/tiandao/src/world-model.ts` 持久化 `NegDomainPendingTribulation`。
- **P1 负灵域叙事正典化**：`agent/packages/tiandao/src/neg-domain-escape.ts` 增加负灵域失锁/重锁 narration 与 `renderNegDomainNarrations` 溺水暗杀术提示，`agent/packages/tiandao/src/runtime.ts` 将 narration 接入 tick 输出。
- **P2 telemetry 与平衡数据管道**：`agent/packages/tiandao/src/world-model.ts` 增加 `NegDomainEscapeTelemetrySnapshot` 与逃遁 session，`agent/packages/tiandao/src/redis-ipc.ts` 扩展 Redis mirror 字段，`agent/packages/schema/src/agent-world-model.ts` 与 `agent/packages/schema/generated/agent-world-model-*.json` 同步 schema。
- **测试与 fixture**：`agent/packages/tiandao/tests/neg-domain-escape.test.ts` 覆盖 P0/P1/P2 行为，`agent/packages/tiandao/tests/runtime.test.ts` 覆盖 runtime wiring 与 telemetrySink，`agent/packages/tiandao/tests/redis-ipc.test.ts` 覆盖 Redis mirror 兼容解析，`agent/packages/tiandao/data/tiandao-snapshot-200.json` / `tiandao-snapshot-300.json` 更新 snapshot 样本。

### 关键 commit

- `915c4e778a8a62d75626f211e076eb0bdf70a8f7`（2026-06-08）`feat(tiandao): P0 负灵域天劫豁免 gate`，PR #450：`https://github.com/Kizunad/Bong/pull/450`。
- `0b429667383dd102522c04354593a721f09a1e36`（2026-06-08）`feat(tiandao): P1 负灵域溺水叙事正典化`，PR #451：`https://github.com/Kizunad/Bong/pull/451`。
- `20b7411d2d4cec5b45ff917be727066df607af2c`（2026-06-08）`feat(tiandao): 接入负灵域逃遁 P2 telemetry`，PR #452：`https://github.com/Kizunad/Bong/pull/452`。

### 测试结果

- P0 PR #450：`e2e` CI 通过，`/review` 判定可合并，CodeRabbit 通过。
- P1 PR #451：`e2e` CI 通过，`/review` 判定可合并，CodeRabbit 通过。
- P2 PR #452：`cd agent/packages/tiandao && npm test -- --run tests/neg-domain-escape.test.ts tests/runtime.test.ts tests/redis-ipc.test.ts` 通过（3 files / 88 tests）。
- P2 PR #452：`cd agent && npm run build` 通过；`cd agent/packages/schema && npm test` 通过（25 files / 554 tests）；`cd agent/packages/schema && npm run generate:check` 通过。
- P2 PR #452：`cd agent/packages/tiandao && npm test` 通过（65 files / 601 tests）；`e2e` CI 通过；`/review` 判定可合并；CodeRabbit 通过。

### 跨仓库核验

- **agent**：`applyNegDomainTribulationGate`、`renderNegDomainNarrations`、`recordNegDomainEscapeTelemetry`、`WorldModel.getNegDomainEscapeTelemetrySnapshot`、`WORLD_MODEL_STATE_FIELDS.negDomainEscapeTelemetry` 已落地。
- **schema**：`AgentWorldModelSnapshotV1.negDomainPendingTribulations`、`AgentWorldModelSnapshotV1.negDomainEscapeTelemetry`、`AgentWorldModelSnapshotV1.negDomainEscapeSessions` 已同步到 generated JSON schema。
- **server/client**：本 plan 未引入 server/client 改动；负灵域抽吸物理继续复用既有 `negative_zone.rs` / `qi_physics::siphon_amount`，叙事与 telemetry 在 agent 层消费 world_state 与 WorldModel snapshot。

### 遗留 / 后续

- `plan-neg-domain-fauna-v1` 仍负责负灵域内诡影/噬灵藓等环境威胁密度，本 plan 不实现 fauna。
- `plan-tribulation-balance-v1` 可消费 `negDomainEscapeTelemetry` 做后续平衡矩阵；本 plan 只提供累计计数与持久化数据管道。
