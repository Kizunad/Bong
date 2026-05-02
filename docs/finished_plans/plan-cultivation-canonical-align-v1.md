# Bong · plan-cultivation-canonical-align-v1

把 cultivation 模块从 MVP 压缩值对齐到 worldview 正典值。**Wave 0 硬阻塞**——所有下游 plan 依赖此对齐完成，整个 100h 路径成立的物理前提。

**世界观锚点**：`worldview.md §三 修炼体系` line 67-153（六境界 + 突破条件 + 时长基线 0.5/3/8/15/25h）

**library 锚点**：`cultivation-0003 六境要录`（六境正典）· `cultivation-0006 淬炼与顿悟杂记`（十二正经 + 八奇经分布）· `cultivation-0001 爆脉流正法`（爆脉代价分级）

**交叉引用**：`plan-cultivation-v1` ✅（待对齐源）· `plan-cultivation-mvp-cleanup-v1` ✅ · `plan-gameplay-journey-v1` §I/§Q.1 Wave 0/O.1（决策来源）

---

## 接入面 Checklist

- **进料**：`server/src/cultivation/components.rs` 当前 `Realm` enum + `required_meridians()` MVP 压缩值 `[0,1,4,8,14,20]`
- **出料**：对齐后的常量表 + 同步更新的 150 测试 + 全栈文案统一
- **共享类型**：复用 `Realm` enum / `MeridianId` / `BreakthroughRequest` ——**不新增类型**
- **跨仓库契约**：server cultivation/* + agent schema/cultivation.ts + client 两套 PlayerStateViewModel + schema sample
- **worldview 锚点**：§三 line 100-131（突破条件）+ line 133-153（时长基线）

---

## §0 设计轴心

- [ ] **不留技术债**（O.1 决策）：本 plan P0 段必须直接对齐到正典值，不允许 MVP 压缩 → 正典 v2 的二次迁移
- [ ] **51.5h 时长基线锁定**：worldview §三 line 133-153 给的 0.5/3/8/15/25h 是设计目标；本 plan 只做门槛对齐，XP 曲线独立模块化留给后续 plan
- [ ] **测试同步**：150 个测试用例必须连同公式一起更新，不允许公式改了测试不改
- [ ] **不修改 worldview**：worldview 是正典，本 plan 是代码追正典，反向不允许
- [ ] **wire 格式保持英文**：`realm_to_string` / `realm_from_string` 维持 `Awaken/Induce/...` 英文串，不在线上层做破坏性变更；UI 本地化在 client 端完成

---

## §1 已识别差异（实地核验版 · 2026-05-01）

### 1.1 核心数值差异

| 项 | 代码现状 | worldview 正典 | 对齐动作 |
|---|---|---|---|
| `required_meridians()` 总数阈值 | `[0, 1, 4, 8, 14, 20]` | **`[1, 3, 6, 12, 16, 20]`**（total 维度） | 改 5 个 match 分支值（仅用于降境 keep / NPC 评分等 total 场景） |
| **突破前置条件缺少正经/奇经分结构检查** | `breakthrough_precondition_error` 只用 `opened_count()` 比 total（line 151-156） | 固元需「12 正经全通」、通灵需「12 正经全通 + 奇经 4 条」、化虚需「奇经全通」 | **新增 `regular_opened_count()` / `extraordinary_opened_count()` helper + 在 precondition 中逐境做分结构检查** |
| `Realm` doc comment | "觉醒/引灵/凝气/灵动/虚明" (components.rs:25-30) | "醒灵/引气/凝脉/固元/通灵/化虚" | 更新 6 行注释 |
| `breakthrough.rs` 头注释 | 已写 "醒灵=3·引气=5·凝脉=7·固元=8·通灵=9·化虚=10" (line 76-77) | — | ✅ 注释已对，无需改 |
| `xp_curve.rs` | **不存在** | plan §3 声称有此文件 | 延后到后续 plan（当前 completeness 公式 `1+0.05×(have-need)` 在 breakthrough.rs 中工作正常） |
| `qi_zero_decay.rs` demo 常量 | `ZERO_THRESHOLD_RATIO=0.01`（即 1%） | worldview "低于上限 20%" → 应为 0.2 | 延后（改阈值影响玩家体感，需 telemetry 回填） |
| `qi_zero_decay.rs` 头注释声称 `opened_at` 需新增字段 | `Meridian` **已有** `opened_at: u64` 字段 (components.rs) | — | 本次修正注释一致性（仅改注释，不改逻辑） |

### 1.2 Realm 命名全景

| 层 | 当前值 | 目标 |
|---|---|---|
| server `Realm` enum variant | `Awaken / Induce / Condense / Solidify / Spirit / Void` | **不变**（enum variant 名不改，仅改注释） |
| server doc comment | 觉醒 / 引灵 / 凝气 / 灵动 / 虚明 | 醒灵 / 引气 / 凝脉 / 固元 / 通灵 / 化虚 |
| `realm_to_string` wire 输出 | 英文 variant 名 | **不变**（wire 保持英文） |
| agent schema `Realm` literal | 英文 variant 名 | **不变**（description 更新） |
| agent tiandao prompt | 混用中英文 | 统一为正典中文名 |
| client `CultivationScreen` | 显示 raw 英文（`境界: Induce`） | 改为中文：`境界: 引气` |
| client 旧 `PlayerStateViewModel` | `humanizeRealm()` 映射到中文 | ✅ 已对（但需确认正典名一致） |
| client `skillCapForRealm` | 用英文 switch | **不变**（映射逻辑不受影响） |

### 1.3 文件路径修正

| plan 声称路径 | 实际路径 | 处理 |
|---|---|---|
| `client/.../cultivation/CultivationScreen.java` | `client/.../ui/CultivationScreen.java` | 修正引用 |
| `client/.../cultivation/MeridianHud.java` | **不存在**；经脉 UI 在 `MeridianDetailPanel.java` + `MeridianMiniView.java` | 修正引用 |
| `check_meridian_count` 独立函数 | **不存在**；逻辑内联在 `breakthrough_precondition_error()` | 修正引用 |

---

## §2 阶段总览

### P0 ⬜ — 核心数值对齐 + 分结构前置检查（硬阻塞，必须首发）

**内容**：

#### A. 新增 helper（`MeridianSystem` 在 `components.rs`）
```rust
impl MeridianSystem {
    pub fn regular_opened_count(&self) -> usize {
        self.regular.iter().filter(|m| m.opened).count()
    }
    pub fn extraordinary_opened_count(&self) -> usize {
        self.extraordinary.iter().filter(|m| m.opened).count()
    }
}
```

#### B. 改 `required_meridians()` 总数值（保留原签名，用于降境 keep / NPC 评分等 total 场景）
- `Awaken → 1`（原 0）
- `Induce → 3`（原 1）
- `Condense → 6`（原 4）
- `Solidify → 12`（原 8）
- `Spirit → 16`（原 14）
- `Void → 20`（不变）

#### C. 重写 `breakthrough_precondition_error`（`breakthrough.rs`）
在现有 `opened_count() >= required_meridians()` 总量检查之后，增加逐境分结构检查：

```rust
// 总量快速拒绝（保留）
let need_total = next.required_meridians();
if meridians.opened_count() < need_total {
    return Some(BreakthroughError::NotEnoughMeridians { need: need_total, have });
}
// 逐境结构检查（新增）
match next {
    Realm::Induce => {
        if meridians.regular_opened_count() < 3 {
            return Some(BreakthroughError::NotEnoughRegularMeridians { need: 3, have: reg });
        }
    }
    Realm::Condense => {
        if meridians.regular_opened_count() < 6 {
            return Some(BreakthroughError::NotEnoughRegularMeridians { need: 6, have: reg });
        }
    }
    Realm::Solidify => {
        if meridians.regular_opened_count() < 12 {
            return Some(BreakthroughError::NotEnoughRegularMeridians { need: 12, have: reg });
        }
    }
    Realm::Spirit => {
        if meridians.regular_opened_count() < 12 {
            return Some(BreakthroughError::NotEnoughRegularMeridians { need: 12, have: reg });
        }
        if meridians.extraordinary_opened_count() < 4 {
            return Some(BreakthroughError::NotEnoughExtraordinaryMeridians { need: 4, have: ext });
        }
    }
    // Void 走 tribulation，不在此处触发
    _ => {}
}
```

新增错误变体（`BreakthroughError` enum）：
- `NotEnoughRegularMeridians { need: usize, have: usize }`
- `NotEnoughExtraordinaryMeridians { need: usize, have: usize }`

#### D. 更新 `Realm` enum 6 行中文注释

#### E. 同步更新 150 个测试
- `required_meridians()` 相关断言：数值从旧 MVP 值更新
- 新增结构检查测试（见验收）
- `tribulation.rs:425`：`Spirit.required_meridians()` 从 14→16

#### F. 修正 `qi_zero_decay.rs` 头注释
- 删除 "`opened_at` 尚未在结构上记录（需要新增字段）"（`Meridian` 已有 `opened_at: u64`）

**影响面**（跨 5 个核心文件 + npc/brain.rs）：
- `breakthrough.rs`：前置条件检查 → **重写**（新增结构检查）；completeness 公式 → 自动适配（仍用 total）
- `death_hooks.rs`：降境后 `keep = required_meridians()` → 自动适配（仍用 total）
- `qi_zero_decay.rs`：归零后 `keep = to.required_meridians()` → 自动适配（仍用 total）
- `tribulation.rs`：渡劫失败 → 值从 14→16（仍用 total），另需确认 tribulation 触发前是否应检查奇经全通语义
- `npc/brain.rs`：`realm_progress_score` → 自动适配（仍用 total）

**验收**：
- `cargo test -p bong-server -- cultivation` 全绿
- `cargo test -p bong-server`（全量）无回归
- 单调性：`[1, 3, 6, 12, 16, 20]` 严格递增
- **新增自动化测试（必须）**：
  1. **固元结构测试**：构造 10 正经 + 6 奇经 = 16 total（≥12），但 `regular_opened < 12`，断言 `Condense → Solidify` 突破**被拒**（`NotEnoughRegularMeridians`）
  2. **通灵结构测试**：构造 12 正经 + 2 奇经 = 14 total（<16 故总量先拒），或 12 正经 + 3 奇经 = 15 total（<16），或 12 正经 + 5 奇经 = 17 total 但 regular 不足 → 均须结构正确才放行
  3. **奇经凑数绕过测试**：构造 8 正经 + 8 奇经 = 16 total（满足 total≥16 且 ext≥4），但 `regular_opened < 12`，断言 `Solidify → Spirit` 突破**被拒**

### P1 ⬜ — 全栈文案统一

**内容**：
1. server：更新 `components.rs` 的 `Realm` 注释（已在 P0 完成）
2. agent schema：更新 `cultivation.ts` 中 `Realm` 的 `description` 为 "6 境界：醒灵/引气/凝脉/固元/通灵/化虚"
3. agent tiandao：
   - `death-insight-runtime.ts`：prompt 中的中文名确认准确
   - `mock-state.ts`：sample realm 值确认准确（英文不变，仅确认语义正确）
4. client：
   - **统一** `CultivationScreen.java` 和旧 `PlayerStateViewModel.humanizeRealm()` 到**同一个共享 helper**（放在 `com.bong.client.util.RealmLabel` 或类似）
   - `CultivationScreen.java` 输出从 raw 英文改为中文标签
   - 更新 `CultivationScreenTest`：期望值从 `"境界: Induce"` → `"境界: 引气"`
5. grep 全仓库确认无残留旧境界中文名（觉醒/引灵/凝气/灵动/虚明）——**仅限注释和 UI 文案，不包括 git 历史和文档引用**

**验收**：
- `npm run build`（agent）无错误
- `./gradlew test`（client）全绿
- `grep -rn '觉醒\|引灵\|凝气\|灵动\|虚明' server/src/ agent/ client/src/ --include='*.rs' --include='*.ts' --include='*.java' | grep -v 'target/' | grep -v 'node_modules/' | grep -v '.md'` 返回空

### P2 ⬜ — client 经脉图标注（可选收口）

> 注：奇经 4 条的结构检查已前移到 P0。P2 仅做 client UI 可视化。

**内容**：
1. 正典要求"固元→通灵 需要奇经八脉通 4 条"，但 worldview 和 `cultivation-0006` 均未指定是哪 4 条。本 plan 做务实决策：
   - **任选 4 条即可**（P0 已用 `extraordinary_opened_count() >= 4` 实现）
   - 如果 worldview 后续明确具体 4 条，仅需改 P0 的一个常量 + 对应测试
2. client `MeridianDetailPanel.java`：经脉图上标注"此脉开启则计入固元→通灵门槛"（奇经高亮 + 计数 `N/4`）
3. `MeridianMiniView.java`：战斗内经脉摘要显示奇经计数（如 `奇经 2/4`）

**验收**：
- client 经脉面板显示 "奇经 N/4" 指示器
- `./gradlew test`（client）无回归

---

## §3 数据契约（修正版）

### P0 核心改动
- [ ] `server/src/cultivation/components.rs` — 新增 `MeridianSystem::regular_opened_count()` / `extraordinary_opened_count()` helper
- [ ] `server/src/cultivation/components.rs:333-363` — `required_meridians()` 6 个 match 分支值更新
- [ ] `server/src/cultivation/components.rs:25-30` — `Realm` enum 6 行中文注释更新
- [ ] `server/src/cultivation/components.rs:341-365` — `required_meridians` 测试断言值更新
- [ ] `server/src/cultivation/breakthrough.rs:145-160` — `breakthrough_precondition_error` 增加逐境正经/奇经分结构检查
- [ ] `server/src/cultivation/breakthrough.rs` — `BreakthroughError` enum 新增 `NotEnoughRegularMeridians` / `NotEnoughExtraordinaryMeridians` 变体
- [ ] `server/src/cultivation/breakthrough.rs` — 测试新增结构约束用例（固元/通灵/奇经凑数绕过 3 个场景）
- [ ] `server/src/cultivation/breakthrough.rs` — 测试断言中依赖旧阈值的更新
- [ ] `server/src/cultivation/tribulation.rs:425` — `Spirit.required_meridians()` 值从 14→16
- [ ] `server/src/cultivation/qi_zero_decay.rs` — 修正头注释（删除 `opened_at` 不存在声明）
- [ ] `server/src/cultivation/death_hooks.rs` / `qi_zero_decay.rs` — 确认降境 keep 自动适配，无需手动改公式

### P1 文案改动
- [ ] `agent/packages/schema/src/cultivation.ts:23-34` — `Realm` description 更新
- [ ] `agent/packages/tiandao/src/death-insight-runtime.ts` — prompt 中文名确认
- [ ] `client/.../ui/CultivationScreen.java:61-97` — 改用共享中文 helper
- [ ] `client/.../state/PlayerStateViewModel.java` — 或 merge 到共享 helper
- [ ] `client/.../PlayerStateViewModel.java:95-177` — `humanizeRealm()` 确认正典名
- [ ] `client/.../network/PlayerStateHandler.java` — raw string 不变，仅确认
- [ ] 所有 client/server/agent 测试中硬编码的旧中文名替换

### P2 收口改动
- [ ] `client/.../inventory/component/MeridianDetailPanel.java` — 奇经 N/4 计数指示器
- [ ] `client/.../combat/inspect/MeridianMiniView.java` — 奇经计数摘要

---

## §4 风险

| 风险 | 等级 | 缓解 |
|---|---|---|
| **奇经凑数绕过 12 正经全通（确定性漏洞）** | 🔴 高 | P0 加入正经/奇经分结构检查（`regular_opened_count` / `extraordinary_opened_count`），不只看 total |
| **Awaken 门槛从 0→1**：降境到 Awaken 后 `keep = 1` 会强制保留 1 条经脉（原 `keep=0` 允许全关） | 🔴 高 | `death_hooks` 中 `keep = required_meridians().max(if realm==Awaken {0} else {1})` 已兜底（Awaken 特殊处理保持 0）。但仍需确认 `qi_zero_decay` 降境到 Awaken 是否兼容 |
| **150 测试更新可能漏** | 🟡 中 | 先跑 `cargo test -- cultivation` 看全量失败数，逐文件修；改完再跑全量 `cargo test` |
| **npc/brain.rs 依赖 required_meridians 做 AI 评分** — 值变大后 npc 突破节奏显著变慢 | 🟡 中 | 接受此变慢为正典意图（末法残土突破应慢）。但不调参，仅在本 plan 记录预期变更：`realm_progress_score = opened/needed.max(1)` 在同 opened 下从旧值下降百分比见下表 |
| **npc AI 节奏量化参考**：同 opened 数下 progress 降幅 | 🟡 | e.g. 开 3 脉时 Induce progress: 旧 3/1=100% → 新 3/3=100%（不变）；开 5 脉时 Condense progress: 旧 5/4=100% → 新 5/6=83%（降 17%）；开 9 脉时 Solidify progress: 旧 9/8=100% → 新 9/12=75%（降 25%）。最显著影响在 Solidify 区间 |
| **realm_from_string 对未知值默认回落到 Awaken** | 🟡 中 | 本 plan 不改 variant 名，风险不触发。后续独立 hardening PR 改为 `Option<Realm>` |
| **client 两套 PlayerStateViewModel 并行** | 🟢 低 | 统一到共享 helper 即可，不涉及协议变更 |
| **奇经 4 条未指定具体经脉** — 后续 worldview 可能明确是 Ren/Du/Chong/Dai | 🟡 中 | 本 plan 取"任选 4 条"。若后续指定，仅需改 P0 的一个常量 + 对应测试（回改成本低） |

---

## §5 开放问题

- [x] 奇经 4 通（固元→通灵 必需）是哪 4 条？→ **决策：任意 4 条奇经即可**（2026-05-01）
- [x] 突破前置条件是否应区分正经/奇经？→ **是。P0 已纳入分结构检查**（2026-05-01 审批修正）
- [ ] XP 曲线是否同时支持 §F 三栏占比（修炼 50% 等）？→ **延后**，本 plan 只做门槛对齐，XP 曲线独立模块化留给后续 plan
- [ ] `required_meridians()` 改动是否需要 server schema version bump？→ 不改 variant 名、不改 wire 格式，**不需要** version bump
- [ ] `qi_zero_decay.rs` demo 常量（1% 阈值）何时升到正典 20%？→ **延后**，需 telemetry 回填再调
- [ ] `opened_at` 字段已存在但 `qi_zero_decay.rs` 实现层仍用数组索引代理 → **延后**（单独小 issue）；注释已在 P0 修正

---

## §6 进度日志

- 2026-05-01：骨架创建。plan-gameplay-journey-v1 §Q Wave 0 硬阻塞，所有下游依赖。
- 2026-05-01：实地核验完成。确认代码现状与正典差异（详见 §1），填充 §0-§5 完整设计。
- 2026-05-01：审批驳回（reviewer）。P0 从「total count 常量表」升级为「正经/奇经分结构检查」，新增 `regular_opened_count()` / `extraordinary_opened_count()` helper + 3 个结构约束测试。P2 收窄为纯 client UI。风险表补全。

## Finish Evidence

### 落地清单

- P0 核心门槛：`server/src/cultivation/components.rs` 将 `Realm::required_meridians()` 对齐为 `[1, 3, 6, 12, 16, 20]`，新增 `MeridianSystem::regular_opened_count()` / `extraordinary_opened_count()`；`server/src/cultivation/breakthrough.rs` 增加正经/奇经结构前置检查和 `NotEnoughRegularMeridians` / `NotEnoughExtraordinaryMeridians` 错误变体。
- P0 回归面：`server/src/cultivation/death_hooks.rs`、`server/src/cultivation/qi_zero_decay.rs`、`server/src/network/mod.rs`、`server/src/npc/brain.rs` 已同步正典阈值测试与注释；`server/src/cultivation/tribulation.rs` 继续通过 `Realm::Spirit.required_meridians()` 自动使用 16。
- P1 文案：`agent/packages/schema/src/cultivation.ts` 与 generated schema 将 Realm description 更新为「醒灵/引气/凝脉/固元/通灵/化虚」；`client/src/main/java/com/bong/client/util/RealmLabel.java` 统一 client 端 wire realm 中文标签；`CultivationScreen`、旧 `PlayerStateViewModel`、库存面板使用共享 helper。
- P2 可视化：`client/src/main/java/com/bong/client/util/MeridianGateLabel.java` 提供奇经 `N/4` 计数；`MeridianDetailPanel.java` 与 `MeridianMiniView.java` 显示通灵门槛提示。
- 流水线清理：删除过期同名 skeleton `docs/plans-skeleton/plan-cultivation-canonical-align-v1.md`，避免 active plan 消费前置校验误判。

### 关键 commit

- `2961dff4` · 2026-05-02 · `docs(plan-cultivation-canonical-align-v1): 移除过期骨架`
- `b4c8fdcd` · 2026-05-02 · `fix(cultivation): 对齐正典经脉门槛与结构检查`
- `5bf30d8f` · 2026-05-02 · `fix(client): 统一境界标签与奇经门槛提示`
- `da090ea6` · 2026-05-02 · `fix(schema): 更新境界正典描述`

### 测试结果

- `cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test -p bong-server -- cultivation`：247 passed。
- `cd server && cargo test`：2054 passed。
- `cd agent && npm run build && (cd packages/tiandao && npm test) && (cd packages/schema && npm test)`：tiandao 205 passed；schema 236 passed。
- `JAVA_HOME=/home/kiz/.sdkman/candidates/java/17.0.18-amzn cd client && ./gradlew test build`：BUILD SUCCESSFUL；JUnit XML 汇总 762 tests。
- `rg -n '觉醒|引灵|凝气|灵动|虚明' server/src agent client/src --glob '*.rs' --glob '*.ts' --glob '*.java'`：返回空。

### 跨仓库核验

- server：`Realm::required_meridians`、`MeridianSystem::regular_opened_count`、`MeridianSystem::extraordinary_opened_count`、`BreakthroughError::NotEnoughRegularMeridians`、`BreakthroughError::NotEnoughExtraordinaryMeridians`。
- agent：`agent/packages/schema/src/cultivation.ts` 的 `Realm` TypeBox description；generated `world-state-v1.json` / `breakthrough-event-v1.json` / `death-insight-request-v1.json` 等同步描述。
- client：`RealmLabel.displayName`、`MeridianGateLabel.spiritExtraordinaryProgress`、`CultivationScreen.describe`、`MeridianDetailPanel`、`MeridianMiniView`。

### 遗留 / 后续

- XP 曲线独立模块化仍按 plan 决策延期，不在本 PR 范围。
- `qi_zero_decay.rs` 的 1% demo 阈值升到 worldview 20% 仍需 telemetry 回填后单独调整。
- `opened_at` 已存在，但归零降境关闭排序仍用数组索引代理；真实 tick 排序留给后续小 issue。
