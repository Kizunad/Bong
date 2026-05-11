# Bong · plan-coffin-v1

凡物棺材——玩家可进入棺材以减缓寿命流逝速度（×0.9），用于下线/挂机时降低寿元消耗。暂时用双箱（两个 `BlockState::CHEST` 并排）作为占位视觉，后续可换专用模型。

> **2026-05-11 依赖实地核验通过**：`LifespanComponent`（`n`）已完整实装（`server/src/cultivation/lifespan.rs`，1500+ 行，26 个单元测试）。`LIFESPAN_OFFLINE_MULTIPLIER=0.1` + `lifespan_delta_years_for_real_seconds` + `lifespan_tick_rate_multiplier`（zone/season 感知）+ `offline_pause_tick` 离线断线重连补齐全链路就绪。audio trigger 框架（`server/src/network/audio_trigger.rs`）＋ HUD planner 模式（`client/.../hud/*.java`）就绪。

> **2026-05-11 审核修订**：(1) 双箱是两个 `BlockState::CHEST` 并排放置，非单个方块类型；(2) 不用 GameMode::Spectator 隐身，改用 set_invisible + movement system 拦截；(3) `lifespan_tick_rate_multiplier` 签名不改，coffin 因子在 `lifespan_aging_tick` 中额外乘算；(4) 本项目无自定义 Block 注册先例，棺材实体用内存 `HashMap<BlockPos, CoffinEntity>` 替代 Valence BlockEntity；(5) `player_lifespan` 表 `ALTER TABLE ADD COLUMN in_coffin`。

**世界观锚点**：`worldview.md §十四` 玩家循环明确有"回家"环节，棺材是回家后在灵龛内放置的挂机安全设施。凡物棺材仅提供寿命减缓，无其他超凡功效。

**前置依赖**：
- `plan-death-lifecycle-v1` ✅ — `LifespanComponent` / `LIFESPAN_OFFLINE_MULTIPLIER` / `offline_pause_tick`
- `plan-niche-defense-v1` ✅ — 灵龛内可放置方块
- `plan-inventory-v1` ✅ — 玩家背包/手持物品
- `plan-audio-world-v1` 🆕 active — 音频事件框架
- `plan-HUD-v1` ✅ — HUD 注册/渲染管线

**反向被依赖**：（无）

---

## 边界：本 plan 做什么 & 不做什么

| 维度 | 范围 | 不做 |
|------|------|------|
| 棺材种类 | 凡物棺材（×0.9 寿命减缓） | 灵材棺材/符文棺材（后续 plan） |
| 模型 | 双箱 block variant + blockstate | 专用模型建模 |
| 放置位置 | 灵龛内任意空地 | 野外放置限制（暂无） |
| 多人 | 一人一棺，不可共享 | 双人棺材 |
| 音效 | 进入/离开/环境低鸣 3 条 | 棺材材质区分音效 |
| 寿命 | 在线 ticking ×0.9 + 离线回算 ×0.09 | 寿元回复/逆转 |

---

## §0 设计轴心

- [ ] **最小侵入**：棺材只改寿命流逝速率，不改修炼/战斗/背包等任何其他系统
- [ ] **双箱即用**：用 Minecraft vanilla Double Chest model + blockstate 作为凡物棺材外观，暂不引入自定义模型
- [ ] **挂机友好**：在线 ticking 走 `lifespan_tick_rate_multiplier` +0.9 因子；离线回算走 `LIFESPAN_OFFLINE_MULTIPLIER` ×0.9，上下线节奏一致
- [ ] **HUD 明确**：进入棺材后屏幕边缘叠加半透明"卧棺"提示 + 寿命流逝速率变化 indicator
- [ ] **音效氛围**：进入低沉空洞音 + 离开木板摩擦音 + 在棺中环境心跳/呼吸变慢 ambient loop

---

## 阶段总览

| 阶段 | 内容 | 状态 |
|----|------|----|
| P0 | 棺材物品注册 + 双箱放置逻辑 + 内存实体追踪 | ⬜ |
| P1 | 进入/离开棺材交互 + CoffinComponent | ⬜ |
| P2 | 寿命流逝速率 ×0.9（online tick + offline 回算） | ⬜ |
| P3 | HUD："卧棺"状态 + 寿命速率提示 | ⬜ |
| P4 | 音效：进入/离开/环境低鸣 | ⬜ |
| P5 | 全链路 E2E：放置→进入→挂机→寿命验证→离开 | ⬜ |

---

## P0 — 棺材物品注册 + 双箱放置逻辑 ⬜

> **技术约束**：本项目无自定义 Block 类型注册先例。棺材实体用内存 `HashMap<BlockPos, CoffinEntity>` 追踪（`server/src/coffin/registry.rs`），不依赖 Valence BlockEntity。双箱 = 两个 `BlockState::CHEST` 并排，共享一个实体记录。

### 交付物

1. **棺材物品定义**（`server/src/coffin/item.rs`）
   - `CoffinItem`：可堆叠 1，手持右键放置
   - 配方：6 木板 + 2 木棍（工作台合成，形状类似床）
   - 注册到 `ItemRegistry`

2. **双箱放置逻辑**（`server/src/coffin/place.rs`）
   - 监听玩家右键事件（手持 CoffinItem）→ 检查目标位置是否合法（灵龛范围内 + 无阻挡）
   - 放置两个 `BlockState::CHEST`：
     - 下半（底箱）：`BlockState::CHEST` + `facing=player_facing` + `type=single`
     - 上半（盖）：底箱上方一格，同样 `BlockState::CHEST` + 相同 facing
   - 两个 chest 属于同一个棺材，写入 `CoffinRegistry`（`HashMap<BlockPos, CoffinEntity>`）——两个 BlockPos 指向同一 entity
   - 破坏任意一半 = 破坏整个棺材：移除两个 BlockState → 掉落 1 个 CoffinItem → 清除 registry 条目

3. **棺材实体追踪**（`server/src/coffin/registry.rs`）
   ```rust
   #[derive(Resource, Default)]
   pub struct CoffinRegistry {
       pub coffins: HashMap<BlockPos, CoffinEntity>,
       /// 每个玩家只能同时在一口棺材里
       pub player_in_coffin: HashMap<Entity, BlockPos>,
   }

   pub struct CoffinEntity {
       pub lower: BlockPos,   // 底箱位置
       pub upper: BlockPos,   // 盖位置
       pub occupied_by: Option<Entity>,
       pub placed_at_tick: u64,
   }
   ```
   - `register()` 注入 `CoffinRegistry` resource

### 验收抓手

- 测试：可合成棺材 → 手持右键 → 两个 chest 方块并排出现 → 破坏任意半 → 双箱消失 + 掉落 CoffinItem
- E2E：客户端看到两个 chest 组成的双箱模型；Serenity 不会因为同坐标双箱而 block state 错乱

---

## P1 — 进入/离开棺材交互 ⬜

### 交付物

1. **右键交互**（`server/src/coffin/interact.rs`）
   - 空手右键棺材（底箱或盖任意半）→ 进入：
     - 玩家模型隐形：Valence `entity.set_invisible(true)`
     - 玩家位置锁定到棺材底箱中心
     - 玩家获得 `CoffinComponent { entered_at_tick, coffin_lower: BlockPos }`
     - `CoffinRegistry` 标记 `player_in_coffin[player_entity] = coffin_lower`
     - 无法移动/攻击/使用物品（在 `player/mod.rs` movement system 中检查 `CoffinComponent` → 跳过移动处理）
   - 在棺材中按 Sneak（潜行）→ 离开：
     - `entity.set_invisible(false)`，恢复可见
     - 移除 `CoffinComponent`
     - 清除 `CoffinRegistry.player_in_coffin` 条目
     - 玩家出现在棺材旁边一格（底箱 +1 格水平偏移）
   - 棺材被破坏时 → 检查 `CoffinRegistry` 是否有人在里面 → 强制"弹出"（等于执行离开流程）
   - **不用 GameMode::Spectator**：Spectator 会改变 Tab 列表显示 + 允许穿墙；改用 set_invisible + movement freeze 更干净

2. **CoffinComponent**（`server/src/coffin/component.rs`）
   ```rust
   #[derive(Component)]
   pub struct CoffinComponent {
       pub entered_at_tick: u64,
       pub coffin_lower: BlockPos,
   }
   ```
   - `register()` 注册为 ECS component
   - 持久化：进棺材时 → `in_coffin=1`，出棺材时 → `in_coffin=0`（写入 `player_lifespan` 表；P2 负责）
   - player spawn/reconnect 时从 persistence 恢复 `CoffinComponent`（若 `in_coffin=1`）

3. **在线限制**
   - 棺材仅对已登录的在线玩家有效（server 必须运行）
   - 若 server 关闭，`in_coffin` 已通过 P2 持久化保存；下次重连时恢复

### 验收抓手

- 测试：`server::coffin::tests::enter_coffin` / `server::coffin::tests::leave_coffin` / `server::coffin::tests::break_coffin_ejects_player`
- 手动：放置棺材 → 右键进入 → 模型消失 → 潜行离开 → 模型恢复

---

## P2 — 寿命流逝速率 ×0.9 ⬜

### 交付物

1. **在线 ticking 速率因子**（修改 `server/src/cultivation/lifespan.rs::lifespan_aging_tick`）
   - `lifespan_tick_rate_multiplier` 签名**不改动**（现有 3 调用方：`lifespan_aging_tick` / `cultivation_detail_emit` / `combat lifecycle`）
   - 在 `lifespan_aging_tick` 中 query `Option<&CoffinComponent>`，先算 `lifespan_tick_rate_multiplier`，再额外乘算 coffin 因子：
     ```rust
     let mut multiplier = lifespan_tick_rate_multiplier(position, zones);
     if coffin_component.is_some() {
         multiplier *= COFFIN_LIFESPAN_FACTOR; // 0.9
     }
     multiplier *= season_aging_modifier(season);
     ```
   - `lifespan_aging_tick` 的 actors query 追加 `Option<&CoffinComponent>`

2. **离线回算速率因子**（修改 `server/src/player/state.rs::load_player_lifespan_from_sqlite`）
   - 从数据库读 `in_coffin` 标志（追加到 SELECT 列）
   - 若 `in_coffin == true`：`LIFESPAN_OFFLINE_MULTIPLIER * 0.9`（即 0.09）
   - 否则保持 `LIFESPAN_OFFLINE_MULTIPLIER = 0.1`

3. **持久化**（修改 `server/src/player/state.rs`）
   - **迁移 SQL**：`ALTER TABLE player_lifespan ADD COLUMN in_coffin INTEGER NOT NULL DEFAULT 0;`
   - `persist_player_lifespan_slice_in_sqlite`：INSERT/UPDATE 追加 `in_coffin` 列
   - `load_player_lifespan_from_sqlite`：SELECT 追加 `in_coffin` 列（`row.get(7)` → `in_coffin != 0`）

4. **常量定义**（`server/src/coffin/constants.rs` 或直接写在 `lifespan.rs`）
   ```rust
   pub const COFFIN_LIFESPAN_FACTOR: f64 = 0.9;
   ```

### 验收抓手

- 测试：`lifespan_delta_years_for_ticks(ticks, 1.0) * 0.9` vs `lifespan_delta_years_for_ticks(ticks, 0.9)` 等值
- 测试：离线 1 小时 → 重连 → 寿命流逝 = `3600 / LIFESPAN_SECONDS_PER_YEAR * 0.1 * 0.9` 年
- 手动：在棺材中挂机 10 分钟 vs 不在棺材中挂机 10 分钟 → 寿命流逝差异可感知

---

## P3 — HUD ⬜

### 交付物

1. **CoffinHudPlanner**（`client/src/main/java/com/bong/client/hud/CoffinHudPlanner.java`）
   - 监听 server→client 推送的 `CoffinState` payload
   - 在屏幕中央下方显示半透明文字："卧棺 · 寿火徐燃"（font color: dark_gray, alpha: 0.6）
   - 附加小 icon：棺材 silhouette（可用现有 chest icon 代替）
   - 寿命速率 indicator：在 `BongHudOrchestrator` 的寿命条旁追加 `[×0.9]` 小字
   - 仅在 `in_coffin == true` 时渲染

2. **Server→Client 推送**（`server/src/network/coffin_state_emit.rs`）
   - 在 `CoffinComponent` 插入/移除时推送 `CoffinStatePayload { in_coffin: bool }`
   - 走现有 `client-payload` 通道

3. **BongHudOrchestrator 注册**
   - `CoffinHudPlanner` 注册到 orchestrator 的 planner 列表
   - 使用 `CoffinStateStore`（类似 `BongHudStateStore` 模式）缓存状态

### 验收抓手

- 测试：`client::hud::tests::coffin_hud_visible_when_in_coffin` / `client::hud::tests::coffin_hud_hidden_when_out`
- 手动：进入棺材 → 屏幕显示"卧棺 · 寿火徐燃"→ 离开 → 提示消失

---

## P4 — 音效 ⬜

### 交付物

1. **音频事件定义**（`server/assets/audio/recipes/coffin.json`）
   - `coffin_enter`：进入棺材——沉重木板合拢声 + 回响衰减
   - `coffin_exit`：离开棺材——木板推开声 + 急促呼吸
   - `coffin_ambient`：在棺材中——低频心跳声（比正常心跳慢 30%）+ 偶尔木板微响
   - `coffin_break`：棺材被破坏——木板碎裂声

2. **音频触发**（`server/src/network/audio_trigger.rs` 追加 coffin 段）
   - 进入棺材时 emit `PlaySoundRecipeRequest { recipe_id: "coffin_enter" }`
   - 离开棺材时 emit `coffin_exit`
   - 在棺材中每 60 tick emit 一次 `coffin_ambient`（ticking 触发）
   - 棺材被破坏时 emit `coffin_break`

3. **音效资源**（`client/src/main/resources/assets/bong/sounds/`）
   - 用 vanilla Minecraft chest 音效作为临时音源（`block.chest.close` / `block.chest.open` / `block.wood.break`）
   - ambient 心跳用 `entity.player.hurt` 降调 + 减速处理
   - 正式音效资源后续 `plan-audio-world-v1` 统一提供

### 验收抓手

- 手动：进入棺材 → 听到木板合拢声 → 在棺材中听到心跳声 → 退出 → 听到木板推开声
- 测试：`server::audio::tests::coffin_trigger_events`

---

## P5 — 全链路 E2E ⬜

### 交付物

1. **E2E 测试脚本**（`scripts/e2e/coffin-lifecycle.sh`）
   - 步骤 1：合成棺材（give 物品 or 指令合成）
   - 步骤 2：放置棺材
   - 步骤 3：右键进入棺材 → 验证 HUD 出现 + 进入音效
   - 步骤 4：在棺材中等待 60 tick → 验证寿命流逝速率 = tick * 0.9 / LIFESPAN_TICKS_PER_YEAR
   - 步骤 5：潜行离开棺材 → 验证 HUD 消失 + 退出音效
   - 步骤 6：再次进入棺材 → 断开客户端（模拟下线）→ 等待 10 秒 → 重连 → 验证离线寿命流逝 = 10 * 0.1 * 0.9 / LIFESPAN_SECONDS_PER_YEAR
   - 步骤 7：在棺材中时破坏棺材 → 验证玩家被弹出

2. **寿命流逝对比测试**
   - Run A：不在棺材中，在线 120 tick
   - Run B：在棺材中，在线 120 tick
   - 预期：Run B 寿命流逝 = Run A × 0.9（误差 < 0.01 年）

### 验收抓手

- E2E 脚本全部通过
- 寿命流逝对比：`run_A_years * 0.9 ≈ run_B_years`

---

## Finish Evidence

- **落地清单**：
  - server：新增 `coffin` 子系统、`mundane_coffin` 物品/手搓 recipe、双格 chest 视觉占位、放置/进入/离开/破坏 lifecycle、`CoffinComponent`、进入隐身与位置 pin、破坏弹出与返还、4 条 coffin 音效 recipe、在线寿命 ×0.9、离线寿命 `0.1 * 0.9`、`player_lifespan.in_coffin` v23 迁移与 legacy 缺表保护。
  - client：新增 `coffin_place` / `coffin_enter` / `coffin_leave` 请求、保留 spawn tutorial `coffin_open`、新增 `coffin_state` handler/store、`CoffinHudPlanner`、HUD layer/preset/immersion 接入。
  - agent/schema：新增 coffin C2S schema、`coffin_state` S2C schema、registry/generated artifacts 与 schema tests。
  - E2E harness：新增 `scripts/e2e/coffin-lifecycle.sh`，串起 server lifecycle/persistence、schema wire contract、client protocol/state/HUD 的可重复验证。
- **关键 commit**：
  - `16afa4604`：`feat(coffin-v1): 实装凡物棺材全链路`
  - `f4c9a3643`：`修复 coffin-v1 验证收尾问题`
  - `4319497f8`：`补充 coffin-v1 生命周期验证脚本`
  - `fd4207eac`：合入 `origin/main`，保持 plan 分支基线同步。
- **验证**：
  - `cd server && cargo fmt --check && CARGO_BUILD_JOBS=1 cargo clippy --all-targets -- -D warnings && CARGO_BUILD_JOBS=1 cargo test -- --test-threads=1`（4287 passed）
  - `cd agent && npm run generate:check -w @bong/schema`（357 generated artifacts fresh）
  - `cd agent && npm run build`
  - `cd agent && npm test -w @bong/schema`（19 files / 368 tests passed）
  - `cd agent && npm test -w @bong/tiandao`（51 files / 352 tests passed）
  - `cd client && JAVA_HOME=/usr/lib/jvm/java-17-openjdk-amd64 PATH=/usr/lib/jvm/java-17-openjdk-amd64/bin:$PATH ./gradlew test build`（BUILD SUCCESSFUL）
  - `scripts/e2e/coffin-lifecycle.sh`（server/schema/client coffin lifecycle harness ok，Java 17）
  - `git diff --check`
- **遗留 / 后续**：灵材棺材（×0.7 / ×0.5 / ×0.3，按灵材等级递增）/ 棺材符文（附加灵气微回复或梦境训练）/ 棺材视觉定制（七流派不同棺材外观）/ 双人棺材（道侣共享）。本 plan 只落凡物棺材，未新增自定义方块模型；当前视觉用双 chest 占位。
