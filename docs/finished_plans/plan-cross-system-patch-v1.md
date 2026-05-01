# Bong · plan-cross-system-patch-v1

**已实装系统的"互不知道"接入补全**。两轮 sonnet subagent 实地代码审计（grep + git log + 三层对账）发现：玩法系统大多已闭环，但**系统之间的 hook / IPC 契约 / S2C payload 路由有 24 处可观测缺口**——已 emit 的事件没人消费、已定义的字段从未应用、agent 该听的 channel 没订阅。本 plan 把这些缺口按"merge 冲突面 + 难度"分四阶段补齐，纯接入面工程，不引入新机制 / 不调系数 / 不动 worldview / 不加 item。

**世界观锚点**：本 plan 不引入新机制，锚点**继承被接入的目标系统**——`worldview.md §三 修炼体系`（skill 全类目）· `§四 战斗系统`（damage 管线 / 炸炉反噬）· `§六 真元染色`（meridian overload 实效）· `§七 动态生物生态`（散修死亡 / 灵田无主）· `§八 天道行为准则`（agent 漏听 channel → narration 喂料）· `§九 经济与交易`（封灵骨币 / 社交印记）· `§十二 一生记录`（aging / lifespan / death event 联动）。

> **注**：本 plan 是 cross-system patch，每个交付物的 worldview 锚点都已在被接入系统的源 plan（finished_plans/ 下）正典化，不重复列举。

**library 锚点**：无（纯接入面工程，不写新书）。

**交叉引用**（依赖关系——本 plan 不动这些 plan 的设计，只补它们之间的 wire）：
- `plan-skill-v1`（finished） · `plan-cultivation-v1`（finished） · `plan-alchemy-v1`（finished） · `plan-forge-v1`（finished） · `plan-shelflife-v1`（finished） · `plan-botany-v1`（finished） · `plan-mineral-v1`（active） · `plan-lingtian-v1`（active） · `plan-social-v1`（finished） · `plan-death-lifecycle-v1`（finished） · `plan-tribulation-v1`（active） · `plan-npc-ai-v1`（finished）
- 与本 plan 解锁后的下游：`plan-botany-agent-v1`（骨架，P0 即本 plan §1 主题 7.1 完成态） · `plan-fauna-v1`（骨架，依赖 §7 NpcRegistry per-zone budget） · `plan-narrative-v1`（骨架，依赖本 plan §1 全部 agent channel 接通后才有 narration 喂料）

---

## §0 接入面 checklist（按 docs/CLAUDE.md §二）

- **进料**：现有 `RedisOutbound` enum / `SkillXpGain` event / `DeathEvent` / `CultivationDeathTrigger` / `CombatEvent` / `MeridianOverloadEvent` / `AlchemyOutcomeEvent` / `ServerDataPayloadV1` / agent `redis-ipc.ts` subscribe 列表 / client `ServerDataRouter` handler map。
- **出料**：补完后 → agent 多收 11 类 channel（narration 喂料）→ client 正确接 `burst_meridian_event` payload → skill 全 7 类目可加点 → alchemy 炸炉真扣血 → 散修死后田释放 → 炼丹/突破查 zone qi。
- **共享类型 / event**：**严禁新建近义类型**。全部复用——加 `SkillId::Combat / Mineral / Cultivation` 是 enum 扩展不是新 event；加 `RedisOutbound::SkillXpGain / SkillLvUp / ...` 是已有 enum 的新变体；agent 侧加 subscribe 是在现有 `redis-ipc.ts` 模式上加行。
- **跨仓库契约**：每个 channel/payload 三层都要动到——
  - server: `RedisOutbound` enum 变体 + emit 调用点
  - agent: `redis-ipc.ts` subscribe + handler module
  - client（仅 §3 主题 8.1）: `ServerDataRouter` handler 注册
  - schema: 全部已在 `agent/packages/schema/src/*.ts` 定义，**本 plan 不新增 TypeBox 类型**
- **worldview 锚点**：见头部"世界观锚点"节，本 plan 继承被接入系统的锚点。

---

## §1 阶段总览

| 阶段 | 主题 | 候选数 | merge 冲突面 | 难度 | 状态 |
|----|------|------|-----------|------|------|
| P0 | Batch A：纯接入面（agent subscribe + client handler + 死 schema 清理） | 12 | 零（每候选 1-2 文件，互不重叠） | ⭐ | ✅ |
| P1 | Batch B：Skill XP 全类目补全（Redis 化 + enum 扩展 + 4 路 emit） | 5 | 内部串行（共改 SkillId enum） | ⭐⭐ | ✅ |
| P2 | Batch C：玩法逻辑接入（damage / zone qi / death lifecycle / inventory snapshot / shelflife / NpcRegistry per-zone） | 9 | 模块独立可并行 | ⭐⭐ | ✅ |
| P3 | Batch D：DuoSheIntent stub → 真实分发 | 1 | 跨 3 文件 | ⭐⭐⭐ | ✅ |

---

## §2 P0 — Batch A：纯接入面（12 候选 · 零冲突 · 可全并行）

每候选独立 PR，机械改动："在已有 subscribe 列表加一行 / 在已有 emit 路径加 send / 在 client handler map 加注册 / 删除全库 0 处构造的 dead schema"。

### 2.1 Agent 漏听 Redis channel（11 候选）

server 已 emit 但 agent `redis-ipc.ts` 未 subscribe。每候选交付物固定模板：
- `agent/packages/tiandao/src/redis-ipc.ts`：加 `subscribe('<channel>')` + handler 函数
- `agent/packages/tiandao/src/<channel>-runtime.ts`：新文件 or 复用现有 runtime（参考 `skill-lv-up-runtime.ts` 模式）
- 测试：`agent/packages/tiandao/test/<channel>-runtime.test.ts`，断言"channel 收到 payload → WorldModel 字段更新 / narration 入队"

| # | Channel | server emit 位置 | 备注 |
|---|---|---|---|
| 7.1 | `bong:botany/ecology` | `server/src/botany/ecology.rs::emit_botany_ecology_snapshot`（每 600 tick）| 等价于 `plan-botany-agent-v1` 的 P0 一次完成 |
| 7.2a | `bong:aging` | `RedisOutbound::Aging`（`network/redis_bridge.rs`） | 寿元 tick |
| 7.2b | `bong:lifespan_event` | `RedisOutbound::LifespanEvent` | 寿元节点 |
| 7.2c | `bong:duo_she_event` | `RedisOutbound::DuoSheEvent` | 夺舍事件 |
| 7.3a | `bong:breakthrough_event` | `RedisOutbound::BreakthroughEvent` | 突破成功 |
| 7.3b | `bong:cultivation_death` | `RedisOutbound::CultivationDeath` | 修仙者死亡 |
| 7.4a | `bong:forge_event` | `RedisOutbound::ForgeEvent` | 锻造结果 |
| 7.4b | `bong:forge/start` | `RedisOutbound::ForgeStart` | 开炉 |
| 7.4c | `bong:forge/outcome` | `RedisOutbound::ForgeOutcome` | 出装 |
| 7.5 | `bong:social/{exposure,pact,feud,renown_delta}` | `RedisOutbound::SocialExposure / SocialPact / SocialFeud / SocialRenownDelta` | 4 个一组接，社交全套 |
| 7.6 | `bong:combat_realtime` / `bong:combat_summary` / `bong:armor/durability_changed` | `RedisOutbound::*` 三变体 | 战斗实况 + 装备耐久 |

**Grep 抓手**（验收）：
```bash
grep -rn "subscribe.*'bong:" agent/packages/tiandao/src/redis-ipc.ts | wc -l   # 完成后 ≥ 16（当前 5）
```

### 2.2 双向空洞清理（1 候选）

| # | Channel | 现状 | 交付物 |
|---|---|---|---|
| 7.7 | `bong:rebirth` | `server/src/schema/channels.rs::CH_REBIRTH` 声明，但 `RedisOutbound` 无变体；agent 也不听 | 决策二选一：① 在 `RedisOutbound` 加 `Rebirth` 变体 + agent 加 subscribe；② 删除 `CH_REBIRTH` 常量。**推荐 ①**（plan-death-lifecycle-v1 的 rebirth 流程已实装，缺的只是 agent 通知） |

### 2.3 Client S2C handler 漏注册（1 候选）

| # | Payload | 现状 | 交付物 |
|---|---|---|---|
| 8.1 | `burst_meridian_event` | `server/src/network/burst_event_emit.rs` 广播全玩家，client `ServerDataRouter.java` 无注册 | `client/src/main/java/com/bong/client/network/ServerDataRouter.java` 加 `handlers.put("burst_meridian_event", new BurstMeridianHandler())` + 新 handler class |

**Grep 抓手**：`grep -n 'burst_meridian_event' client/src/main/java/com/bong/client/network/ServerDataRouter.java` 必须命中。

### 2.4 死 schema 清理（1 候选）

| # | Schema | 现状 | 交付物 |
|---|---|---|---|
| 8.2 | `MiningProgress` | `server/src/schema/server_data.rs` 全套定义 + `agent_bridge.rs` label 映射，但全库 0 处 `ServerDataPayloadV1::MiningProgress { .. }` 构造 | 决策二选一：① 实装——在 `mineral/break_handler.rs` 采矿进度 tick 内构造 + emit；② 删除 schema + label。**先调研 `plan-mineral-v1` 是否计划用**，再决定 |

---

## §3 P1 — Batch B：Skill XP 全类目补全（5 候选 · 串行 · 一个 PR）

`SkillId` 当前只有 `Herbalism / Alchemy / Forging` 三变体（`server/src/skill/components.rs:21-25`），剩下半数玩法系统压根无 XP 通路。本阶段一次扩齐 + 把 skill channel 从 client-only 推到 Redis（agent 才能消费）。

**强制串行**：`SkillId` enum 改一次，1.1–1.4 共改这一处，必须一个 PR 完成；1.5 是 Redis 化，独立但同期落地以避免 agent 收到旧 enum。

| # | 交付物 | 位置 | grep 抓手 |
|---|---|---|---|
| 1.5 | `RedisOutbound` 加 `SkillXpGain / SkillLvUp / SkillCapChanged / SkillScrollUsed` 变体；`network/skill_emit.rs` 在推 client 同时推 Redis | `network/redis_bridge.rs` + `network/skill_emit.rs` | `grep -n "RedisOutbound::SkillXpGain" server/src/network/`；agent `redis-ipc.ts` `subscribe('bong:skill/xp_gain')` 命中 |
| 1.1 | `SkillId::Combat` 变体 + `combat/resolve.rs::kill_npc` 路径 emit `SkillXpGain { skill: Combat }` | `skill/components.rs` + `combat/resolve.rs:692` 附近 | `grep -rn "SkillId::Combat" server/src/combat/` ≥ 1 |
| 1.2 | `SkillId::Mineral` 变体 + `consume_mineral_drops_into_inventory` 完成后 emit | `skill/components.rs` + `mineral/inventory_grant.rs:31` | `grep -rn "SkillId::Mineral" server/src/mineral/` ≥ 1 |
| 1.3 | `SkillId::Cultivation` 变体 + `breakthrough.rs` / `meridian_open.rs` 成功路径 emit | `skill/components.rs` + `cultivation/breakthrough.rs` + `cultivation/meridian_open.rs:65` 附近 | `grep -rn "SkillId::Cultivation" server/src/cultivation/` ≥ 2 |
| 1.4 | `lingtian/systems.rs` 翻土 / 种植 / 浇灵 / 收获 4 路径 emit `SkillXpGain { skill: Herbalism }`（参考 `botany/harvest.rs:196` 模式） | `lingtian/systems.rs` | `grep -n "SkillXpGain" server/src/lingtian/systems.rs` ≥ 4 |

**测试**：`skill::xp_gain_full_coverage` —— 7 类 SkillId 各跑一次完整 emit→accumulate→cap roll 路径，断言无 panic + xp_total 增量正确。

---

## §4 P2 — Batch C：玩法逻辑接入（9 候选 · 模块独立可并行）

模块边界清晰，可同时并行多 PR。每候选独立 worktree。

### 4.1 Damage 管线接入（3 候选）

| # | 交付物 | 位置 | grep 抓手 |
|---|---|---|---|
| 2.1 | **alchemy 炸炉真扣血**：消费 `AlchemyOutcomeEvent`，当 `outcome == Explode { damage, meridian_crack }` 时 send `CombatEvent` 对 caster 造成 `damage` 伤害 + send `MeridianOverloadEvent { crack: meridian_crack }` | `alchemy/mod.rs` 加新 system；`alchemy/resolver.rs:39` 字段已有 | `grep -rn "ResolvedOutcome::Explode" server/src/alchemy/` 至少 2 处（resolver 定义 + 新消费方）；测试 `alchemy::explode_applies_damage_to_caster` |
| 2.2 | **食物 spoil 扣血**：在玩家食物消费路径（非丹药），读 `spoil_check` 结果，`Warn / CriticalBlock` 时 send `CombatEvent` 小伤害 | player 食物消费路径 + `shelflife/consume.rs` | `grep -n "SpoilCheckOutcome" server/src/<player food path>` 命中 |
| 2.3 | **`MeridianOverloadEvent` 加 cultivation 实效消费**（当前只 `audio_trigger.rs:115` 接音效）：在 cultivation 内加 reader，写入 `MeridianCrackState` 或扣真元 | `cultivation/overload.rs` 新增 system or 在 existing tick 加 reader | `grep -rn "EventReader<MeridianOverloadEvent>" server/src/cultivation/` ≥ 1 |

### 4.2 Inventory snapshot 推送 + ItemEffect 双入口对齐（3 候选）

| # | 交付物 | 位置 | grep 抓手 |
|---|---|---|---|
| 4.1 | mineral 采矿后调 `send_inventory_snapshot_to_client` | `mineral/inventory_grant.rs:31` 后 | `grep -n "send_inventory_snapshot_to_client" server/src/mineral/` ≥ 1 |
| 4.2 | lingtian 收获后调 `send_inventory_snapshot_to_client` | `lingtian/systems.rs` 收获 system 末尾 | `grep -n "send_inventory_snapshot_to_client" server/src/lingtian/` ≥ 1 |
| 4.3 | `handle_alchemy_take_pill` 对 `MeridianHeal / ContaminationCleanse` 的 warn-log 改为调 `cast_emit.rs:320` 已实装的 helper | `network/client_request_handler.rs:4417` | `grep -A3 "ItemEffect::MeridianHeal" server/src/network/client_request_handler.rs` 不再含 `warn!` |

### 4.3 Death lifecycle 闭环（2 候选）

| # | 交付物 | 位置 | grep 抓手 |
|---|---|---|---|
| 5.1 | 散修 NPC 死 → `lingtian.plot.owner` 释放：新增 system 读 `EventReader<DeathEvent>`，对应 `plot.owner == Some(dead_entity)` 时 set `None` | `lingtian/systems.rs` or `lingtian/lifecycle.rs` 新文件 | `grep -rn "EventReader<DeathEvent>" server/src/lingtian/` ≥ 1 |
| 5.2 | 社交仇恨印记 on death：`npc/social.rs` 加 `EventReader<DeathEvent>`，按 attacker_id 写仇家记录 | `npc/social.rs` | `grep -rn "EventReader<DeathEvent>" server/src/npc/social.rs` ≥ 1 |

### 4.4 Zone qi 玩法接入（2 候选）

| # | 交付物 | 位置 | grep 抓手 |
|---|---|---|---|
| 6.1 | alchemy 炉点燃 / session 启动时查 `ZoneQiAccount.spirit_qi`；qi 不足则效率衰减或 reject | `alchemy/session.rs` or `alchemy/mod.rs` | `grep -rn "spirit_qi\|ZoneQiAccount" server/src/alchemy/` ≥ 1 |
| 6.2 | breakthrough 凝核阶段查 zone qi（meridian_open `MIN_ZONE_QI_TO_OPEN` 已查 ✅；breakthrough 也加门槛） | `cultivation/breakthrough.rs` | `grep -n "spirit_qi\|MIN_ZONE_QI" server/src/cultivation/breakthrough.rs` ≥ 1 |

### 4.5 Shelflife 真接入（1 候选）

| # | 交付物 | 位置 | grep 抓手 |
|---|---|---|---|
| 3.1 | `alchemy/mod.rs:347` 把硬编码 `SpoilCheckOutcome::NotApplicable / AgePeakCheck::NotApplicable` 替换为真调 `spoil_check / age_peak_check` | `alchemy/mod.rs:347` | `grep -B2 -A2 "NotApplicable" server/src/alchemy/mod.rs` 不再命中（或仅在 fallback 分支命中） |

### 4.6 NpcRegistry per-zone 预算（1 候选）

| # | 交付物 | 位置 | grep 抓手 |
|---|---|---|---|
| 9.1 | `NpcRegistry` 加 `per_zone_caps: HashMap<String, usize>` 字段 + `reserve_zone_batch` 方法 + `release_zone_slot`；现有全局 cap 不变 | `npc/lifecycle.rs:178` | `grep -n "per_zone_caps\|reserve_zone_batch" server/src/npc/lifecycle.rs` ≥ 2 |

---

## §5 P3 — Batch D：DuoSheIntent stub → 真实分发（1 候选 · 跨模块）

| # | 交付物 | 位置 | grep 抓手 |
|---|---|---|---|
| 9.2 | `npc/possession.rs` 顶注释 "stub 消费系统（仅 log）" 移除；`forward_duoshe_intent` system 完整接通：坐标继承 + NPC entity 移除 + 玩家 entity 替换 | `npc/possession.rs` + `cultivation/possession.rs` + `player/state.rs` | `grep -rn "stub 消费系统\|stub consumer" server/src/npc/possession.rs` 0 命中 |

**说明**：本阶段单 PR、人工 review，不交 GPT 自动并行。

---

## §6 验收命令汇总

完整跑过：
```bash
cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
cd agent && npm run build && cd packages/tiandao && npm test
cd agent/packages/schema && npm test
cd client && ./gradlew test
```

每阶段独立验收（按 §2/§3/§4/§5 各节"grep 抓手"列表逐条命中）。

---

## §7 不做的事

- **不**新增 worldview 章节（接入面工程，沿用现有锚点）
- **不**新增 component / event / schema 类型（违反 §0 接入面 checklist）
- **不**调任何数值 / 系数 / 概率（damage 字段值用 `resolver.rs` 已计算的，不重算）
- **不**加新 item / 新 recipe / 新 plant / 新 NPC 类型
- **不**动 client UI 视觉（`burst_meridian_event` handler 仅做协议路由 + 日志，UI 由后续 plan 补）
- **不**改 `/consume-plan` 默认行为（本 plan 各阶段 PR 走标准 consume 流程）
- **不**顺手归档其他 plan（CLAUDE.md "一个 PR 只动一个 plan" 红线）

---

## Finish Evidence

- **落地清单**：
  - P0：`agent/packages/tiandao/src/redis-ipc.ts` 订阅并缓冲 botany / aging / lifespan / duo_she / breakthrough / cultivation_death / forge / social / combat / rebirth / skill channels；`client/src/main/java/com/bong/client/network/ServerDataRouter.java` 注册 `burst_meridian_event` 并新增 `BurstMeridianHandler`；`server/src/mineral/break_handler.rs` 补齐 `MiningProgress` payload emit。
  - P1：`server/src/skill/components.rs` 扩展 `SkillId::{Combat,Mineral,Cultivation}`；`server/src/network/skill_emit.rs` 对 `SkillXpGain / SkillLvUp / SkillCapChanged / SkillScrollUsed` 做 client + Redis 双发；combat / mineral / cultivation / lingtian 成功路径接入 XP emit；TS/Java wire ids 同步 6 类 skill。
  - P2：`server/src/alchemy/mod.rs` 消费 `AlchemyOutcomeEvent::Explode` 并造成真实伤害 / 死亡 / meridian overload；alchemy start 与 breakthrough 加 zone qi gate；`server/src/cultivation/overload.rs` 消费 `MeridianOverloadEvent`；mineral / lingtian 收获后推 inventory snapshot；服丹路径复用 shelflife 和 `apply_item_effect`；NPC death 释放灵田 owner 并写社交 feud；`server/src/npc/lifecycle.rs` 增加 per-zone budget 并接入 spawn / TSY / world event / agent command。
  - P3：`server/src/npc/possession.rs` 移除 stub 文案并将 `DuoSheIntent` 转发为 `DuoSheRequestEvent`；`server/src/cultivation/possession.rs` 在成功夺舍结算中继承 target 坐标 / 维度 / layer，标记 target `PossessedVictim + Despawned`，并在有 player persistence 时写回 slow slice；`server/src/cultivation/mod.rs` 保证调度在 intent forward 后执行。
- **关键 commit**：
  - `a68e5c50`（2026-05-01）：`plan-cross-system-patch-v1: 补齐 P0 接入通道`。
  - `abed5377`（2026-05-01）：`plan-cross-system-patch-v1: 串起 P1 跨系统熟练度`。
  - `0a44af11`（2026-05-01）：`plan-cross-system-patch-v1: 接通 P2 玩法逻辑`。
  - `decd1e58`（2026-05-01）：`plan-cross-system-patch-v1: 接通 P3 夺舍分发`。
  - `b48e6dcd`（2026-05-01）：`归档 plan-cross-system-patch-v1：Bong · plan-cross-system-patch-v1`。
- **测试结果**：
  - server P2 gate：`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`，`1894 passed`。
  - server P3 gate：`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`，`1895 passed`。
  - P3 定向：`cargo test process_duo_she`，`2 passed`；`cargo test duoshe_intent_event_forwards_runtime_request`，`1 passed`。
  - rebase 后 server final gate：`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`，`1906 passed`。
  - agent：`npm run build` 通过；`agent/packages/tiandao npm test`，`188 passed`。
  - schema：`agent/packages/schema npm test`，`231 passed`。
  - client：`JAVA_HOME=/usr/lib/jvm/java-1.17.0-openjdk-amd64 PATH="/usr/lib/jvm/java-1.17.0-openjdk-amd64/bin:$PATH" ./gradlew test build`，`BUILD SUCCESSFUL`。
- **跨仓库核验**：
  - server：`RedisOutbound::SkillXpGain`、`SkillId::{Combat,Mineral,Cultivation}`、`EventReader<MeridianOverloadEvent>`、`send_inventory_snapshot_to_client`、`per_zone_caps`、`reserve_zone_batch`、`DuoSheIntentForwardSet` 均已命中。
  - agent/schema：`bong:botany/ecology`、`bong:rebirth`、`bong:skill/xp_gain`、`bong:forge/outcome` 等 channel 常量 / subscribe 路径已命中。
  - client：`ServerDataRouter.java` 已注册 `burst_meridian_event`，`BurstMeridianHandler.java` 与 `ServerDataRouterTest.java` 已覆盖。
- **遗留 / 后续**：
  - alchemy start zone qi gate 当前复用请求上下文 / spawn zone 判定，尚未按炉 block position 做精确 zone 查找。
  - `AlchemyOutcomeEvent` consumer 已闭环，但完整炼丹结算 runtime producer 的进一步覆盖仍留给 alchemy 后续 plan。
  - `npc/tsy_hostile.rs` 的 per-zone remaining 回滚仍按深层 zone 分组，若后续观测到计数偏差再单独收敛。
