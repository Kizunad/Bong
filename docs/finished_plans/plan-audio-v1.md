# Bong · plan-audio-v1

**音效 / 音乐专项**。HUD §5 把音效作为沉浸式反馈主通道。**本 plan 核心策略：100% 复用 MC vanilla SoundEvent，通过层叠 / 变调 / 延时组合产生修仙氛围 —— 不自制任何音频资源**。目标：LLM 维护，无画面/音效制作成本。

**世界观锚点**：`worldview.md §八` 天道叙事语调冷漠古意；`worldview.md §七` 音效作为天道注视的隐性信号之一；`worldview.md §三` 修炼/战斗场景的听觉锚点。

**交叉引用**：`plan-HUD-v1.md §5`（音效触发表主索引）· `plan-combat-v1.md §4/§7`（战斗打击感 / 防御反馈）· `plan-tribulation-v1.md §2`（天劫氛围）· `plan-narrative-v1.md`（升级/顿悟 narration 同步播放）。

---

## §0 设计轴心

- [ ] **零自制资源**：不做 `.ogg` 文件，不做 resource pack，不做 sound mod
- [ ] **SoundRecipe = JSON**：层叠多个 vanilla 音效 + pitch/volume/delay → LLM 直接编辑 JSON 造新效果
- [ ] **客户端播放，服务端触发**：server 发 `PlaySoundRecipe { recipe_id, pos }` → client 按 recipe 依次播 vanilla 音
- [ ] **可替换层**：未来若有资源包，recipe 里 `"minecraft:..."` 可换为 `"bong:..."`（同 id 约定），不改代码
- [ ] **音效 ≠ 文字**：关键信息（渡虚劫警告 / 真元见底 / 弹反窗口）都有独立可辨识音，不依赖文字 tooltip

---

## §1 核心策略：vanilla 音效组合

### §1.1 资源来源

- MC 1.20.1 vanilla SoundEvent（约 1000+ 种），按 namespace `minecraft:*` 引用
- 客户端 `MinecraftClient.getSoundManager().play(...)` 即可播放
- **全部可用的音材**：原版生物叫声、方块放置/破坏、环境（雨/雷/洞穴）、note_block 七音、末影事件、附魔台、beacon、bell 等

### §1.2 修仙适用 vanilla 音材白名单（LLM 写 recipe 参考）

避免生僻/不合氛围 id，LLM 新增 recipe 时**优先**从这 50 条内挑：

**低沉 / 庄重**（突破 / 天劫 / 重击）：
`block.anvil.land` · `block.anvil.fall` · `block.anvil.destroy` · `block.beacon.activate` · `block.beacon.deactivate` · `entity.lightning_bolt.thunder` · `entity.lightning_bolt.impact` · `entity.enderdragon.growl` · `entity.enderdragon.flap` · `entity.wither.spawn` · `block.respawn_anchor.deplete`

**清脆 / 铃钟**（弹反 / 悟道 / 升级）：
`block.bell.use` · `block.bell.resonate` · `block.note_block.bell` · `block.note_block.chime` · `block.note_block.pling` · `block.note_block.xylophone` · `block.amethyst_block.chime` · `block.amethyst_block.resonate` · `entity.experience_orb.pickup` · `entity.player.levelup`

**魔幻 / 玄异**（渡劫 / 虚化 / 神识）：
`entity.enderman.teleport` · `block.portal.ambient` · `block.portal.trigger` · `block.end_portal.spawn` · `block.end_portal_frame.fill` · `block.conduit.activate` · `block.conduit.ambient` · `block.enchantment_table.use` · `item.trident.riptide_1`

**交互 / 日常**（采药 / 种田 / 交易）：
`block.grass.break` · `block.grass.step` · `block.rooted_dirt.hit` · `block.gravel.place` · `block.water.ambient` · `entity.generic.drink` · `entity.generic.eat` · `block.brewing_stand.brew` · `block.composter.fill` · `item.book.page_turn` · `item.book.put`

**状态警告**（HP / qi / 负面）：
`entity.irongolem.step` · `entity.player.hurt` · `entity.villager.no` · `entity.wither.hurt` · `block.fire.extinguish` · `block.note_block.bass` · `block.note_block.didgeridoo` · `entity.zombie.villager.converted` · `block.soul_sand.hit` · `block.bone_block.break` · `entity.generic.extinguish_fire` · `entity.generic.explode`

> 超出此清单可用，但需 §10 启动期校验确认 id 真实存在。MC 完整列表约 1000 条，大多数非氛围适配。

### §1.3 组合手法

| 技巧 | 作用 | 举例 |
|---|---|---|
| **变调** pitch 0.5-2.0 | 改变音色 / 模拟距离 | 远雷 = `entity.lightning_bolt.thunder` pitch 0.5（低沉） |
| **叠层** 同 tick 多音 | 合成新音 | 心跳 = `irongolem.step`×0.4 pitch + `player.hurt`×0.2 volume |
| **延时** delay ticks | 制造节奏/回响 | 金属弹反 = `anvil.land`×1.5p + 2tick 后 `note_block.bell`×1.8p |
| **空间化** 3D pos | 方向感 | 天劫远雷在天劫者位置播，其他玩家听到方向 |
| **循环** loop flag | 持续状态 | HP<30% 心跳 loop，5tick 一次 |

---

## §2 SoundRecipe JSON 结构

路径：`server/assets/audio/recipes/*.json`（纯数据，热加载可做）。

```json
{
  "id": "heartbeat_low_hp",
  "layers": [
    { "sound": "minecraft:entity.irongolem.step", "volume": 0.4, "pitch": 0.6, "delay_ticks": 0 },
    { "sound": "minecraft:entity.player.hurt",    "volume": 0.15, "pitch": 0.7, "delay_ticks": 2 }
  ],
  "loop": { "interval_ticks": 20, "while_flag": "hp_below_30" },
  "attenuation": "player_local"
}
```

```json
// 弹反成功（清脆金属）
{
  "id": "parry_clang",
  "layers": [
    { "sound": "minecraft:block.anvil.land",       "volume": 0.5, "pitch": 1.6, "delay_ticks": 0 },
    { "sound": "minecraft:block.note_block.bell",  "volume": 0.3, "pitch": 1.8, "delay_ticks": 2 },
    { "sound": "minecraft:entity.experience_orb.pickup", "volume": 0.2, "pitch": 1.4, "delay_ticks": 4 }
  ]
}
```

```json
// 渡虚劫远雷（全服广播）
{
  "id": "tribulation_thunder_distant",
  "layers": [
    { "sound": "minecraft:entity.lightning_bolt.thunder", "volume": 0.3, "pitch": 0.4, "delay_ticks": 0 },
    { "sound": "minecraft:entity.enderdragon.growl",      "volume": 0.15, "pitch": 0.3, "delay_ticks": 40 }
  ],
  "attenuation": "global_hint"  // 全服玩家都听得到，但根据与渡劫点距离变弱
}
```

```json
// 真元见底（"叮"一声警告）
{
  "id": "qi_depleted_warning",
  "layers": [
    { "sound": "minecraft:block.note_block.pling", "volume": 0.4, "pitch": 0.7, "delay_ticks": 0 },
    { "sound": "minecraft:block.note_block.bell",  "volume": 0.2, "pitch": 0.5, "delay_ticks": 3 }
  ]
}
```

```json
// 境界突破（庄重）
{
  "id": "realm_breakthrough",
  "layers": [
    { "sound": "minecraft:block.beacon.activate",        "volume": 0.6, "pitch": 0.8, "delay_ticks": 0 },
    { "sound": "minecraft:block.bell.use",                "volume": 0.4, "pitch": 1.0, "delay_ticks": 10 },
    { "sound": "minecraft:block.enchantment_table.use",  "volume": 0.3, "pitch": 0.9, "delay_ticks": 20 },
    { "sound": "minecraft:entity.enderdragon.flap",      "volume": 0.2, "pitch": 1.2, "delay_ticks": 35 }
  ]
}
```

```json
// 药入体（咕咚 + 温热）
{
  "id": "pill_consume",
  "layers": [
    { "sound": "minecraft:entity.generic.drink",            "volume": 0.4, "pitch": 1.0, "delay_ticks": 0 },
    { "sound": "minecraft:block.brewing_stand.brew",        "volume": 0.3, "pitch": 1.2, "delay_ticks": 5 }
  ]
}
```

---

## §3 典型 recipe 清单（MVP ~20 条）

对齐各 plan 的关键事件，全部走 vanilla 层叠：

| recipe_id | 触发来源 plan | priority | category | attenuation | vanilla 层叠骨架 |
|---|---|---|---|---|---|
| `heartbeat_low_hp` | combat | 70 | HOSTILE | player_local | irongolem.step + player.hurt |
| `qi_depleted_warning` | combat / cultivation | 75 | HOSTILE | player_local | note_block.pling + note_block.bell |
| `parry_clang` | combat §4 | 85 | HOSTILE | world_3d | anvil.land + note_block.bell + experience_orb.pickup |
| `cast_interrupt` | combat | 80 | HOSTILE | world_3d | fire.extinguish + note_block.bass |
| `stance_switch` | combat §4 | 60 | VOICE | player_local | note_block.chime + enderman.teleport quiet |
| `phase_shift_in` | combat | 65 | HOSTILE | world_3d | enderman.teleport + block.portal.ambient |
| `tribulation_thunder_distant` | tribulation | **95** | AMBIENT | global_hint | lightning.thunder pitch-down + enderdragon.growl |
| `tribulation_wave_impact` | tribulation | **98** | HOSTILE | world_3d | lightning.impact + generic.explode quiet |
| `realm_breakthrough` | cultivation | 90 | VOICE | zone_broadcast | beacon.activate + bell.use + enchantment_table.use + enderdragon.flap |
| `realm_regression` | cultivation | 75 | VOICE | player_local | bell.resonate pitch-down + zombie.villager.converted |
| `meridian_crack` | cultivation | 70 | HOSTILE | player_local | bone_block.break + player.hurt pitch-up |
| `pill_consume` | alchemy | 40 | VOICE | player_local | generic.drink + brewing_stand.brew |
| `furnace_boom` | alchemy（炸炉） | 70 | BLOCKS | world_3d | generic.explode + anvil.destroy |
| `hammer_strike_light` | forge Tempering J | 50 | BLOCKS | world_3d | anvil.land pitch-up + wood.hit quiet |
| `hammer_strike_heavy` | forge Tempering K | 55 | BLOCKS | world_3d | anvil.land pitch-down + anvil.fall |
| `hammer_strike_fold` | forge Tempering L | 50 | BLOCKS | world_3d | anvil.place + fire.ambient |
| `harvest_pluck` | botany | 30 | BLOCKS | world_3d | grass.break + grass.step |
| `till_soil` | lingtian | 30 | BLOCKS | world_3d | rooted_dirt.hit + gravel.place |
| `plot_replenish` | lingtian | 40 | BLOCKS | world_3d | water.ambient + amethyst.chime |
| `niche_breach` | social（灵龛失效） | 80 | VOICE | player_local | bell.resonate pitch-down + soul_sand.hit |
| `exposure_name` | social（身份暴露） | 45 | VOICE | player_local | item.book.page_turn + note_block.harp |
| `skill_lv_up` | skill | 55 | VOICE | player_local | bell.use + note_block.chime + experience_orb.pickup |

**priority 段位约定**：环境 0-30 · 日常交互 30-50 · 重要状态 50-70 · 关键战斗 70-85 · 世界级事件 85-100

---

## §4 环境 BGM 策略

**也走 vanilla 环境音堆叠 + MC 内建 music disc？**

- [ ] 区域氛围**仅用 vanilla `ambient.*` / biome 音**（cave / underwater / 下雨）组合，不自制 BGM
- [ ] 特殊场景（天劫 / 突破）用 **music disc 类 event**（`music_disc.11` / `music_disc.13`）短时替代正常 BGM
- [ ] **没有完整作曲**——接受音乐表现弱于音效的取舍，集中火力做音效
- [ ] 未来若接资源包：`bong:music/qingyun_peak` 等自定义 BGM 按 `recipe` 架构挂接

---

## §5 混音与优先级

- [ ] **优先级队列**：同 tick 多触发时，取 top-N（默认 N=4）按 priority 排序播放，其他丢弃
  - priority 由 recipe 自带（0-100，段位见 §3 末尾）
  - **抢占规则**：`priority >= 85`（世界级事件）**打断**正在播放的同类 loop 与 top-N 队列尾部；`< 85` 只进队列，不打断
  - loop 类 recipe 永远占队列最后 1 槽（默认 top-3 one-shot + 1 loop）
- [ ] **BGM ducking**：依据 `CombatStateStore.in_combat`（client 端，由 combat §1.5 维护；来自 server `bong:combat/state_sync`），值为 true 时 AMBIENT 音量 ×0.3，切换时 2s 内线性过渡
- [ ] **玩家音量分组**（走 MC 原生 `SoundCategory`）：
  - `MASTER` 主音量
  - `HOSTILE` 战斗音（HP / 真元 / 弹反 / 受击）
  - `AMBIENT` 环境（BGM / 天气）
  - `VOICE` 叙事/UI（skill_lv_up / exposure_name）
  - `BLOCKS` 交互（till_soil / harvest_pluck / furnace_boom）
- [ ] **空间化 attenuation**：
  - `player_local` 只自己听（HP/qi 警告）
  - `world_3d` 按距离衰减（默认）
  - `global_hint` 全服听得到但根据距离变弱（渡虚劫 / 域崩）
  - `zone_broadcast` 仅同 zone 玩家（区域级事件）

---

## §6 数据契约

### Server

```rust
pub struct SoundRecipeRegistry {
    pub recipes: HashMap<RecipeId, SoundRecipe>,
}

pub struct SoundRecipe {
    pub id: String,
    pub layers: Vec<SoundLayer>,
    pub loop_cfg: Option<LoopConfig>,
    pub priority: u8,
    pub attenuation: Attenuation,
    pub category: SoundCategory,
}

pub struct SoundLayer {
    pub sound: String,        // "minecraft:..." 或 "bong:..."（未来）
    pub volume: f32,
    pub pitch: f32,
    pub delay_ticks: u32,
}

pub enum Attenuation { PlayerLocal, World3D, GlobalHint, ZoneBroadcast }
```

- [ ] `PlaySoundRecipeEvent` → 网络包 → client
- [ ] 启动期加载 `server/assets/audio/recipes/*.json`
- [ ] **热加载（MVP 级）**：debug 命令 `/audio reload` 触发 registry 重扫目录 —— 为 LLM 快速迭代 recipe 必需（每次改 JSON 不想重启服务器）；不做文件监听（避免 fs 依赖）

### Client

- [ ] `SoundRecipePlayer`（Fabric 侧）：接收网络包 → 按 layer 调度 `MinecraftClient.getSoundManager().play(...)`
- [ ] Loop 管理：`while_flag` 由 Client 侧 Store 维护（`PlayerStateStore.hp_percent < 0.3`）→ 自动停止
- [ ] 按 `SoundCategory` 走 MC 原生音量设置

### Channel 与网络包

```rust
// Server → Client
pub struct PlaySoundRecipePayload {
    pub recipe_id: String,        // 必填
    pub instance_id: u64,         // 本次播放的唯一 id（server 生成，用于之后 stop 定向）
    pub pos: Option<BlockPos>,    // world_3d / zone_broadcast 时必填；player_local / global_hint 可空
    pub flag: Option<String>,     // 如 "hp_below_30"，用于 loop while_flag 检测绑定
    pub volume_mul: f32,          // 额外乘子（默认 1.0），便于同 recipe 不同情境微调
    pub pitch_shift: f32,         // 全层 pitch 额外偏移（默认 0，单位 ±octave 语义同 MC）
}

pub struct StopSoundRecipePayload {
    pub instance_id: u64,         // **指定具体实例**（解决同 recipe 多次循环叠加问题）
    pub fade_out_ticks: u32,      // 淡出时长（0 = 立即停）
}
```

- [ ] Channel：`bong:audio/play` · `bong:audio/stop`
- [ ] instance_id 由 server 单调分配；client 维护 `HashMap<u64, ActiveRecipePlayback>`
- [ ] player_local 类 recipe 发送目标 = 单个玩家；world_3d / zone_broadcast 走现有世界广播管道（视 attenuation 由 client 决定是否播）

---

## §7 MVP 阶段划分

| Phase | 内容 | 验收 |
|---|---|---|
| P0 | SoundRecipe schema + Registry + JSON 加载 | 启动加载 ~20 份 recipe 无错 |
| P1 | Server → Client 网络包 + `SoundRecipePlayer` | 测试 recipe "pill_consume" 能播出组合音 |
| P2 | 接入 combat（heartbeat / qi / parry / cast / stance）| HP < 30% 自动循环；弹反清脆可辨 |
| P3 | 接入 alchemy / forge / botany / lingtian | 各 session 关键事件有独立音 |
| P4 | 接入 tribulation / cultivation / skill | 境界突破 + 渡虚劫远雷 + skill 升级 |
| P5 | 混音队列 + ducking + attenuation | 多事件同 tick 只取 top 4；战斗进入压低环境音 |
| P6 | LLM 维护验证：让 LLM 新增 5 条 recipe（从白名单 §1.2 挑 id） + `/audio reload` 热加载生效 | 完全靠编辑 JSON，不改代码，效果可辨 |

---

## §8 跨 plan 钩子

- [ ] **plan-HUD-v1 §5**：触发表合并到本 plan §3 recipe 清单（HUD 只管视觉反馈 + 触发点）
- [ ] **plan-combat-v1 §4/§7**：弹反 / 截脉 / 过载 / 虚化 触发 recipe
- [ ] **plan-tribulation-v1 §2**：渡虚劫三阶段音（远雷 → 锁定嗡鸣 → 劫波撞击）
- [ ] **plan-cultivation-v1**：突破 / 跌境 / 经脉裂 触发
- [ ] **plan-alchemy-v1 / plan-forge-v1**：session 各阶段音（火候 / 锻打 / 炸炉 / 炸砧）
- [ ] **plan-botany-v1 / plan-lingtian-v1**：采集 / 开垦 / 补灵 session 音
- [ ] **plan-skill-v1**：Lv up / 残卷消耗 触发
- [ ] **plan-social-v1**：灵龛失效 / 身份暴露 / 盟约成立
- [ ] **plan-narrative-v1**：每条 narration 前可选播"开始音"（bell/page_turn）引起玩家注意

---

## §9 TODO / 开放问题（v2+）

- [ ] **动态混音曲线**：玩家状态（真元/HP/境界）影响 recipe 选择（境界越高音色越深沉）
- [ ] **EQ / 滤波**：MC vanilla 不支持，若要加需 resource pack + 自定义处理
- [ ] **Agent 生成音效叙事**：agent 为关键事件（渡虚劫/域崩）临场挑 recipe + 改 pitch 做情绪调节
- [ ] **NPC 声线**：散修/货商语音（目前无计划，依赖 narration text）
- [ ] **玩家自定义 recipe**：玩家上传 JSON 自造音（服务器审核）
- [ ] **vanilla 音效不够用时的 fallback**：需要哪些"无法用 vanilla 组合出"的效果，列在此处等资源包补
- [ ] **资源包路径迁移**：未来接 `bong:` namespace 时需要的 mod / datapack 打包结构

---

## §10 风险与对策

| 风险 | 对策 |
|---|---|
| vanilla 音效受限 → 某些效果做不出来 | 接受取舍；做不出来的效果降级为"不播 / 用叙事替代"；列入 §9 TODO |
| 多 recipe 叠加混乱 | §5 优先级队列 top-N + SoundCategory 分组控制 |
| loop 音遗漏停止 → 一直响 | `while_flag` + 客户端 tick 每秒自动校验 flag 仍满足，否则停 |
| LLM 编辑 JSON 出错 → sound id 不存在 | 启动期校验所有 recipe 的 sound id 是否在 vanilla 注册表；失败 recipe 跳过（不 crash）+ log |
| 玩家觉得"听起来像 MC" | 大量变调 + 层叠能显著脱离原感；若仍不足，未来接资源包即可（架构已留口）|
| 音量过大 / 扰人 | 走 MC 原生 SoundCategory，玩家可细粒度调节 |

---

## §11 进度日志

- 2026-04-25：代码核查未发现 SoundRecipe schema/Registry/JSON/网络包/Player 任何实装（server `assets/audio/` 不存在，`SoundRecipe/PlaySoundRecipe` 符号缺失，client 仅 BongPunchCombo 直接用 vanilla SoundEvents），P0/P1 暂按未实装保留 `[ ]`
