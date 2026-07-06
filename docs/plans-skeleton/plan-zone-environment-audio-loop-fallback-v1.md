# plan-zone-environment-audio-loop-fallback-v1（骨架）

> **骨架（草案）**。一句话主题：`zone_environment` 客户端本地环境音对 `FogVeil` / `HeatHaze` 声明了专属 loop id（`mist_low_loop` / `cicada_summer_loop`），但 `EnvironmentAudioController.soundFor()` 漏接这两个 recipe_id，运行时统一静默退回 `minecraft:ambient.cave`。结果是 **阴霾 / 灵雾 / scorch 热浪等世界环境音全部塌成同一条洞穴底噪**，与已归档 plan 的分层音景设计不符。

> 立项动机：这是 world audio / ambience 主链里的**真运行时错接线**，不是已知的 ambient zone audio stale anchor，也不是 fauna audio fade stop ignored，更不是 weather overlay collapse。问题位于 `bong:zone_environment` → client 本地 loop 构造链，玩家在 scorch / TSY / 渡劫区 / HeavyHaze / LingMist / DroughtWind 等正常路径都可触发。

## 阶段总览

| 阶段 | 主题 | 路由 | 状态 |
|---|---|---|---|
| P0 | `FogVeil` / `HeatHaze` 环境音 loop id 被默认分支吞掉 | fix_pr | ⬜ |

## P0 — `FogVeil` / `HeatHaze` 环境音 loop id 被默认分支吞掉

- **#1 major（fix_pr）**：client `EnvironmentEffect` 明确把 `FogVeil` / `HeatHaze` 的专属 loop id 固化为 `mist_low_loop` / `cicada_summer_loop`（[client/src/main/java/com/bong/client/environment/EnvironmentEffect.java](/home/kiz/Code/Bong/.worktree/bughunt-loop-20260705-bm/client/src/main/java/com/bong/client/environment/EnvironmentEffect.java:216)、[client/src/main/java/com/bong/client/environment/EnvironmentEffect.java](/home/kiz/Code/Bong/.worktree/bughunt-loop-20260705-bm/client/src/main/java/com/bong/client/environment/EnvironmentEffect.java:354)），且归档 plan 已把这两条音景写为正式交付（[docs/finished_plans/plan-zone-environment-v1.md](/home/kiz/Code/Bong/.worktree/bughunt-loop-20260705-bm/docs/finished_plans/plan-zone-environment-v1.md:66)、[docs/finished_plans/plan-zone-environment-v1.md](/home/kiz/Code/Bong/.worktree/bughunt-loop-20260705-bm/docs/finished_plans/plan-zone-environment-v1.md:69)、[docs/finished_plans/plan-zone-environment-v1.md](/home/kiz/Code/Bong/.worktree/bughunt-loop-20260705-bm/docs/finished_plans/plan-zone-environment-v1.md:201)。
- **可达链路**：server `weather_to_environment_bundle()` 会在 `DroughtWind` 生成 `HeatHaze + FogVeil`，在 `HeavyHaze` / `LingMist` 生成 `FogVeil`（[server/src/world/weather_to_environment.rs](/home/kiz/Code/Bong/.worktree/bughunt-loop-20260705-bm/server/src/world/weather_to_environment.rs:39)、[server/src/world/weather_to_environment.rs](/home/kiz/Code/Bong/.worktree/bughunt-loop-20260705-bm/server/src/world/weather_to_environment.rs:71)、[server/src/world/weather_to_environment.rs](/home/kiz/Code/Bong/.worktree/bughunt-loop-20260705-bm/server/src/world/weather_to_environment.rs:84)）；即便不靠天气，`scorch_zone_effects` / `tribulation_zone_effects` / `tsy_zone_effects` 也会常驻产出 `FogVeil`（[server/src/world/environment.rs](/home/kiz/Code/Bong/.worktree/bughunt-loop-20260705-bm/server/src/world/environment.rs:343)、[server/src/world/environment.rs](/home/kiz/Code/Bong/.worktree/bughunt-loop-20260705-bm/server/src/world/environment.rs:372)、[server/src/world/environment.rs](/home/kiz/Code/Bong/.worktree/bughunt-loop-20260705-bm/server/src/world/environment.rs:389)）。这些 effect 经 `zone_environment_broadcast_system` 发到 client（[server/src/network/zone_environment_bridge.rs](/home/kiz/Code/Bong/.worktree/bughunt-loop-20260705-bm/server/src/network/zone_environment_bridge.rs:15)）。
- **根因链路**：client `EnvironmentAudioController.update()` 读取 `emitter.behavior().ambientLoopRecipe(emitter.effect())`，把 `recipeId` 直接交给 `startLoop()` / `recipe()`（[client/src/main/java/com/bong/client/environment/EnvironmentAudioController.java](/home/kiz/Code/Bong/.worktree/bughunt-loop-20260705-bm/client/src/main/java/com/bong/client/environment/EnvironmentAudioController.java:28)、[client/src/main/java/com/bong/client/environment/EnvironmentAudioController.java](/home/kiz/Code/Bong/.worktree/bughunt-loop-20260705-bm/client/src/main/java/com/bong/client/environment/EnvironmentAudioController.java:65)、[client/src/main/java/com/bong/client/environment/EnvironmentAudioController.java](/home/kiz/Code/Bong/.worktree/bughunt-loop-20260705-bm/client/src/main/java/com/bong/client/environment/EnvironmentAudioController.java:105)；但 `soundFor()` 只显式处理 `thunder_distant_loop` / `static_crackle_loop` / `wind_*`，对 `mist_low_loop` 与 `cicada_summer_loop` 没有 case，最终全部落入 `default -> "ambient.cave"`（[client/src/main/java/com/bong/client/environment/EnvironmentAudioController.java](/home/kiz/Code/Bong/.worktree/bughunt-loop-20260705-bm/client/src/main/java/com/bong/client/environment/EnvironmentAudioController.java:117)）。也就是说：**effect 层声明是分化的，真正播声层却把两条 recipe 压成同一个 cave 底噪**。
- **复现路径**：
  1. 进入任一带 `FogVeil` 的正常区域：scorch / TSY / tribulation zone，或让某 zone 进入 `HeavyHaze` / `LingMist`。
  2. client 收到 `bong:zone_environment` 后，本地 `EnvironmentEffectController` 驱动 `EnvironmentAudioController.update()` 拉起 loop。
  3. 预期应分别听到 `mist_low_loop` 的低雾感、`cicada_summer_loop` 的热浪蝉鸣感；实际两者都走 `minecraft:ambient.cave`，听感与 `soundFor()` 默认分支一致。
- **影响面**：
  - `FogVeil`：阴霾、血谷、灵雾、TSY、渡劫区、scorch 常驻雾层都受影响。
  - `HeatHaze`：`DroughtWind` / 夏季热浪 / scorch 热扭曲区受影响。
  - 因 `FogVeil` 是高频 effect，这不是边角 case，而是世界环境音常用分支的系统性塌缩。
- **这个 bug 对实际游玩体验的影响**：玩家进入雾区、热浪区、TSY、渡劫区时，本应靠环境音立刻分辨“潮湿低雾”“炽热蝉鸣”“洞穴空响”等不同空间语义；现在这些区域大量退化成同一条 `ambient.cave`，导致世界辨识度下降、天气/区域切换听感失真，远距离只靠耳朵判断风险与地貌氛围的能力被削弱。
- **修复建议**：
  - 在 `EnvironmentAudioController.soundFor()/volumeFor()/pitchFor()/priorityFor()` 补齐 `mist_low_loop` 与 `cicada_summer_loop` 的显式映射，避免 default fallback 吞掉合法 recipe_id。
  - 补 client 单测：`FogVeil` 起 loop 时 recipe/id/sound 必须是 `mist_low_loop`，`HeatHaze` 必须是 `cicada_summer_loop`；不允许回落到 `ambient.cave`。
  - 若团队想把本地环境音从“手写 switch”升级为“共享 server recipe 表”，可另立后续重构；本 bug 的最小修复仍是先把缺失分支接上。

## 反方裁决

> 当前会话未提供可用的 subagent / 委派工具；本轮按要求做**退化处理**：由主代理执行两轮反方裁决，并把反方论点与驳回理由显式记录如下。

1. **Round 1 反方论点**：`ambient.cave` 可能只是刻意的占位音，`mist_low_loop` / `cicada_summer_loop` 只是语义标签，不要求独立映射。
   **驳回理由**：`EnvironmentEffect` 已把这两个 id 明确暴露给运行时，且 `plan-zone-environment-v1` 把 FogVeil/HeatHaze 的 audio loop 作为已完成交付写死，没有 defer / TODO / placeholder 注记；同一文件里 `wind_*` / `static_crackle_loop` / `thunder_distant_loop` 都有显式分支，唯独这两个掉默认分支，形态上更像漏接线而非有意降级。
2. **Round 2 反方论点**：就算 `soundFor()` 默认成 `ambient.cave`，也许还有别的 client 路径会在本地重写 recipe，最终不一定真播错。
   **驳回理由**：`EnvironmentAudioController` 本身就是本地 `zone_environment` loop 的构造点，`recipe()` 直接 `new AudioRecipe(...)`，`SoundRecipePlayer.instance().play(...)` 立刻消费，仓内不存在第二条对 `mist_low_loop` / `cicada_summer_loop` 的本地播放重写链；并且已有文档明确说明客户端本地播放只能程序化 `new AudioRecipe(...)`，没有运行时 JSON loader（见 `plan-niche-craft-fix-v1` 对这条链的核验结论）。

## 审计来源

bughunt 定点轮（范围仅 `world audio / ambience / event sound`，显式避开 ambient zone audio stale anchor、fauna audio fade stop ignored、weather overlay collapse）。本结论为 **report-only**：只新增 skeleton，不改源码。候选经主代理代码追链、归档 plan 对照、两轮退化反方裁决后保留，判定为 **real-on-HEAD 的 client world-audio 错接线**。
