# Bong · plan-npc-fixups-v2 · 骨架

NPC 系统**第二批正确性 bug fastlane**。承接 plan-npc-fixups-v1 P3 sonnet Explore 异步探查输出：8 个 ECS lifecycle / state machine race / silent stuck / register panic 类 bug + 3 个未列待二次探查（→ P3 / 可能 v3）。**主题：ECS query 缺 `Without<Despawned>` filter + Action `Executing` 状态 silent continue 死锁 + `Executing` 无超时永久卡死**。**无 worldview / qi_physics 锚点**（纯 fix plan）。

**前置依赖**：`plan-npc-fixups-v1` ⏳（#1 #2 先修，否则 baseline 污染）/ `plan-npc-ai-v1` ✅

**反向被依赖**：

- `plan-npc-perf-v1` ⏳ → baseline 前应已修 #1 #2 #6 #8（quota / tribulation race 影响 hydrated NPC 基线数据可信度）
- `plan-npc-virtualize-v1` ⏳ → dormant hydrate-on-tribulation 前应修 #6 #8（quota race 在 dormant hydrate 时更频繁）
- `plan-tribulation-v1` ✅ finished → 本 plan #6 #8 是其 ECS lifecycle 缺失的补强
- `plan-lingtian-npc-v1` ✅ finished → 本 plan #1 是其多 zone 部署的隐式前提

---

## 接入面 Checklist

- **进料**：`server/src/npc/lingtian_pressure.rs:30` / `server/src/npc/brain.rs:929-930` / `server/src/npc/navigation.rs`（chase/flee）/ `server/src/npc/tsy_hostile.rs:351,354` / `server/src/npc/retire_action_system.rs`（NpcRetireRequest）/ `server/src/npc/tribulation.rs:95-111` / `server/src/npc/farming_brain.rs`（4 Action）/ `server/src/npc/ascension_quota.rs`（AscensionQuotaStore）
- **出料**：各 bug 独立 PR + 回归测试；新增 §3 强约束（lint / CI grep 脚本 candidate）
- **共享类型**：复用已有 `Despawned`、`NpcRetireRequest`、`AscensionQuotaStore`、`MeleeAttackAction`（不新建）
- **跨仓库契约**：纯 server 端 fix，无 agent / client 影响
- **worldview 锚点**：无（纯 fix）
- **qi_physics 锚点**：无

---

## §0 Bug 清单

### 高优先 P0

**#1 `lingtian_pressure.rs:30` 多 zone 时选错地块**

- **现象**：道伥召唤打错 zone（多灵田部署时，`plots.iter().next()` 拿到不属于当前 zone 的第一块）
- **修法**：改为按当前 zone 过滤再 `.next()`，或在 query 加 zone 匹配 filter

**#2 `brain.rs:929-930` MeleeAttackAction Executing query miss 用 continue 而非 Failure**

- **现象**：beast / disciple 战斗中击杀对手时，Action 永久冻僵（Executing 态无出口）
- **根因**：目标死亡后 query miss 走 `continue`，Action 停留 `Executing` 不转 `Failure`
- **修法**：query miss 时显式 `action_state.set(ActionState::Failure)`

---

### 中优先 P1

**#3 chase/flee Failure 时拿不到 navigator 不能 stop（NPC 鬼走抖动）**

- **现象**：chase/flee Action Failure 后 NPC 继续抖动走动
- **修法**：Failure 分支显式调 `navigator.stop()`，加 Without<Despawned> guard

**#4 `tsy_hostile.rs:351,354` JSON 加载 unwrap_or_else panic**

- **现象**：CI 缺文件 → server 在 JSON 解析失败时 panic 崩溃（unwrap_or_else 内部 panic 路径）
- **修法**：改为 `?` 或 `warn! + default`，服务器不因资源文件缺失崩溃

**#5 `retire_action_system` NpcRetireRequest 不幂等**

- **现象**：commoner 双胞胎超配额（NpcRetireRequest 触发两次 retire）
- **修法**：检查 NpcRetireRequest 是否已处理，加幂等 guard（processed marker component 或 dedup）

---

### 中优先 P2

**#6 `tribulation.rs:95-111` `npc_tribulation_auto_wave_tick` 缺 `Without<Despawned>`**

- **现象**：NPC 渡劫中被击杀 → 偶发误升 realm（已死 NPC 的 tribulation tick 仍运行）
- **修法**：query 加 `Without<Despawned>` filter

**#7 `farming_brain` 4 个 Action Executing 无超时**

- **现象**：NPC 灵田卡死直到 server 重启（farming Action 没有 deadline）
- **修法**：所有 Executing Action 加 `deadline_ticks`，超时 → `Failure`（具体值见 §3 开放问题）

**#8 `AscensionQuotaStore::release` 缺 `Without<Despawned>`**

- **现象**：化虚名额 1 tick 泄漏（despawned NPC 触发 quota release，名额多出）
- **修法**：release 调用处加 Without<Despawned> check

---

### P3 待二次探查

三个来自 sonnet Explore 的未列 bug，结果回来后评估：纳入本 plan 补 PR / 派生 v3 / 入 reminder。

---

## §1 强约束（新立 ECS Lifecycle 卫生规则）

以下 6 条规则在本 plan P0 决策门确认后写入 `docs/CLAUDE.md §四` 红旗：

1. **query 必加 `Without<Despawned>`**：所有 NPC query（除专门处理 Despawned 的 system）
2. **Action `Executing` 必转 `Failure`**：query miss / 目标消失 / 条件不满足时不允许 `continue`
3. **Executing 必有 `deadline` 超时**：所有 big-brain Action 注册时声明 `deadline_ticks`；无 deadline = 红旗
4. **deferred commands 不可信用 `Added<C>`**：同 tick deferred spawn 后不能立即假设 `Added<C>` 可见，需 1 tick 延迟
5. **register 不允许 panic**：资源文件 / JSON 加载路径全部 `Result<>` 传播，不 `unwrap` / `expect`
6. **多 zone 必按 zone filter**：所有 zone-scoped query（如 `plots.iter()`）必须先过滤目标 zone

---

## §2 阶段规划

| 阶段 | 内容 | 状态 |
|---|---|---|
| P0 | Bug #1 #2（高优，独立 PR）+ §3 强约束草案写入 CLAUDE.md §四 | ⬜ |
| P1 | Bug #3 #4 #5（中优，独立 PR） | ⬜ |
| P2 | Bug #6 #7 #8（中优，独立 PR） | ⬜ |
| P3 | sonnet Explore 二次探查剩余 3 bug 评估处理 | ⬜ |

## §3 验收标准

- `cargo test -p server npc` 全绿（每 bug 独立回归测试 pin 行为）
- plan-npc-perf-v1 baseline 在 P0 完成后录档
- plan-npc-virtualize-v1 hydrate-on-tribulation 在 P2 完成后可安全启动（#6 #8 修好）

## §4 开放问题（P0 决策门收口）

1. **CI grep 脚本**：是否在 CI 加 `grep -rn "Without<Despawned>"` 缺失检测脚本（轻量 lint）还是走 proc-macro 强约束
2. **Action 超时 default 值粒度**：farming Action 超时用统一 default（如 300 ticks = 15s at 20Hz）还是各 Action 自定义
3. **`docs/CLAUDE.md §四` 红旗升级**：§1 六条规则全部写入还是只写最高频（Without<Despawned> + deadline）
