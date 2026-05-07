# Bong · plan-multi-life-v1 · Finished

> **状态**：✅ finished 2026-05-07（P0/P1/P2/P3 全部 ✅，Finish Evidence 完整）。前置 plan-death-lifecycle-v1 ✅ finished + plan-tsy-loot-v1 ✅ finished + plan-spawn-tutorial-v1 ✅ finished + plan-lifespan-v1 ✅ finished（寿元数据 single source of truth）。
>
> **2026-05-04 reframe**（Q-ML0 A）：multi-life 不再独立维护寿元上限 / 死亡扣寿口径——**全部引用 lifespan-v1 §2**（`LifespanCapTable` + 死亡扣寿公式）。本 plan 仅管"角色终结后怎样开新角色"。

多周目机制:**per-life 运数**(每角色独立 3 次,**不跨角色累计**) + 寿命归零强制重开（依 lifespan-v1）+ 知识继承(玩家脑内 + 亡者博物馆) + 实力归零。**对应 plan-gameplay-journey-v1 §M.3**。

**世界观锚点**：`worldview.md §十二 死亡重生与一生记录` · `§十二 运数/劫数 Roll`（寿元上限引 lifespan-v1）

**交叉引用**：`plan-lifespan-v1` ⏳(寿元数据 source of truth，本 plan 全引用) · `plan-death-lifecycle-v1` ✅(运数实装,本 plan 检查是否 per-life) · `plan-tsy-loot-v1` ✅(道统遗物随机分散) · `plan-spawn-tutorial-v1` ✅(新角色出生 spawn_plain) · `plan-gameplay-journey-v1` §M.3/O.4/O.11

---

## 接入面 Checklist

- **进料**：玩家死亡事件 + 当前角色寿元 + 运数池
- **出料**：角色终结后的新角色生成 + 满运数(3 次)重置 + 知识继承(亡者博物馆生平卷可读)
- **共享类型**：复用 `LifeRecord` ✅ + `Aging` ✅ + `Karma` ✅
- **跨仓库契约**：server character_lifecycle + agent legacy narration + client character_select UI
- **worldview 锚点**：§十二

---

## §0 设计轴心

- [ ] **per-life 运数**(O.4 决策)：每角色独立 3 次,**不跨角色累计**——每新角色满运数重置
- [ ] **化虚 per-life 可达**(O.11 决策)：n 世玩家化虚不受影响,只看本世能否走通
- [ ] **知识继承不影响实力**：亡者博物馆可读 → 后人玩家可用脑内知识缩短路径,但物品/真元/境界仍归零
- [ ] **不允许跳过教学**：破坏 worldview 末法残土设定,新角色必须从醒灵走 + 第二世仍 spawn_plain（Q-ML4 决议）
- [ ] **无家族 / 无姓氏概念**（Q-ML1 B）：每个角色完全独立，避免出戏。亡者博物馆按 player_id 历代列出，但不归类家族
- [ ] **不允许主动放弃**（Q-ML2 A）：还有寿元 / 运数时不可主动重开——破坏末土残忍设定（worldview §十二"寿元宽裕"原意是逼人前进，主动重开会消解）
- [ ] **前世坐标不传承**（Q-ML3 B+ 精细化）：服务器不存"前世坐标"、客户端不显示——玩家自己脑内记。**前世物资 / 基地服务器不主动清除**（自然存在），**也不主动保护**——前世死后的物资箱子 / 基地可能被 NPC / 其他玩家搜刮 / 占据，玩家回去找发现"已被人翻过"是合理叙事（worldview §十"末土资源被翻是常态"）
- [ ] **道统遗物仅随机分散**（Q-ML5 A）：不做 plan-void-actions-v1 legacy_assign 化虚指定继承人。统一随机分散到 4 tsy 副本（worldview §十二"道统遗物天道分配"）

---

## §1 阶段总览

| 阶段 | 内容 | 验收 |
|---|---|---|
| **P0** ✅ 2026-05-07 | 检查 plan-death-lifecycle-v1 运数是否 per-life;若不是则改 | 单元测试: 新角色满运数 3 次 |
| **P1** ✅ 2026-05-07 | 寿命归零强制重开流程 | 寿元归零玩家自动进 character_select |
| **P2** ✅ 2026-05-07 | 道统遗物随机分散到 4 tsy 副本(已 ✅) + 知识继承 narration | 新角色可在副本拾到前世遗物 |
| **P3** ✅ 2026-05-07 | 角色史(亡者博物馆) library-web 公开页面 + 同玩家历代统计 | 玩家可看自己历代生平 |

---

## §2 关键流程

> **2026-05-04 Q-ML0 reframe**：寿元上限 + 死亡扣寿公式**全部引用 plan-lifespan-v1 §2**（`LifespanCapTable` 是 single source of truth）。本 plan 不另写一套数值。

```
角色死亡:
  寿元 -= 死亡扣除值（按 plan-lifespan-v1 §2，例如：被杀 = 当前境界寿元上限 × 5%；
                     渡劫失败 -100 年；老死即终结）
  运数池 -= 1(运数 > 0 时 100% 重生在灵龛)
  寿元 = 0 OR 运数耗尽 → 角色终结
   → LifeRecord 写入亡者博物馆（按 player_id 历代列出，不归类家族 Q-ML1）
   → NaturalDeathCorpse / 战斗死遗骸就地生成（plan-lifespan-v1 §5）
   → 前世物资 / 基地不主动清除（Q-ML3 决议，可能被 NPC/玩家搜刮）
   → 道统遗物随机分散到 4 tsy 副本（Q-ML5，不做 legacy_assign 指定继承人）
   → 玩家进入 character_select（不允许主动放弃 Q-ML2，仅在终结后进入）
   → 新角色生成: Realm = Awaken, 运数 = 3, 寿元 = 醒灵 cap（引 LifespanCapTable）,
                  spawn 位置 = spawn_plain（Q-ML4，与新玩家相同）, 物品/真元/境界 = 0

寿元上限:
  → 引 plan-lifespan-v1 §2 LifespanCapTable（醒灵 120 → 化虚 2000，详 lifespan plan）
```

---

## §3 数据契约

- [ ] `server/src/cultivation/lifespan.rs::on_death` 死亡扣寿（plan-lifespan-v1 P0 实施，本 plan **不重复**）
- [ ] `server/src/cultivation/character_lifecycle.rs::regenerate_or_terminate` 重生/终结判定（本 plan P0 主体）
- [ ] `server/src/cultivation/character_select.rs` 新角色生成 + spawn_plain 出生（Q-ML4）
- [ ] `server/src/cultivation/luck_pool.rs` per-life 运数池（**P0 验证 plan-death-lifecycle-v1 已实装是否 per-character**；不是则改）
- [ ] `agent/.../skills/era.md` "新一世" 开场 narration(协调 plan-spawn-tutorial-v1)
- [ ] `library-web/src/pages/lives/[player_id].astro` 历代生平统计（按 player_id，不按家族 Q-ML1）

---

## §4 开放问题

- [x] **Q-ML0 ✅**（user 2026-05-04 A，reframe）：完全引用 lifespan-v1 §2 寿元数据 + 扣寿公式（不另写一套）
- [x] **Q-ML1 ✅**（user 2026-05-04 B）：**无家族 / 无姓氏概念**——亡者博物馆按 player_id 历代列出
- [x] **Q-ML2 ✅**（user 2026-05-04 A）：**不允许主动放弃**——还有寿元 / 运数时不可主动重开
- [x] **Q-ML3 ✅**（user 2026-05-04 B+ 精细化）：**不传承前世坐标**——服务器不存 / 客户端不显示，玩家自己脑内记。**前世物资 / 基地服务器不主动清除**（自然存在）也不主动保护——可能被 NPC / 玩家搜刮
- [x] **Q-ML4 ✅**（user 2026-05-04 A）：第二世新角色出生位置 = spawn_plain（与新玩家相同）
- [x] **Q-ML5 ✅**（user 2026-05-04 A）：**仅随机分散** 4 tsy 副本，不做 plan-void-actions-v1 legacy_assign 化虚指定继承人

> **本 plan 无未拍开放问题**——P0 可立刻起。

## §5 进度日志

- 2026-05-01：骨架创建。plan-gameplay-journey-v1 §M.3 / O.4 / O.11 决策落点。
- **2026-05-04**：skeleton → active 升级（user 拍板）。**6 决策闭环 + reframe**：
  - Q-ML0 reframe：与 lifespan-v1 §2 数值对齐，不另维护
  - Q-ML1 无家族（亡者博物馆按 player_id）
  - Q-ML2 不允许主动放弃
  - Q-ML3 前世坐标不传承，物资不主动清也不主动保护（"末土残忍"）
  - Q-ML4 第二世仍 spawn_plain
  - Q-ML5 道统遗物仅随机分散（不做 legacy_assign）
  - 关键修正：原 §2 寿元上限表 (80/150/300/500/1000/2000) → 引 lifespan-v1 §2 LifespanCapTable (120/200/350/600/1000/2000)
  - 下一步起 P0 worktree（character_lifecycle::regenerate_or_terminate + 验证 luck_pool per-character + character_select spawn_plain 路径）
- **2026-05-07**：P0/P1/P2/P3 全部 ✅。`/consume-plan multi-life-v1` 一次过：cultivation 三个新模块（luck_pool / character_select / character_lifecycle）+ era skill 新一世 narration 章节 + library-web /lives/[player_id].astro。详 Finish Evidence。

## Finish Evidence

### 落地清单

* **P0 — luck_pool / character_select / character_lifecycle 三个 cultivation 子模块**
  * `server/src/cultivation/luck_pool.rs`：per-life 运数池 API（`INITIAL_FORTUNE_PER_LIFE=3` + `current_fortune` / `is_exhausted` / `spend_fortune` / `reset_for_new_life`）
  * `server/src/cultivation/character_select.rs`：`NewCharacterSpec { spawn_pos, realm, initial_fortune, lifespan_cap }` 单一数据源 + `next_character_spec()` 唯一入口
  * `server/src/cultivation/character_lifecycle.rs`：纯决策函数 `regenerate_or_terminate` + `LifeOutcome::{Revive, Terminate}` + `TerminateReason::{NaturalAging, LifespanExhausted, FortuneExhausted}`
  * `server/src/cultivation/mod.rs`：注册三个 `pub mod`
  * `server/src/combat/lifecycle.rs::reset_for_new_character`：从 `next_character_spec()` 读 spec（修正旧 bug：cap 用 `MORTAL=80` 与 attach 路径 `AWAKEN=120` 数值漂移），fortune 重置改走 `luck_pool::reset_for_new_life`
* **P1 — 寿命归零强制重开流程集成测试**
  * `server/src/cultivation/character_lifecycle.rs::tests`：4 条贯穿决策器与 spec 提供器的集成测试（`lifespan_zero_to_new_life_spec_full_chain` / `natural_aging_to_new_life_spec_full_chain` / `fortune_exhausted_to_new_life_spec_resets_pool` / `revive_outcome_does_not_use_new_life_spec`）
  * 既有 combat::lifecycle 流程 (`lifespan_aging_tick → CultivationDeathTrigger{NaturalAging} → terminate_lifecycle_with_death_context → CreateNewCharacter → reset_for_new_character`) 已自动用上 P0 的 spec
* **P2 — agent era skill 新一世开场 narration**
  * `agent/packages/tiandao/src/skills/era.md`：新增"新一世开场叙事"章节，规定触发条件 / scope / 文风 / 禁忌（不泄前世坐标 / 不暴名 / 无家族姓氏 / 无实力继承）
  * 道统遗物随机分散到 4 tsy 副本：plan-tsy-loot-v1 finished 已 ✅ 实装（`ItemRarity::Ancient` + `server/src/inventory/ancient_relics.rs` + 99/1 loot table），本 plan §0 Q-ML5 引用即可
* **P3 — library-web 历代生平页面**
  * `library-web/src/pages/lives/[player_id].astro`：动态路由按 player_id（`offline:<username>` 前两段）聚合 `_index.json` 中同玩家的历代角色卷宗
  * `getStaticPaths` 在 build 阶段从 `public/deceased/_index.json` 提取 player_id 集合预生成静态页面，URL 编码 `:` → `-`（`offline:Ancestor` → `/lives/offline-Ancestor`）
  * 客户端 JS 按 `died_at_tick` 升序显示"第 N 世"列表，链接到现有 `/deceased/view?path=...` 单卷详情

### 关键 commit

| hash | 日期 | 一句话 |
|---|---|---|
| `ee17585` | 2026-05-07 | P0 引入 luck_pool / character_select / character_lifecycle 三个 cultivation 子模块 + reset_for_new_character 修复 cap 漂移 bug |
| `bf01305` | 2026-05-07 | P1 寿命归零 → character_select 流程的端到端集成测试 (+4 测试) |
| `0999d1e` | 2026-05-07 | P2 era skill 新增"新一世"开场 narration 章节 + 道统遗物引用 |
| `23571c3` | 2026-05-07 | P3 历代生平页面 lives/[player_id].astro |
| `9fb5bee` | 2026-05-07 | fmt 微调 character_lifecycle 测试块 |

### 测试结果

* `cd server && cargo fmt --check` → 干净
* `cd server && cargo test cultivation::luck_pool` → 9 passed
* `cd server && cargo test cultivation::character` → 19 passed（character_select 6 + character_lifecycle 13 → 后续追加 P1 集成测试至 17）
* `cd server && cargo test combat::lifecycle` → 22 passed（含 reset_for_new_character 旧测试更新到 AWAKEN cap）
* `cd server && cargo test cultivation` → 321 passed
* `cd server && cargo test` 全栈 → **2554 passed; 0 failed**
* `cd agent && npm install && npm run build` → schema + tiandao 全绿
* `cd agent/packages/tiandao && npm test` → 38 files / 256 passed
* `cd library-web && LOCAL_LIBRARY_PATH=../docs/library npm run build` → 41 pages built（含 `/lives/offline-Ancestor/index.html` 自动从现有 `_index.json` 中的 `offline:Ancestor` 生成）
* `cargo clippy --all-targets` → 我加的三个新模块 + combat/lifecycle.rs 改动 0 warning（main 已存在的 13 warning 不在本 plan 范围）

### 跨仓库核验

* **server**：`cultivation::luck_pool::INITIAL_FORTUNE_PER_LIFE` / `cultivation::character_select::NewCharacterSpec` / `cultivation::character_lifecycle::{LifeOutcome, TerminateReason, regenerate_or_terminate}` / `combat::lifecycle::reset_for_new_character` 接 spec
* **agent**：`agent/packages/tiandao/src/skills/era.md` 新一世开场叙事章节（规则文档，无新 schema）
* **library-web**：`/lives/<encoded-player-id>` 动态路由（构建期从 `public/deceased/_index.json` 提取 player_id 集合）

### 遗留 / 后续

* **client UI**："再来一世"按钮的具体视觉与 emit_terminate_screen 的端到端 e2e 联调待 client 改 plan 跟进——server 侧 `RevivalActionKind::CreateNewCharacter` 协议已稳定，client 现状已能触发该 intent
* **library-web 顶级入口**：本 plan 只加了 `/lives/[player_id]` 单玩家页，"列出所有玩家 player_id" 的索引页 (`/lives/index.astro`) 暂未做——可作后续小补丁，从同一 `_index.json` 聚合
* **agent 新一世 narration 实运行触发**：era.md 已定义触发条件与文风，但 era Agent 的 `narrations` payload 路由（player vs broadcast）与 spawn_tutorial 端协调具体由后续 agent plan 决定，本 plan 只锁文档接口
* **lifespan-v1 P3 化虚化路径**：plan §0 第 2 条"化虚 per-life 可达"已由 lifespan-v1 数值表保证，但化虚 quota 终结/释放在多周目后是否需要额外语义跟踪，待 lifespan-v1 P4+ 推进时确认
