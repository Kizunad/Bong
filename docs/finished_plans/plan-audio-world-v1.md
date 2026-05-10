# Plan: Audio World v1（世界音效与氛围）

> 音效框架已完整（38 recipe、SoundRecipePlayer、AudioTriggerS2c、JSON hot-reload），但**全部使用 vanilla MC 音效组合**，且**环境音/氛围音/音乐状态机完全缺失**——玩家进入末法残土听到的和 vanilla 生存模式一模一样。本 plan 建立区域感知的氛围音系统 + 战斗音乐状态机 + 修炼/TSY 专属音景。

---

## 接入面 Checklist（防孤岛）

- **进料**：`audio::SoundRecipePlayer` ✅ / `audio::AudioCategory` ✅ / `audio::AudioAttenuation` ✅ / `world::zone::ZoneRegistry` ✅ / `combat::CombatState` ✅ / `cultivation::Cultivation` ✅ / `tsy::TsyPresence` ✅
- **出料**：ambient recipe JSON → `server/assets/audio/recipes/` / ambient system → `server/src/audio/ambient.rs` / music state machine → `client/src/main/java/com/bong/client/audio/MusicStateMachine.java`
- **共享类型/event**：新增 `AmbientZoneS2c { zone_name, ambient_recipe_id }` packet / 复用 `AudioTriggerS2c`
- **跨仓库契约**：server 玩家切换 zone 时 emit `AmbientZoneS2c` → client `MusicStateMachine` 切换 ambient loop
- **worldview 锚点**：§十三 地理（6 区域各自风貌）/ §十六 坍缩渊（负压嗡鸣）/ §十七 末法节律（炎汐/凝汐音差）

---

## 阶段总览

| 阶段 | 目标 | 状态 |
|------|------|------|
| P0 | 区域 ambient loop + 昼夜切换 | ✅ |
| P1 | 战斗音乐状态机 + 修炼冥想音 | ✅ |
| P2 | TSY 负压音景 + 节律音差 + 天劫氛围 | ✅ |

---

## P0 — 区域 ambient + 昼夜切换 ✅

### 交付物

1. **区域 ambient 系统**（`server/src/audio/ambient.rs`）
   - `ambient_zone_change_system`：检测玩家 `CurrentZone` 变化 → emit `AmbientZoneS2c`
   - 每个 zone 配置 `ambient_recipe_id`（在 `zones.json` / `zones.tsy.json` 追加字段）

2. **6 区域 ambient recipe**
   - `ambient_spawn_plain.json`：`minecraft:ambient.cave`(volume 0.1) + `minecraft:weather.rain`(pitch 0.3, volume 0.05)（寂静荒原风声）
   - `ambient_qingyun_peaks.json`：`minecraft:ambient.basalt_deltas.mood`(pitch 1.5, volume 0.1)（高山风啸）
   - `ambient_spring_marsh.json`：`minecraft:block.pointed_dripstone.drip_water`(loop, pitch 0.8)（灵泉滴水）
   - `ambient_rift_valley.json`：`minecraft:ambient.warped_forest.mood`(volume 0.15)（裂谷低鸣）
   - `ambient_north_wastes.json`：`minecraft:weather.rain`(pitch 0.1, volume 0.08) + `minecraft:entity.phantom.flap`(pitch 0.2, volume 0.03)（北荒寒风）
   - `ambient_wilderness.json`：`minecraft:ambient.cave`(volume 0.05)（荒野静谧）

3. **昼夜音量切换**
   - client `MusicStateMachine`：检测 world time → 夜间(13000-23000) ambient volume ×1.5 + 追加 `minecraft:entity.bat.ambient`(pitch 0.3, volume 0.02)
   - 日夜切换 crossfade 3s（不突兀）

4. **client MusicStateMachine**（`client/src/main/java/com/bong/client/audio/MusicStateMachine.java`）
   - 状态：`AMBIENT` / `COMBAT` / `CULTIVATION` / `TSY` / `TRIBULATION`
   - 切换优先级：TRIBULATION > COMBAT > TSY > CULTIVATION > AMBIENT
   - 每种状态绑定对应 recipe loop + 进入/退出 crossfade

### 验收抓手

- 测试：`server::audio::tests::zone_change_emits_ambient` / `client::audio::tests::music_state_transitions`
- 手动：在 spawn_plain → 听到风声 → 走到灵泉湿地 → 听到滴水 → 天黑 → 音量增大 + 蝙蝠声

---

## P1 — 战斗音乐 + 修炼冥想音 ✅

### 交付物

1. **战斗音乐状态切换**
   - 进入战斗（`CombatState::InCombat`）→ `MusicStateMachine` 切换到 `COMBAT`
   - `combat_music.json`：`minecraft:music.dragon`(pitch 0.6, volume 0.15) + `minecraft:entity.warden.heartbeat`(pitch 0.8, volume 0.1)
   - 脱战由 `CombatState::in_combat_until_tick` 结束窗口控制，回到 AMBIENT 时按 3 秒 / 60 ticks crossfade

2. **修炼冥想音**
   - 实际引气修炼窗口（`CultivationSessionPracticeAccumulator::is_recently_practicing`）→ `MusicStateMachine` 切换到 `CULTIVATION`
   - `cultivation_meditate.json`：`minecraft:block.amethyst_block.chime`(pitch 0.5, volume 0.08, loop 4s interval) + `minecraft:ambient.underwater.loop`(pitch 0.3, volume 0.03)（深沉冥想氛围）
   - 经脉打通瞬间：`minecraft:block.amethyst_cluster.break`(pitch 1.5, volume 0.3)（清脆灵感音）

3. **低 HP 心跳**
   - `heartbeat_low_hp.json` 已存在 → 确认接线：HP < 20% 时 emit `AudioTriggerS2c`
   - 真元耗尽警告：`qi_depleted_warning.json` 已存在 → 确认 server 真元 < 10% 时 emit

### 验收抓手

- 测试：`server::audio::tests::combat_state_triggers_music` / `server::audio::tests::meditation_triggers_ambient`
- 手动：遇敌 → 战斗鼓点 → 脱战 → 恢复环境音 → 打坐 → 水晶钟声 → 打通经脉 → 清脆音

---

## P2 — TSY 音景 + 节律 + 天劫 ✅

### 交付物

1. **TSY 负压音景**
   - 进入 TSY 维度 → `MusicStateMachine` 切换到 `TSY`
   - `ambient_tsy.json`：`minecraft:ambient.nether_wastes.mood`(pitch 0.3, volume 0.12) + `minecraft:entity.enderman.stare`(pitch 0.2, volume 0.04, loop 8s)（压迫感嗡鸣）
   - 深层 tier：追加 `minecraft:block.respawn_anchor.deplete`(pitch 0.4, volume 0.06)（更重的负压嗡鸣）

2. **节律音差**（配合 plan-jiezeq-v1 ✅）
   - 炎汐期：ambient `pitch_shift=+0.10` + 追加 `minecraft:block.fire.ambient`(volume 0.02)（温暖感）
   - 凝汐期：ambient `pitch_shift=-0.10` + 追加 `minecraft:block.powder_snow.step`(volume 0.02)（寒冷感）
   - 不给玩家显式提示（worldview §K 红线"不显式提示汐转期"），仅通过音差暗示

3. **天劫氛围**
   - 渡劫时 → `MusicStateMachine` 切换到 `TRIBULATION`（最高优先级）
   - `tribulation_atmosphere.json`：`minecraft:entity.lightning_bolt.thunder`(pitch 0.4, volume 0.2, delay random 2-5s) + `minecraft:music.dragon`(pitch 0.3, volume 0.2)
   - 已有 `tribulation_thunder_distant.json` 和 `tribulation_wave_impact.json` → 确认按 wave 阶段切换

### 验收抓手

- 测试：`server::audio::tests::tsy_dimension_triggers_ambient` / `client::audio::tests::tribulation_overrides_combat`
- E2E：进入 TSY → 压迫嗡鸣 → 回到主世界 → 恢复风声 → 触发渡劫 → 雷鸣覆盖一切

---

## 前置依赖

| 依赖 plan | 状态 | 用到什么 |
|-----------|------|---------|
| plan-audio-v1 | ✅ finished | SoundRecipePlayer / AudioTriggerS2c / recipe JSON schema / 38 existing recipes |
| plan-combat-no_ui | ✅ finished | CombatState |
| plan-cultivation-v1 | ✅ finished | CultivationSessionPracticeAccumulator / MeridianSystem |
| plan-tsy-dimension-v1 | ✅ finished | DimensionKind::Tsy / TsyPresence |
| plan-jiezeq-v1 | ✅ finished | SeasonState / 炎汐/凝汐 |
| plan-tribulation-v1 | ✅ finished | TribulationAnnounce / wave flow |
| plan-zone-weather-v1 | ✅ finished | zone 天气 profile |

**全部依赖已 finished，无阻塞。**

## Finish Evidence

### 落地清单

- P0 区域 ambient + 昼夜切换：
  - `server/src/audio/ambient.rs` 新增 `ambient_zone_change_system`，根据玩家 zone/dimension/combat/meditation/TSY/tribulation 状态发送 `bong:audio/ambient_zone` custom payload。
  - `server/zones.json`、`server/zones.tsy.json` 增加 `ambient_recipe_id`；`AmbientZoneRecipes` 从 zone 配置加载 recipe，并保留旧 zone 名 fallback。
  - `server/assets/audio/recipes/ambient_*.json` 覆盖 spawn、qingyun、lingquan/spring marsh、rift/blood valley、north wastes、wilderness、TSY。
  - `client/src/main/java/com/bong/client/audio/MusicStateMachine.java` 负责 ambient loop、3 秒 fade、night volume multiplier 与切换去重。

- P1 战斗音乐 + 修炼冥想音：
  - `server/src/audio/ambient.rs` 按 `TRIBULATION > COMBAT > TSY > CULTIVATION > AMBIENT` 解析音乐状态，`CombatState` 进入 `combat_music`。
  - 当前代码没有 `Cultivation::is_meditating`，改用 `qi_regen_and_zone_drain_tick` 记录的真实引气窗口：`CultivationSessionPracticeAccumulator::is_recently_practicing` 为真时进入 `cultivation_meditate`。
  - `server/src/cultivation/meridian_open.rs` 新增 `MeridianOpenedEvent`，`server/src/network/audio_trigger.rs` 将经脉打通瞬间映射到 `meridian_open_chime`。
  - 低 HP 心跳阈值从 30% 收敛到 plan 要求的 20%，新 flag 为 `hp_below_20`，client 保留 `hp_below_30` 兼容。

- P2 TSY + 节律 + 天劫：
  - TSY 维度 / `TsyPresence` / TSY zone depth 进入 `ambient_tsy`，deep tier 追加 `minecraft:block.respawn_anchor.deplete`。
  - `WorldSeasonState` / `query_season` 驱动炎汐 `pitch_shift=+0.10` + fire layer、凝汐 `pitch_shift=-0.10` + powder snow layer，不发显式提示。
  - `TribulationState` 进入最高优先级 `tribulation_atmosphere`，现有 wave 音效 trigger 保持接线。

- 跨仓库契约：
  - `server/src/schema/audio.rs` 增加 `AmbientZoneS2c`，并校验 version、music_state、season 与数值范围。
  - `agent/packages/schema/src/audio-event.ts` 增加 `AmbientZoneEventV1` / `AudioSeasonV1`，generated schema 已刷新。
  - `client/src/main/java/com/bong/client/network/AmbientZoneHandler.java` 强制解析并校验 `season`、`ambient_recipe_id == recipe.id`、fade/volume/pitch 范围后交给 `MusicStateMachine`。

### 关键 commits

- `cf70c7eaf` 2026-05-10 `plan-audio-world-v1: 接入服务端环境音状态`
- `7faa2d0f7` 2026-05-10 `plan-audio-world-v1: 接入客户端音乐状态机`
- `daccb1d7d` 2026-05-10 `plan-audio-world-v1: 扩展音频事件契约`
- `d5e62ba88` 2026-05-10 `fix(audio): 收紧环境音乐状态契约`
- `43a62b480` 2026-05-11 `fix(audio): 收紧 ambient zone 边界`
- `2381e154e` 2026-05-11 `fix(audio): 补齐 ambient review 边界`

### 测试结果

- `git diff --check` ✅
- `server/zones.tsy.json`: `python3 -m json.tool server/zones.tsy.json >/dev/null` ✅
- `server/`: `cargo fmt --check` ✅
- `server/`: `CARGO_BUILD_JOBS=1 RUSTFLAGS="-C debuginfo=0" cargo clippy --all-targets -- -D warnings` ✅
- `server/`: `CARGO_BUILD_JOBS=1 RUSTFLAGS="-C debuginfo=0" cargo test audio::` ✅ 17 passed
- `server/`: `CARGO_BUILD_JOBS=1 RUSTFLAGS="-C debuginfo=0" cargo test practice_accumulator` ✅ 2 passed
- `server/`: `CARGO_BUILD_JOBS=1 RUSTFLAGS="-C debuginfo=0 -C link-arg=-Wl,--no-keep-memory" cargo test` ✅ 3830 passed
- `client/`: `JAVA_HOME=/usr/lib/jvm/java-17-openjdk-amd64 PATH=/usr/lib/jvm/java-17-openjdk-amd64/bin:$PATH ./gradlew test --tests "com.bong.client.network.AmbientZoneHandlerTest"` ✅ BUILD SUCCESSFUL
- `client/`: `JAVA_HOME=/usr/lib/jvm/java-17-openjdk-amd64 PATH=/usr/lib/jvm/java-17-openjdk-amd64/bin:$PATH ./gradlew test --tests "com.bong.client.audio.MusicStateMachineTest" --tests "com.bong.client.audio.SoundRecipePlayerTest" --tests "com.bong.client.network.AmbientZoneHandlerTest"` ✅ BUILD SUCCESSFUL
- `client/`: `JAVA_HOME=/usr/lib/jvm/java-17-openjdk-amd64 PATH=/usr/lib/jvm/java-17-openjdk-amd64/bin:$PATH ./gradlew test build` ✅ BUILD SUCCESSFUL
- `agent/`: `npm run build` ✅
- `agent/`: `npm test -w @bong/schema` ✅ 353 passed
- `agent/packages/tiandao`: `npm test` ✅ 325 passed

### 验证备注

- 默认 debug `CARGO_BUILD_JOBS=1 cargo test` 在本机链接测试二进制时被 SIGKILL；同一代码用 `RUSTFLAGS="-C debuginfo=0 -C link-arg=-Wl,--no-keep-memory"` 完整跑过 3830 tests。
- 2026-05-11 review 修复后，本机同时有多个其他 plan worktree 的 Rust 编译占满内存/swap，`cargo test meridian_open` 在链接测试二进制阶段被 SIGKILL；错误发生在 link 阶段，未出现 Rust 编译错误或测试断言失败。该项需在资源释放或 PR CI 中补足门禁。
- `ambient_north_wastes.json` 用 `minecraft:entity.phantom.flap` 替代 plan 中的 `minecraft:entity.wind_charge.wind_burst`；当前 Fabric/MC 1.20.1 没有 wind charge 音效。

### 遗留 / 后续

- 无功能遗留。实际听感仍建议在后续手动 QA 中用 `runClient` 走一遍 zone/TSY/tribulation 场景，参数可继续按听感微调。
