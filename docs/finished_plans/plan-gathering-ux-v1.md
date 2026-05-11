# Bong · plan-gathering-ux-v1 · 采集体验 + 工具补全

采集系统体验层 + 缺失工具补全。当前问题：① 斧头/镐没有实装（只有锄头）② 锄头贴图不对 ③ 多个植物 icon 是重复贴图 ④ 采集 = 右键瞬间获得没有体感。本 plan 做四件事：① 补全斧头/镐物品 + 贴图 ② 修复锄头贴图 + 重复植物 icon ③ 给采集加进度环 + 品质判定 ④ 采集全流程动画/粒子/音效。

**世界观锚点**：`worldview.md §九` 资源匮乏 → 采集不是随手捡，是辛苦劳动 · `§七` 散修生态（拾荒/采药是日常）→ 采集是核心 gameplay loop · `§十三` 不同 zone 产出不同 → 工具效率受 zone 影响

**前置依赖**：
- `plan-botany-v1` ✅ → PlantRegistry / HarvestEvent / 植物系统
- `plan-botany-v2` ✅ → 17 种 v2 植物 + 季节依赖
- `plan-lingtian-v1` ✅ → 灵田动作系统（锄头 till/植）
- `plan-craft-v1` ✅ → CraftRegistry（工具制作配方）
- `plan-weapon-v1` ✅ → 材质系统复用（工具材质 = 武器材质子集）
- `plan-inventory-v2` ✅ → 物品栏
- `plan-vfx-v1` ✅ → 粒子基类
- `plan-audio-v1` ✅ → SoundRecipePlayer
- `plan-HUD-v1` ✅ → HUD 层

**反向被依赖**：
- `plan-botany-visual-v1` ✅ → 植物 icon 修复后 botany-visual 可引用正确贴图

---

## 接入面 Checklist

- **进料**：`botany::PlantRegistry` / `botany::HarvestEvent` / `lingtian::LingtianActionEvent` / `craft::CraftRegistry` / `weapon::MaterialTier`（复用材质分级）/ `inventory::PlayerInventory` / `cultivation::Realm`（境界影响采集速度）
- **出料**：server `gathering::*` 模块（GatheringTool enum / GatheringSession / 进度 tick / 品质 roll）+ 3 种工具物品注册（斧/镐/锄 × 3 材质）+ client `GatheringProgressHud`（进度环 + 品质指示）+ 修复后的植物 icon + 工具 icon + 采集动画/粒子/音效
- **跨仓库契约**：server `GatheringSessionS2c { progress, quality_hint }` → client `GatheringProgressHud`

---

## §0 设计轴心

- [x] **采集 ≠ 瞬间获得**：长按右键 → 进度环填充（1-3s 按材质/工具/境界决定）→ 完成后获得
- [x] **工具提升效率**：无工具可采但慢 ×3 / 有对应工具正常速度 / 高材质工具 +速度 +品质
- [x] **品质 roll**：采集完成时 roll 品质（普通/优良/极品），工具材质+境界影响概率
- [x] **修复贴图优先**：P0 先修锄头贴图 + 重复植物 icon，再做新功能
- [x] **HUD auto-hide**：进度环仅在采集时显示，完成后 1s fade out

---

## 阶段总览

| 阶段 | 内容 | 状态 |
|----|------|----|
| P0 | 修复锄头贴图 + 修复重复植物 icon + 补全斧头/镐物品注册 + 工具 icon 生成 | ✅ |
| P1 | 采集进度系统 server（GatheringSession + 进度 tick + 工具效率修正） | ✅ |
| P2 | 品质 roll 系统 + client 进度环 HUD + 品质结果 UI | ✅ |
| P3 | 采集动画 + 粒子 + 音效全流程 | ✅ |
| P4 | 工具 craft 配方 + 全工具×全植物×全品质饱和化测试 | ✅ |

---

## P0 — 贴图修复 + 工具补全 ✅

### 交付物

1. **锄头贴图修复**
   - 检查 `client/src/main/java/com/bong/client/lingtian/HoeVanillaIconMap.java` → 确认映射正确
   - 当前锄头 icon 指向错误贴图 → 修正为正确的工具贴图路径
   - 如果没有专用锄头贴图 → 用 gen.py item 档生成（石质锄头/铁锄头/骨锄头）

2. **重复植物 icon 修复**
   - 扫描 `client/src/main/resources/assets/bong-client/textures/gui/botany/*.png`
   - 找出内容完全相同的重复文件（md5 比对）
   - 对重复的植物重新生成独立 icon（gen.py item 档，描述用植物名+特征）
   - 确保 39 张植物 icon 各自不同

3. **斧头/镐物品注册**（`server/src/gathering/tools.rs`）
   - `GatheringTool` enum：`Axe` / `Pickaxe` / `Hoe`（锄头已有，补前两者）
   - 3 种材质 tier（复用 weapon-v1 的 `MaterialTier`）：骨制 / 铁制 / 铜制
   - 物品 ID：`bong:axe_bone` / `bong:axe_iron` / `bong:axe_copper` / `bong:pickaxe_bone` / `bong:pickaxe_iron` / `bong:pickaxe_copper`
   - 每种工具属性：采集速度修正 / 耐久 / 适用目标（斧=木/竹类 / 镐=矿石/石类 / 锄=草药/灵田）

4. **工具 icon 生成**
   - `scripts/images/gen.py` item 档：6 张新 icon（3 斧 + 3 镐）
   - 输出到 `client/src/main/resources/assets/bong-client/textures/gui/items/tools/`
   - 风格：石质粗糙 / 铁质暗灰 / 铜质温润

### 验收抓手
- 植物 icon md5 去重后 39 张各不同
- 锄头 icon 正确显示（非占位/非错误引用）
- 6 个新工具物品注册成功 + icon 显示

---

## P1 — 采集进度系统 ✅

### 交付物

1. **`GatheringSession`**（`server/src/gathering/session.rs`）
   - 玩家对可采集方块/实体长按右键 → 创建 `GatheringSession` component
   - 进度 tick：每 tick +1 → 到达 `gather_time` → 完成
   - `gather_time` 计算：
     - 基础时间：植物 40 tick(2s) / 矿石 60 tick(3s) / 木材 50 tick(2.5s)
     - 工具修正：无工具 ×3 / 骨制 ×1.2 / 铁制 ×1.0 / 铜制 ×0.8
     - 境界修正：每境 -5%（化虚 = ×0.75）
   - 打断条件：移动 / 被攻击 / 松开右键 → 进度归零
   - 完成 → emit `GatheringCompleteEvent { target, quality, tool_used }`

2. **`GatheringSessionS2c` packet**
   - `{ progress_ticks, total_ticks, target_name, quality_hint }`
   - 每 10 tick 同步一次（不 per-tick 避免网络洪流）
   - 打断时发 `{ progress: 0 }`

3. **可采集物标记**
   - 植物：`botany::Harvestable` component 已有 → 复用
   - 矿石/木材：新增 `gathering::Gatherable { gather_type, base_time, loot_table }` component
   - 长按右键时 crosshair 变化暗示"可以采"（client 侧按 entity/block 类型判断）

### 验收抓手
- 测试：`server::gathering::tests::session_progress_ticks` / `server::gathering::tests::tool_speeds_up` / `server::gathering::tests::realm_reduces_time` / `server::gathering::tests::interrupt_on_move`
- 手动：对植物长按 → 2s 后获得 / 无工具 → 6s / 有铁锄 → 2s / 移动 → 中断

---

## P2 — 品质 roll + client 进度环 ✅

### 交付物

1. **品质 roll 系统**（`server/src/gathering/quality.rs`）
   - 采集完成时 roll：`Normal(70%)` / `Fine(25%)` / `Perfect(5%)`
   - 工具材质加成：铜制 → Fine +10% Perfect +3% / 铁制 → Fine +5% Perfect +1%
   - 境界加成：每境 Fine +2% Perfect +0.5%
   - 品质影响产物属性：Fine = 数量 ×1.5 / Perfect = 数量 ×2 + 稀有副产物概率

2. **`GatheringProgressHud`**（`client/src/main/java/com/bong/client/hud/GatheringProgressHud.java`）
   - crosshair 周围的圆弧进度环（半径 12px，2px 宽）
   - 颜色：进度中 = 白色渐填 → 即将完成 = 绿色
   - 完成瞬间：环变金色 flash 0.2s → fade out 0.5s
   - **auto-hide**：不采集时不显示 / 完成后 1s fade out

3. **品质结果 UI**
   - 完成时 crosshair 上方小文字 flash：
     - Normal：无特殊提示（不打扰）
     - Fine："优良" 绿色字 0.5s fade
     - Perfect："极品!" 金色字 1s + 微粒子 flash
   - 获得物品同时 toast 显示物品名+数量（复用 hud-polish-v1 toast 系统）

### 验收抓手
- 测试：`server::gathering::tests::quality_roll_probabilities` / `server::gathering::tests::copper_tool_fine_bonus` / `client::gathering::tests::progress_ring_fade_after_complete`
- 手动：采集植物 → 进度环走满 → "优良" 绿字闪 → 获得 1.5x 数量

---

## P3 — 动画 + 粒子 + 音效 ✅

### 交付物

1. **采集动画**（需 player-animation-implementation-v1 配合——如果未就绪则 skip 动画仅做粒子/音效）
   - `gather_herb.json`：弯腰伸手采（rightArm reach down + body lean，loop per tick）
   - `gather_mine.json`：镐挥击循环（rightArm pickaxe swing，loop per 10 tick）
   - `gather_chop.json`：斧劈循环（rightArm axe swing overhead，loop per 12 tick）
   - 如果 animation 系统未就绪 → 先用 arm swing vanilla 动画占位

2. **采集粒子**
   - 采药：植物碎叶粒子（`BongSpriteParticle` `cloud256_dust` tint 绿色 × 2 per 10 tick 从植物位置向上飘）
   - 挖矿：石屑粒子（tint 灰色 × 3 per 8 tick 从方块面弹出）
   - 砍树：木屑粒子（tint 棕色 × 2 per 12 tick 向侧方飞出）
   - 完成时爆发：大量碎片粒子 × 8 从目标位置向外飞散 + 掉落物品 3D 弹出

3. **采集音效**
   - `gather_herb_tick.json`：`minecraft:block.grass.break`(pitch 1.3, volume 0.15)（每 20 tick 一次轻响）
   - `gather_mine_tick.json`：`minecraft:block.stone.break`(pitch 0.8, volume 0.2)（每 10 tick）
   - `gather_chop_tick.json`：`minecraft:block.wood.break`(pitch 1.0, volume 0.2)（每 12 tick）
   - `gather_complete.json`：`minecraft:entity.item.pickup`(pitch 1.2, volume 0.3)（完成获得音）
   - `gather_perfect.json`：`minecraft:entity.player.levelup`(pitch 2.0, volume 0.15)（极品品质额外音）

4. **稀有采集 cinematic**（Perfect 品质触发）
   - 极品品质采集时：time scale ×0.5 持续 0.5s（微慢动作）+ camera 微推近目标 + 金色粒子爆发
   - 仅自己可见（其他玩家看到正常速度）

### 验收抓手
- 测试：`client::gathering::tests::herb_particles_per_10_tick` / `client::gathering::tests::perfect_slowmo_self_only`
- 手动：采药 → 绿叶飘 + 草声 → 完成 → 碎片爆发 + 获得音 → 极品 → 微慢动作 + 金色

---

## P4 — 工具 craft + 饱和化测试 ✅

### 交付物

1. **工具 craft 配方注册**
   - 6 个工具 → CraftRegistry（配合 plan-craft-ux-v1 的 UI 显示）：
     - `axe_bone`：骨币 ×3 + 木材 ×2
     - `axe_iron`：铁矿 ×3 + 木材 ×1
     - `axe_copper`：铜矿 ×3 + 木材 ×1
     - `pickaxe_bone`：骨币 ×3 + 木材 ×2
     - `pickaxe_iron`：铁矿 ×4 + 木材 ×1
     - `pickaxe_copper`：铜矿 ×4 + 木材 ×1
   - category: `ToolCraft`

2. **工具耐久**
   - 每次采集成功 → 工具耐久 -1
   - 骨制 60 / 铁制 120 / 铜制 90（铜轻便但不耐用）
   - 耐久 0 → 工具破碎（同 armor 破碎逻辑——碎片粒子 + 音效）

3. **饱和化测试矩阵**
   - 3 工具 × 3 材质 × 3 采集类型（草药/矿石/木材）× 3 品质 × 有无工具 = 81+ 组合
   - 全工具 craft → 采集 → 耐久消耗 → 破碎 链路
   - 贴图验证：39 植物 icon 各不同 + 6 工具 icon + 3 锄头 icon 正确

### 验收抓手
- 全 81 组合自动化测试
- 贴图 md5 去重验证脚本

---

## Finish Evidence

### 落地清单

- **P0 工具 / icon / 贴图核验**：`server/src/gathering/tools.rs` 注册 Axe / Pickaxe / Hoe 三类采集工具与 Bone / Iron / Copper 三材质；`server/assets/items/tools.toml` 增加 6 个斧 / 镐凡器 item；`client/src/main/resources/assets/bong-client/textures/gui/items/tools/` 新增 6 张工具 icon；`client/src/main/java/com/bong/client/inventory/ItemIconRegistry.java` 显式映射工具 icon。锄头沿用现有 `HoeVanillaIconMap` 三档 vanilla 映射；botany 贴图用 md5 校验，`botany/` 下 100 张 PNG 无重复 hash。
- **P1 server 采集进度**：新增 `server/src/gathering/session.rs` / `tools.rs` / `quality.rs` / `feedback.rs` / `mod.rs`，覆盖 `GatheringSession`、`GatheringProgressFrame`、`GatheringCompleteEvent`、工具速度修正、境界时间修正、移动 / 受击打断、品质 roll、耐久扣减和统一反馈。`server/src/botany/mod.rs`、`server/src/spiritwood/mod.rs`、`server/src/mineral/break_handler.rs` 已把既有采药 / 砍灵木 / 挖矿流程桥接到统一 `gathering_session` 进度帧。
- **P2 schema + client HUD**：`server/src/schema/server_data.rs`、`agent/packages/schema/src/server-data.ts` 与 generated schema 增加 `gathering_session`；client 新增 `GatheringSessionStore` / `GatheringSessionViewModel` / `GatheringSessionHandler` / `GatheringProgressHud`，并接入 `ServerDataRouter` 与 `BongHudOrchestrator`。HUD 支持 crosshair 进度环、目标名、优良 / 极品提示、完成 / 中断后 auto-hide。
- **P3 动画 / 粒子 / 音效**：`server/assets/audio/recipes/gather_{herb,mine,chop}_tick.json`、`gather_complete.json`、`gather_perfect.json` 已注册；`VfxBootstrap` 与 `BotanyHarvestBurstPlayer` 注册 herb / mine / chop / complete / perfect 五类采集 VFX；`emit_gathering_feedback` 按目标触发 `harvest_crouch` / `npc_mine` / `npc_chop_tree`，完成时触发 `loot_bend` / `release_burst` 动画。
- **P4 craft / 耐久 / 覆盖测试**：`server/src/craft/mod.rs` 注册 6 个 ToolCraft 配方；`damage_equipped_gathering_tool` 复用 inventory durability update 与 `InventoryDurabilityChangedEvent`；测试覆盖工具矩阵、速度、品质、反馈、craft、client HUD、server-data router、icon registry、VFX registry。

### 关键 commits

- `cf54c97a1` 2026-05-11 `feat(gathering): 补采集工具和服务端进度流程`
- `578fec44e` 2026-05-11 `feat(gathering): 接入客户端采集 HUD 和协议`
- `7903aa2ea` 2026-05-11 `fix(gathering): 接通采集进度反馈`
- `7b1d9ac08` 2026-05-11 `fix(gathering): 收紧采集协议与完成事件`
- `ba2703c8c` 2026-05-11 `fix(gathering): 收紧矿物采集边界与 Rust 协议枚举`
- `bc3fdbb5a` 2026-05-11 `fix(gathering): 补齐采集 review 边界护栏`
- `654680039` 2026-05-11 `fix(gathering): 补齐协议与配方测试护栏`
- `529945fad` 2026-05-11 `fix(gathering): 对齐采集协议输出与原版镐识别`

### 测试结果

- `cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`：4268 passed
- `cd server && cargo fmt --check`：pass
- `cd server && CARGO_BUILD_JOBS=1 cargo clippy --all-targets -- -D warnings`：pass
- `cd server && CARGO_BUILD_JOBS=1 cargo check --tests`：pass
- `cd server && CARGO_BUILD_JOBS=1 cargo test equipped_pickaxe_tier`：5 passed
- `cd server && CARGO_BUILD_JOBS=1 cargo test mineral_gatherable`：1 passed
- `cd server && CARGO_BUILD_JOBS=1 cargo test startup_spawns_index_entries_and_skips_exhausted_positions`：1 passed
- `cd server && CARGO_BUILD_JOBS=1 cargo test gathering_wire_enums_match_shared_schema_values`：1 passed
- `cd server && CARGO_BUILD_JOBS=1 cargo test loads_default_audio_recipes`：1 passed
- `cd server && CARGO_BUILD_JOBS=1 cargo test register_gathering_tool_recipes`：2 passed
- `cd server && CARGO_BUILD_JOBS=1 cargo test gathering_session_rejects_invalid_enum_values`：1 passed
- `cd server && CARGO_BUILD_JOBS=1 cargo test hud_payload_wire_type_matches_label`：1 passed
- `cd server && CARGO_BUILD_JOBS=1 cargo test gathering_progress_frame`：2 passed
- `cd server && CARGO_BUILD_JOBS=1 cargo test vanilla_pickaxe`：2 passed
- `cd server && CARGO_BUILD_JOBS=1 cargo test survival_start_opens_gathering_progress_without_drop_or_cleanup`：1 passed
- `cd server && cargo test mineral::break_handler::tests`：12 passed
- `cd server && cargo test spiritwood::tests`：8 passed
- `cd server && cargo test craft::tests::register_gathering_tool_recipes_adds_six_tool_entries`：1 passed
- `cd client && JAVA_HOME="$HOME/.sdkman/candidates/java/17.0.18-amzn" PATH="$HOME/.sdkman/candidates/java/17.0.18-amzn/bin:$PATH" ./gradlew test build`：BUILD SUCCESSFUL
- `cd client && JAVA_HOME="$HOME/.sdkman/candidates/java/17.0.18-amzn" PATH="$HOME/.sdkman/candidates/java/17.0.18-amzn/bin:$PATH" ./gradlew test --tests com.bong.client.network.ServerDataRouterTest --tests com.bong.client.inventory.ItemIconRegistryTest --tests com.bong.client.hud.GatheringProgressHudTest`：BUILD SUCCESSFUL
- `cd agent && npm run build`
- `cd agent && npm test -w @bong/tiandao`：348 passed
- `cd agent && npm test -w @bong/schema`：372 passed
- `cd agent && npm run generate -w @bong/schema`：generated schemas refreshed
- `git diff --check`：clean

### 跨仓库核验

- **server**：`GatheringToolSpec` / `GatheringSession` / `GatheringProgressFrame` / `GatheringCompleteEvent` / `ServerDataPayloadV1::GatheringSession` / `register_gathering_tool_recipes`
- **agent/schema**：`ServerDataGatheringSessionV1` / `server-data-gathering-session-v1.json` / `SCHEMA_REGISTRY.serverDataGatheringSessionV1`
- **client**：`GatheringSessionHandler` / `GatheringProgressHud` / `GatheringSessionViewModel` / `ItemIconRegistry` / `BotanyHarvestBurstPlayer.GATHER_PERFECT`

### 遗留 / 后续

- 高阶采集工具仍归 forge 后续 plan；本 plan 只落凡器斧 / 镐与既有锄头兼容。
- 采集点重生、NPC 采集竞争、稀有采集的真实 camera slowmo / camera push 需要后续专门 camera / world interaction plan；本次已落 self-visible HUD 金色反馈 + perfect VFX / SFX / player animation。
