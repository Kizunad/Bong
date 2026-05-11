# Bong · plan-audio-implementation-v1 · 骨架

音效系统全事件铺量与精细化——在 `plan-audio-world-v1` ✅ active 基础上拓展。audio-world-v1 已覆盖 **P0 区域 ambient loop+昼夜切换+MusicStateMachine / P1 战斗音乐+修炼冥想音+低HP心跳 / P2 TSY 负压音景+节律音差+天劫氛围**，建立了完整的 `MusicStateMachine`（5 状态）+ `AmbientZoneS2c` 管道 + 6 区域 ambient recipe。本 plan **不重复**这些基础管道和 ambient 层，而是在其之上做 4 件事：① 全游戏事件 recipe 铺量（战斗/修炼/产出/社交 60+ recipe）② 3D 空间化精调 ③ 7 流派专属音效集成 ④ 音量分层混合系统。

**世界观锚点**：`worldview.md §八` 天道叙事语调冷漠古意（音效不甜腻）· `§七` 音效作为天道注视隐性信号 · `§三` 修炼/战斗听觉锚点 · `§四` 战斗打击感需独立音 · `§五` 七流派各自标志性音效

**library 锚点**：`cultivation-0002 烬灰子内观笔记 §音论`（振荡/接触面与音的物理推导）

**前置依赖**：
- `plan-audio-world-v1` 🆕 active → **硬依赖**（MusicStateMachine / AudioTriggerS2c / AmbientZoneS2c / 6 区域 ambient 全由其建立）
- `plan-audio-v1` ✅ → SoundRecipe JSON 规范 / 白名单 / 组合手法 / 38 existing recipes
- `plan-vfx-v1` ✅ → VFX 事件通道可复用
- `plan-combat-no_ui` ✅ + `plan-combat-ui_impl` ✅ → 战斗 hit/parry/dodge 事件垫
- `plan-cultivation-v1` ✅ → 突破/修炼/经脉打通事件垫
- `plan-tribulation-v1` ✅ → 天劫雷序列音
- `plan-forge-v1` ✅ → 锻造状态机
- `plan-alchemy-v1` ✅ → 炼丹状态机
- `plan-lingtian-v1` ✅ → 灵田动作事件

**反向被依赖**：
- `plan-baomai-v3` 🆕 active → 崩拳 5 招音效 recipe
- `plan-dugu-v2` 🆕 active → 蚀针飞行/命中/侵染音
- `plan-tuike-v2` 🆕 active → 伪皮蜕落/转移污染音
- `plan-zhenfa-v2` 🆕 active → 阵眼激活/诡雷爆/聚灵阵嗡鸣
- `plan-combat-gamefeel-v1` 🆕 skeleton → 战斗 juice 音效层

---

## 与 audio-world-v1 的边界

| 维度 | audio-world-v1 已做 | 本 plan 拓展 |
|------|-------------------|-------------|
| ambient | 6 区域 ambient loop + 昼夜 crossfade | 不碰。ambient 留在 audio-world |
| MusicStateMachine | 5 状态（AMBIENT/COMBAT/CULTIVATION/TSY/TRIBULATION）+ crossfade | 不碰状态机本身。仅为其提供更多 recipe |
| 战斗音乐 | combat_music.json（整体战斗 loop） | 60+ 细分 recipe：7 流派各自 hit/parry/dodge 独立音 + 全力一击 charge/release + 过载撕裂 |
| 修炼音 | 冥想 loop + 经脉打通音 | 突破三境各自独立音（引气清脆 / 凝脉厚重 / 固元震响）+ 顿悟瞬时音 |
| 产出 | 无 | forge 敲击 / alchemy 沸腾 / 灵田补灵 / 灵龛设置 各自 recipe |
| 3D 空间化 | 复用 vanilla SoundInstance | 精调 attenuation 参数（天劫远雷定向 / NPC 脚步距离衰减 / 弹反贴身立体） |
| 混合/均衡 | 无 | 全局 3 bus（战斗/环境/UI）+ 沉浸模式 UI 静音 + telemetry |

---

## 接入面 Checklist

- **进料**：`audio::SoundRecipePlayer` ✅ / `audio::AudioTriggerS2c` ✅ / `MusicStateMachine`（audio-world-v1 出料）/ `combat::HitEvent` / `combat::ParryEvent` / `combat::DodgeEvent` / `forge::ForgeSessionState` / `alchemy::BrewSessionState` / `lingtian::LingtianActionEvent` / `social::PactEvent` / 流派 skill cast events
- **出料**：60+ recipe JSON（`server/assets/audio/recipes/`）/ `AudioBusMixer`（3 bus 分层）/ 3D attenuation profile per-recipe / `AudioTelemetry`（recipe 播放频次统计）
- **共享类型/event**：复用 `AudioTriggerS2c`，不新增通道——仅在各模块的 event handler 中追加 emit
- **跨仓库契约**：server 各模块 emit `AudioTriggerS2c(recipe_id, pos, entity)` → client `SoundRecipePlayer` 消费；agent `tiandao::narration` 可附带 `audio_trigger_id` 字段

---

## §0 设计轴心

- [ ] **零自制资源**：不做 .ogg、resource pack、sound mod——100% vanilla SoundEvent 组合
- [ ] **recipe 差异化**：7 流派同类事件（如 hit）必须各自不同的音效组合（体修=沉重低音 / 暗器=锐利高频 / 毒蛊=阴渗嗡鸣 / 涡流=真空吸音）
- [ ] **3D 空间化**：战斗音贴身（no attenuation）/ 环境音中距（linear 16 格）/ 天劫远雷远距（linear 128 格 + directional）
- [ ] **不用 AudioTrigger 洪流**：同一 recipe 100ms 内不重复 emit（server 侧 dedup）

---

## 阶段总览

| 阶段 | 内容 | 状态 |
|----|------|----|
| P0 | 战斗细分 recipe 铺量（20+） | ⬜ |
| P1 | 修炼/产出/社交 recipe 铺量（25+） | ⬜ |
| P2 | 3D 空间化精调 + 环境 ambient detail | ⬜ |
| P3 | 7 流派专属音效集成 | ⬜ |
| P4 | 音量分层系统 + 沉浸模式 + telemetry | ⬜ |
| P5 | 饱和化测试 + 多人压测 | ⬜ |

---

## P0 — 战斗细分 recipe 铺量 ⬜

### 交付物

1. **通用战斗 recipe（12 条）**
   - `hit_light.json`：`minecraft:entity.player.attack.strong`(pitch 1.2, volume 0.4)（轻击）
   - `hit_heavy.json`：`minecraft:entity.player.attack.strong`(pitch 0.7, volume 0.6) + `minecraft:block.anvil.land`(pitch 0.3, volume 0.15)（重击）
   - `hit_critical.json`：`minecraft:entity.player.attack.crit`(pitch 0.8, volume 0.5) + `minecraft:entity.lightning_bolt.impact`(pitch 2.0, volume 0.1)（暴击）
   - `parry_success.json`（已存在 → 确认接线 + 参数微调）
   - `parry_perfect.json`：`minecraft:block.anvil.use`(pitch 1.5, volume 0.5) + `minecraft:entity.experience_orb.pickup`(pitch 0.5, volume 0.2)（完美弹反——清脆+共鸣）
   - `dodge_success.json`：`minecraft:entity.phantom.flap`(pitch 1.5, volume 0.3)（闪避风声）
   - `charge_start.json`：`minecraft:block.beacon.activate`(pitch 0.5, volume 0.2, fade_in 0.5s)（蓄力开始低频嗡）
   - `charge_release.json`：`minecraft:entity.generic.explode`(pitch 1.5, volume 0.4) + `minecraft:block.beacon.deactivate`(pitch 0.3)（蓄力释放）
   - `overload_tear.json`：`minecraft:entity.player.hurt`(pitch 0.5, volume 0.5) + `minecraft:block.glass.break`(pitch 0.8, volume 0.3)（过载撕裂——肉痛+碎裂）
   - `qi_collision.json`：`minecraft:block.amethyst_block.hit`(pitch 0.6, volume 0.3) + `minecraft:entity.warden.sonic_boom`(pitch 2.5, volume 0.08)（异体排斥碰撞——水晶震+超声）
   - `kill_confirm.json`：`minecraft:entity.player.levelup`(pitch 0.3, volume 0.2, delay 0.3s)（击杀确认——低调但确定）
   - `wound_inflict.json`：`minecraft:entity.player.hurt`(pitch 0.8) + `minecraft:block.bone_block.break`(pitch 1.2, volume 0.15)（造成伤口——肉+骨）

2. **server 侧 emit 接线**
   - `combat::resolve_system`：按 damage tier 选择 `hit_light`/`hit_heavy`/`hit_critical`
   - `combat::parry_system`：按 timing window 选择 `parry_success`/`parry_perfect`
   - `combat::dodge_system`：emit `dodge_success`
   - `combat::charge_system`：charge_start 开始 / charge_release 释放
   - `combat::overload_system`：emit `overload_tear`
   - `qi_physics::collision`：emit `qi_collision`
   - `combat::kill_system`：emit `kill_confirm`
   - `combat::wound_system`：emit `wound_inflict`
   - 100ms dedup：同一 entity 同一 recipe_id 内不重发

### 验收抓手

- 测试：`server::combat::tests::hit_tier_selects_correct_recipe` / `server::combat::tests::parry_timing_selects_recipe` / `server::audio::tests::dedup_100ms`
- 手动：战斗中 → 轻击清脆 → 重击沉闷 → 完美弹反金属共鸣 → 蓄力嗡 → 释放爆 → 过载撕裂痛感

---

## P1 — 修炼/产出/社交 recipe 铺量 ⬜

### 交付物

1. **修炼 recipe（8 条）**
   - `meditate_loop.json`（已在 audio-world-v1 → 确认不重复）
   - `meridian_open.json`：`minecraft:block.amethyst_cluster.break`(pitch 1.5, volume 0.3)（经脉打通——清脆灵感）
   - `breakthrough_yinqi.json`：`minecraft:entity.experience_orb.pickup`(pitch 0.8, volume 0.3) + soft chime（引气突破——轻盈）
   - `breakthrough_ningmai.json`：`minecraft:block.beacon.activate`(pitch 0.6, volume 0.4) + `minecraft:block.amethyst_block.resonate`(pitch 0.5)（凝脉——厚重共鸣）
   - `breakthrough_guyuan.json`：`minecraft:entity.warden.sonic_boom`(pitch 0.3, volume 0.2) + `minecraft:entity.lightning_bolt.thunder`(pitch 0.5, volume 0.15)（固元——震响远传）
   - `breakthrough_fail.json`：`minecraft:block.glass.break`(pitch 0.6, volume 0.4) + `minecraft:entity.player.hurt`(pitch 0.5)（突破失败——碎裂+痛）
   - `enlightenment_flash.json`：`minecraft:block.amethyst_cluster.hit`(pitch 2.0, volume 0.5)（顿悟瞬间——极高频清音一闪）
   - `qi_depleted_warning.json`（已在 audio-world-v1 → 确认接线）

2. **产出 recipe（10 条）**
   - `forge_hammer_light.json`：`minecraft:block.anvil.use`(pitch 1.3, volume 0.3)（轻锤）
   - `forge_hammer_heavy.json`：`minecraft:block.anvil.land`(pitch 0.8, volume 0.5) + `minecraft:block.lava.pop`(pitch 0.5, volume 0.1)（重锤+火星）
   - `forge_inscribe.json`：`minecraft:block.enchantment_table.use`(pitch 1.2, volume 0.3)（铭文刻划——附魔音变调）
   - `forge_consecrate.json`：`minecraft:block.beacon.activate`(pitch 1.0, volume 0.4)（开光）
   - `forge_complete.json`：`minecraft:entity.player.levelup`(pitch 0.8, volume 0.3)（锻造完成）
   - `alchemy_bubble.json`：`minecraft:block.pointed_dripstone.drip_lava`(pitch 1.5, volume 0.2, loop 2s)（炼丹沸腾——岩浆滴变调循环）
   - `alchemy_overheat.json`：`minecraft:block.fire.extinguish`(pitch 0.5, volume 0.4)（火候过高）
   - `alchemy_complete.json`：`minecraft:block.brewing_stand.brew`(pitch 0.8, volume 0.4)（成丹）
   - `alchemy_fail.json`：`minecraft:block.fire.extinguish`(pitch 0.3, volume 0.5) + `minecraft:block.glass.break`(pitch 1.0)（炸炉）
   - `lingtian_till.json`：`minecraft:item.hoe.till`(pitch 0.9, volume 0.3)（灵田开垦）

3. **社交 recipe（7 条）**
   - `niche_establish.json`：`minecraft:block.respawn_anchor.charge`(pitch 0.7, volume 0.4)（灵龛建立——锚定音）
   - `pact_bind.json`：`minecraft:block.amethyst_block.chime`(pitch 0.8, volume 0.3) × 2 layer（结契——双声共振）
   - `renown_gain.json`：`minecraft:entity.experience_orb.pickup`(pitch 1.0, volume 0.2)（声望提升——微妙）
   - `renown_loss.json`：`minecraft:block.soul_sand.break`(pitch 0.5, volume 0.2)（声望下降——沉闷）
   - `death_insight.json`：`minecraft:ambient.cave`(pitch 0.3, volume 0.3) + `minecraft:block.soul_sand.break`(pitch 0.2, delay 1s)（遗念浮现——空洞+碎裂）
   - `inventory_open.json`：`minecraft:block.chest.open`(pitch 1.5, volume 0.15)（背包打开——轻巧）
   - `inventory_close.json`：`minecraft:block.chest.close`(pitch 1.5, volume 0.15)（背包关闭）

### 验收抓手

- 测试：`server::cultivation::tests::breakthrough_realm_selects_recipe` / `server::forge::tests::hammer_step_emits_audio` / `server::alchemy::tests::brew_phase_emits_audio`
- 手动：打坐 → 经脉打通清音 → 突破引气轻盈 / 凝脉厚重 / 固元震响 → 锻造敲击 → 炼丹沸腾 → 背包开关

---

## P2 — 3D 空间化精调 + 环境 detail ⬜

### 交付物

1. **3D attenuation profile 分类**
   - `SELF`：无衰减，仅自己听到（UI 音 / 修炼内心音 / 顿悟闪音）
   - `MELEE`：8 格线性衰减（战斗 hit/parry/dodge / 产出敲击）
   - `AREA`：32 格线性衰减（突破光柱音 / 灵龛设置 / 阵法激活）
   - `WORLD`：128 格线性衰减 + directional（天劫远雷 / 兽潮奔腾 / 全服异象）
   - 每条 recipe JSON 追加 `"attenuation": "SELF|MELEE|AREA|WORLD"` 字段
   - client `SoundRecipePlayer` 按 profile 设置 `SoundInstance.attenuation`

2. **NPC 脚步声空间化**
   - NPC 移动时 per-step `minecraft:entity.player.swim`(pitch 1.5, volume 0.1)（轻微脚步——不用真正的 footstep 音，那个太重）
   - 衰减：MELEE profile（8 格线性）
   - 材质差异：残灰方块上 pitch ×0.7 + 追加 `minecraft:block.sand.step`(volume 0.05)（沙感）
   - 水面 pitch ×0.8 + 追加 `minecraft:entity.player.splash`(pitch 2.0, volume 0.03)
   - server 不 emit——纯 client 侧按 entity position + block type 本地播放（减少网络开销）

3. **环境 ambient detail**（不替代 audio-world-v1 的 6 区域 ambient，而是叠加细节）
   - 灵泉湿地追加：偶发蛙鸣 `minecraft:entity.frog.ambient`(pitch 0.6, volume 0.05, random interval 10-30s)
   - 血谷追加：偶发岩石崩落 `minecraft:block.stone.break`(pitch 0.3, volume 0.08, random interval 20-60s, directional random)
   - 北荒追加：偶发狼嚎 `minecraft:entity.wolf.howl`(pitch 0.4, volume 0.06, random interval 30-90s, distance 64-128)
   - 坍缩渊追加：偶发金属撞击 `minecraft:block.anvil.land`(pitch 0.2, volume 0.03, random 15-45s)（古器残响暗示）

### 验收抓手

- 测试：`client::audio::tests::attenuation_profile_applies` / `client::audio::tests::npc_footstep_material_pitch` / `client::audio::tests::ambient_detail_interval_range`
- 手动：远距离听到天劫雷声有方向性 → 近距离战斗音贴身 → 站在灵泉湿地偶尔听到蛙鸣 → 跟 NPC 走 → 听到脚步声从远到近

---

## P3 — 7 流派专属音效集成 ⬜

### 交付物

每个流派 5 条 recipe（hit × 3 强度 + cast + signature）= 35 条

1. **爆脉流（baomai）**
   - `baomai_hit_light.json`：`minecraft:block.anvil.land`(pitch 1.0, volume 0.3)（沉重锤击）
   - `baomai_hit_heavy.json`：`minecraft:block.anvil.land`(pitch 0.6, volume 0.5) + `minecraft:entity.generic.explode`(pitch 2.0, volume 0.1)（爆裂重击）
   - `baomai_hit_critical.json`：爆裂+地面震 `minecraft:entity.warden.sonic_boom`(pitch 1.5, volume 0.15)
   - `baomai_cast.json`：`minecraft:block.beacon.activate`(pitch 0.3, volume 0.3)（崩拳蓄力——深沉嗡）
   - `baomai_signature.json`：`minecraft:entity.warden.sonic_boom`(pitch 0.5, volume 0.25)（爆脉流标志性——体内震波）

2. **蚀针流（dugu）**
   - 尖锐高频为主调：`minecraft:entity.arrow.shoot`(pitch 1.8) / `minecraft:entity.arrow.hit_player`(pitch 2.0) / `minecraft:entity.phantom.bite`(pitch 2.0, volume 0.2)

3. **蜕壳流（tuike）**
   - 黏腻+蜕变为主调：`minecraft:entity.slime.squish`(pitch 0.5) / `minecraft:block.honey_block.step`(pitch 0.3) / `minecraft:entity.phantom.flap`(pitch 0.3)

4. **涡流流（woliu）**
   - 真空+吸力为主调：`minecraft:entity.enderman.teleport`(pitch 0.3) / `minecraft:block.portal.ambient`(pitch 2.0, volume 0.1) / 吸入音 `minecraft:entity.generic.drink`(pitch 0.2)

5. **阵法流（zhenfa）**
   - 符文+共振为主调：`minecraft:block.enchantment_table.use`(pitch 0.8) / `minecraft:block.amethyst_block.resonate`(pitch 0.6) / 阵法连接嗡 `minecraft:block.beacon.ambient`(pitch 0.5)

6. **截脉流（zhenmai）** + **毒蛊流（dugu-poison variant）**
   - 截脉：金属碰撞清脆 `minecraft:block.anvil.use`(pitch 1.5) + 经脉震断 `minecraft:block.bone_block.break`
   - 毒蛊：阴渗嗡鸣 `minecraft:entity.bee.loop_aggressive`(pitch 0.3, volume 0.1) + 腐蚀 `minecraft:block.honey_block.break`(pitch 0.4)

7. **server 侧流派 recipe 路由**
   - `skill::cast_system`：按 `SkillSchool` enum 选择对应流派 recipe 前缀
   - hit recipe 选择：`{school}_hit_{light|heavy|critical}` 按 damage tier

### 验收抓手

- 测试：`server::skill::tests::school_routes_to_correct_recipe_prefix` / 35 条 recipe JSON schema 校验
- 手动：体修出拳 → 沉重锤击 → 暗器命中 → 尖锐穿刺 → 毒蛊攻击 → 阴渗嗡 → 涡流施法 → 真空吸音——每个流派听感明确不同

---

## P4 — 音量分层系统 + 沉浸模式 + telemetry ⬜

### 交付物

1. **`AudioBusMixer`**（`client/src/main/java/com/bong/client/audio/AudioBusMixer.java`）
   - 3 个 bus：`COMBAT`（战斗音 + hit + parry）/ `ENVIRONMENT`（ambient + 脚步 + 产出）/ `UI`（背包/菜单/toast）
   - 每个 bus 独立 volume slider（在设置界面追加，复用 owo-lib options screen）
   - Master volume 控制全局
   - 每条 recipe JSON 追加 `"bus": "COMBAT|ENVIRONMENT|UI"` 字段

2. **沉浸模式音量联动**
   - HUD 沉浸模式 ON（来自 hud-polish-v1 P0 `ImmersiveModeToggle`）→ `UI` bus volume → 0（静音 UI 音）
   - `COMBAT` + `ENVIRONMENT` bus 不变
   - 被攻击时临时恢复 `UI` bus 5s（与 HUD 恢复同步）

3. **`AudioTelemetry`**（`client/src/main/java/com/bong/client/audio/AudioTelemetry.java`）
   - 统计每条 recipe 30min 内播放次数
   - 超频告警：同一 recipe > 100 次/30min → 日志 warn（说明某处 emit 过于频繁需要 throttle）
   - 按 F8 打开 debug overlay：当前播放 recipe 列表 + bus volume 实时显示
   - 仅 debug build 可用

4. **ducking（闪避压低）**
   - `TRIBULATION` 状态时 `ENVIRONMENT` bus volume ×0.3（天劫压盖一切）
   - `COMBAT` 状态时 `ENVIRONMENT` bus volume ×0.6（战斗时环境音退后）
   - 突破/顿悟瞬间所有 bus 静音 0.1s → 单独播 breakthrough/enlightenment recipe（"世界安静了一瞬"效果）

### 验收抓手

- 测试：`client::audio::tests::bus_mixer_volume_independence` / `client::audio::tests::immersive_mode_mutes_ui_bus` / `client::audio::tests::ducking_tribulation_reduces_env`
- 手动：打开设置 → 调战斗音量 → 战斗中环境音自动退后 → 沉浸模式 → UI 音消失 → 被攻击 → 临时恢复 → 渡劫 → 环境音几乎消失只剩雷

---

## P5 — 饱和化测试 ⬜

### 交付物

1. **每条 recipe 全距离 e2e**
   - 60+ recipe × 4 attenuation × 近/中/远 3 距离 = 720+ test case
   - 自动化：`scripts/audio_recipe_test.sh`（spawn entity at distance → emit → assert client receives）

2. **多人压测**
   - 10 玩家同时战斗 → 50+ recipe 并发 emit → client 不掉帧 / 不爆音
   - 音量混合器在高负载下 bus 不互相干扰

3. **流派 × 强度矩阵**
   - 7 流派 × 3 强度 × 3 attenuation = 63 组合——确保每组合有独立听感

### 验收抓手

- 自动化脚本 + 帧率基线：30fps 下音效渲染开销 < 1ms
- PVP 实战校准：5v5 混战中每个流派的 hit 音能被区分

---

## Finish Evidence

- **落地清单**
  - P0 战斗细分 recipe：`server/assets/audio/recipes/hit_light.json` / `hit_heavy.json` / `hit_critical.json` / `parry_success.json` / `parry_perfect.json` / `dodge_success.json` / `charge_start.json` / `charge_release.json` / `overload_tear.json` / `qi_collision.json` / `kill_confirm.json` / `wound_inflict.json`；`server/src/audio/implementation.rs` 负责 damage tier / parry window / 100ms dedup，`server/src/network/audio_trigger.rs` 负责战斗、死亡、伤口 emit。
  - P1 修炼 / 产出 / 社交 recipe：`breakthrough_yinqi` / `breakthrough_ningmai` / `breakthrough_guyuan` / `breakthrough_fail` / `meridian_open` / `enlightenment_flash` / `forge_*` / `alchemy_*` / `lingtian_*` / `niche_establish` / `pact_bind` / `renown_gain` / `renown_loss` / `death_insight` / `inventory_open` / `inventory_close`；server 路由覆盖 cultivation / forge / alchemy / lingtian / social。
  - P2 3D 空间化 + ambient detail：`AudioAttenuation::{SELF,MELEE,AREA,WORLD}`、`AUDIO_MELEE_RADIUS=8`、`AUDIO_AREA_RADIUS=32`、`AUDIO_WORLD_RADIUS=128`；`server/src/audio/ambient.rs` 追加灵泉湿地蛙鸣、血谷岩崩、北荒狼嚎、TSY 金属残响；client `MinecraftSoundSink` 将 SELF / PLAYER_LOCAL 映射为无衰减。
  - P3 七流派专属音效：`baomai_*` / `dugu_*` / `dugu_poison_*` / `tuike_*` / `woliu_*` / `zhenfa_*` / `zhenmai_*` 共 35 条流派 recipe，`school_recipe_prefix` / `school_hit_recipe` 负责 hit/cast/signature 路由。
  - P4 bus / 沉浸 / telemetry：`AudioBus::{COMBAT,ENVIRONMENT,UI}` 跨 server schema、agent schema、client parser 对齐；`AudioBusMixer` 支持三 bus 独立 volume、沉浸模式 UI mute、COMBAT/TRIBULATION 环境 ducking；`SoundRecipePlayer` 在 combat active / HP 下降边沿恢复 UI bus 100 tick；`AudioTelemetry` 统计 30min recipe 播放次数并在超过 100 次时 warn；`NpcFootstepAudioController` 纯 client 本地 NPC 脚步声按材质选择 default/ash/water recipe，首帧只建状态不播放 phantom step。
  - P5 饱和化测试：recipe registry 锁定 184 条 JSON，schema/generated artifacts、server routing、registry-sourced attenuation recipient、全路径 dedup、client mixer、NPC footstep、music-state ducking 均有回归测试；多人实机压测与可视化调参未纳入自动 CI。

- **关键 commit**
  - `d94c11d2c` · 2026-05-11 · `plan-audio-implementation-v1: 扩展音频协议与 bus 契约`
  - `ad539c26d` · 2026-05-11 · `plan-audio-implementation-v1: 补齐音效 recipe 与 ambient detail`
  - `ba2b3a0b6` · 2026-05-11 · `plan-audio-implementation-v1: 接入音频事件路由与去重`
  - `d5b0713d6` · 2026-05-11 · `plan-audio-implementation-v1: 接入客户端混音与 NPC 脚步`
  - `c517e8c21` · 2026-05-11 · `fix(plan-audio-implementation-v1): 收紧音频路由与去重边界`

- **测试结果**
  - `cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`：`4314 passed; 0 failed; 0 ignored`。
  - `cd client && JAVA_HOME=/usr/lib/jvm/java-17-openjdk-amd64 PATH=/usr/lib/jvm/java-17-openjdk-amd64/bin:$PATH ./gradlew test build`：`BUILD SUCCESSFUL`。
  - `cd agent && npm run build && (cd packages/tiandao && npm test) && (cd packages/schema && npm test)`：`tiandao 52 files / 354 tests passed`，`schema 19 files / 370 tests passed`。

- **跨仓库核验**
  - server：`SoundRecipeRegistry::load_default()` / `AudioAttenuation` / `AudioBus` / `PlaySoundRecipeRequest` / `recipient_for_attenuation` / `AudioImplementationDedup` / `AudioEmitWriter` / `emit_*_audio_triggers`。
  - agent/schema：`agent/packages/schema/src/audio-event.ts`、`generated/audio-event-v1.json`、`generated/play-sound-recipe-event-v1.json`、`generated/sound-recipe-v1.json` 对齐 bus + attenuation。
  - client：`AudioEventEnvelope` 解析 inline recipe bus，`AudioRecipe.bus()`、`AudioBusMixer.effectiveVolume()`、`SoundRecipePlayer.setMusicState()`、`NpcFootstepAudioController.recipeForMaterial()`。

- **遗留 / 后续**
  - 本 plan 不新增 .ogg / resource pack；全部 recipe 仍按 vanilla SoundEvent 组合。
  - 音量设置 UI slider、F8 debug overlay、10 人实机压测属于后续可视化/调参工作；本 PR 已落地底层 bus、telemetry snapshot 与自动化回归护栏。
  - agent narration 的 `audio_trigger_id` 可在后续叙事联动 plan 中接入；当前未新增 agent→client 音频通道。
