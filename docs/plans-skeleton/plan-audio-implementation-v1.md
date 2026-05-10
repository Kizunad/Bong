# Bong · plan-audio-implementation-v1 · 骨架

音效系统端到端实现。承接 `plan-audio-v1` ✅ finished（SoundRecipe JSON 设计、vanilla 音效组合策略、50 条白名单、5 种组合手法）—— v1 把设计落地为可工作的 server↔client 音效管道。**零自制音频资源**，100% 复用 MC vanilla SoundEvent 通过层叠/变调/延时合成修仙氛围。

**世界观锚点**：`worldview.md §八` 天道叙事语调冷漠古意（音效不甜腻）· `§七` 音效作为天道注视隐性信号 · `§三` 修炼/战斗听觉锚点 · `§四` 战斗打击感需独立音 · `§五` 七流派各自标志性音效

**library 锚点**：`cultivation-0002 烬灰子内观笔记 §音论`（振荡/接触面与音的物理推导）

**前置依赖**：
- `plan-audio-v1` ✅ → SoundRecipe JSON 规范 / 白名单 / 组合手法
- `plan-vfx-v1` ✅ → VFX 事件通道可复用（`bong:vfx_event` CustomPayload）
- `plan-HUD-v1` ✅ → 音效触发表（HUD §5 已有分类索引）
- `plan-combat-no_ui` ✅ + `plan-combat-ui_impl` ✅ → 战斗 hit/parry/dodge 事件垫
- `plan-cultivation-v1` ✅ → 突破/修炼/经脉打通事件垫
- `plan-tribulation-v1` ✅ → 天劫雷序列音

**反向被依赖**：
- `plan-baomai-v3` 🆕 active → 崩拳 5 招音效 recipe
- `plan-dugu-v2` 🆕 skeleton → 蚀针飞行/命中/侵染音
- `plan-tuike-v2` 🆕 skeleton → 伪皮蜕落/转移污染音
- `plan-woliu-v2` 🆕 skeleton → 涡流真空吸入/紊流场音
- `plan-zhenfa-v2` 🆕 skeleton → 阵眼激活/诡雷爆/聚灵阵嗡鸣
- `plan-zhenmai-v2` 🆕 → 弹反正反馈金属音

---

## 接入面 Checklist

- **进料**：`plan-audio-v1` 33 个 recipe 草稿 JSON / `bong:vfx_event` CustomPayload 通道 / server 侧 `AudioTriggerEvent`（新建）/ client 侧 `MinecraftClient.getSoundManager()`
- **出料**：server `audio::SoundRecipeRegistry`（加载 recipes）+ `audio::AudioTrigger` component + `audio::play_recipe` system / client `AudioRecipePlayer`（解析 recipe 层层叠加播 vanilla SoundEvent）/ `AudioSpatializer`（3D 方向/衰减复用 vanilla）/ `bong:audio_trigger` CustomPayload 通道 / 首批 30+ recipe JSON
- **共享类型**：`AudioTriggerEvent { recipe_id, pos, entity }` → agent narration 可附带
- **跨仓库契约**：server `audio::*` → client `AudioRecipePlayer` | agent `tiandao::narration` 可 emit AudioTrigger

---

## §0 设计轴心

- [ ] **零自制资源**：不做 .ogg、resource pack、sound mod
- [ ] **SoundRecipe → server registry → client play** 端到端管道
- [ ] **3D 空间化**复用 vanilla SoundInstance（`attenuation` 参数选 player_local / linear / none）
- [ ] **AudioTrigger 走专用 CustomPayload**（与 `bong:vfx_event` 平行，避免 VFX 通道被音效洪流淹没）

---

## 阶段总览

| 阶段 | 内容 | 状态 |
|----|------|----|
| P0 | server `audio::*` 骨架（SoundRecipeRegistry + AudioTrigger component + play_recipe system + `bong:audio_trigger` CustomPayload）+ client `AudioRecipePlayer`（解析 recipe → 逐层播 vanilla SoundEvent）+ 3 个验证 recipe（`heartbeat_low_hp` / `parry_success` / `breakthrough_pulse`）e2e | ⬜ |
| P1 | 30+ recipe JSON 铺量：12 战斗（7 流派 hit + parry + dodge + 全力一击 charge + release + 过载撕裂）+ 6 修炼（打坐循环 / 经脉打通 / 突破三境 / 真元见底警告）+ 6 环境（灵压分区 / 汐转过渡 / 负压警告 / 伪灵脉出现 / 域崩倒数 / 天劫雷序列）+ 6 UI/状态（背包开/关 / 锻造敲击 / 炼丹沸腾 / 灵龛设置 / 死亡遗念 / 顿悟瞬时） | ⬜ |
| P2 | 3D 空间化：天劫远雷（定向）、NPC 脚步声（距离衰减）、弹反金属（贴身立体）+ 环境 ambient loop（灵泉湿地虫鸣、北荒风啸、坍缩渊负压低频嗡） | ⬜ |
| P3 | 流派 vN+1 音效集成：给 baomai-v3 / dugu-v2 / tuike-v2 / woliu-v2 / zhenfa-v2 各新增 3-5 recipe + agent narration 同步播 AudioTrigger（天道声音 + 音效联动） | ⬜ |
| P4 | 混合/均衡：全局音量分层（战斗/环境/UI 三个 bus）+ 沉浸模式一键静音 UI 音（保留战斗+环境）+ 音频 telemetry（各 recipe 播放频次 PVP 校准） | ⬜ |
| P5 | 饱和度测试：每个 recipe 全境界/全距离/全音量 e2e + 压测（10 玩家同时播 50 recipe 不掉帧） | ⬜ |

---

## Finish Evidence（待填）

- **落地清单**：server `audio::*` 模块 / client `AudioRecipePlayer` / 30+ recipe JSON / `bong:audio_trigger` channel
- **关键 commit**：P0-P5 各自 hash
- **测试结果**：30+ recipe e2e / 压测 10p × 50 recipe
- **遗留 / 后续**：agent narration 联动 AudioTrigger（plan-cross-system-patch-v1）/ 音效白名单扩展（新 MC 版本）
