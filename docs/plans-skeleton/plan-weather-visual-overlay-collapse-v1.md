# plan-weather-visual-overlay-collapse-v1（骨架）

> **骨架（草案）**。一句话主题：server 端会把 zone 固有环境层与天气层一起下发给 client，但 `EnvironmentEffectRegistry` 用 `zoneId + stableKey()` 去重，而 `FogVeil` / `EmberDrift` / `AshFall` / `SnowDrift` 的 `stableKey()` 只看几何范围、不看 `tint` / `density` / `glow` / `wind`，导致**同一 zone 内本应叠加的多层天气/环境视觉被后一层直接覆盖**。影响是：像 `blood_valley_east_scorch` 这类本来应该同时呈现“焦土底色 + 雷暴天气”的区域，客户端最终只剩最后写入的一层，环境辨识度和天气层次一起塌缩。

> 立项动机：这条链路命中本轮限定范围的天气 / 季节 / 环境视觉主线，而且不是已知的 ambient audio、season stale client、tide sky omen、zone atmosphere mismatch 复题。问题位于 `server world environment -> client zone_environment -> client emitter registry` 的正式生产路径，且当前现网已有可达 zone（`server/zones.json` 中 3 个 scorch zone + `server/weather_profiles.json` 的雷暴增权）会稳定放大症状。

## 阶段总览

| 阶段 | 主题 | 路由 | 状态 |
|------|------|------|------|
| P0 | client 环境效果去重键误合并，导致天气/环境视觉叠层塌缩 | fix_pr | ⬜ |

## P0 — client 环境效果去重键误合并，导致天气/环境视觉叠层塌缩

- **现象**：server 侧 `sync_zone_environment_effects()` 会把 zone 固有环境效果与当前天气效果一起拼进同一个 `Vec<EnvironmentEffect>`（`server/src/world/environment.rs:321-338`）。例如 scorch zone 固有层自带 `AshFall + EmberDrift + FogVeil + LightningPillar`（`:343-369`），若当天气是 `Thunderstorm`，`weather_to_environment_bundle()` 又会再追加 `LightningPillar + EmberDrift + FogVeil`（`server/src/world/weather_to_environment.rs:20-38`）。但 client 侧 `EnvironmentEffectRegistry.onZoneStateUpdate()` 用 `activeKey(zoneId, effect)` 做唯一键（`client/src/main/java/com/bong/client/environment/EnvironmentEffectRegistry.java:48-79,129-130`），同 key 时直接 `active.refresh(...)` 覆盖旧 emitter（`:66-71`），不会保留两层效果。
- **根因链路**：`activeKey = zoneId + "@" + effect.stableKey()`；而 `FogVeil.stableKey()` 只包含 AABB（`client/.../EnvironmentEffect.java:195-223`），`EmberDrift.stableKey()` 只包含 AABB（`:283-311`），`AshFall.stableKey()` 只包含 AABB（`:145-172`），`SnowDrift.stableKey()` 只包含 AABB（`:384-414`），都**不包含**决定视觉差异的 `tintRgb` / `density` / `glow` / `wind_dir`。结果是：只要两个同 kind effect 共享同一几何范围，它们在 client 看来就是“同一个 emitter”，后一层刷新前一层，而不是并存。
- **可达链路**：`server/zones.json` 已有 `blood_valley_east_scorch` / `north_waste_east_scorch` / `drift_scorch_001` 三个 scorch zone；`server/weather_profiles.json` 给这三处都配了 `thunderstorm_multiplier: 5.0`。也就是说，正式游戏里这些 zone 一旦 roll 到雷暴，server 就会下发“焦土固有 FogVeil/EmberDrift + 雷暴 FogVeil/EmberDrift”的组合；client 收到 payload 后，在 registry 层就把同 AABB 的两层合并掉，玩家永远看不到作者想要的叠层结果。
- **复现路径**：
  1. 进入任一 scorch zone，例如 `blood_valley_east_scorch`。
  2. 等待或用现有天气调试路径让该 zone 进入 `Thunderstorm`。
  3. server 会生成两组同 AABB 的 `FogVeil` / `EmberDrift`：一组来自 scorch 固有层（焦土棕红 / glow 0.65 / density 0.34），一组来自 thunderstorm 天气层（灰雷云 / glow 0.5 / density 0.4）。
  4. client 侧 `EnvironmentEffectRegistry` 遍历 state 时先创建固有 emitter，再在第二个同 key effect 上调用 `refresh()` 覆盖，于是最终只剩天气层；焦土底色与固有 ember 表现被静默吞掉。
- **为什么这是 bug，不是设计**：server 明确把 base zone effect 与 weather effect 作为**两个独立 effect** 同时下发，说明设计意图是“可叠加”，不是“天气层独占”。如果产品意图只是单层替换，server 根本不需要在 `default_effects_for_zone_with_profile()` 里把两边都 `extend()` 到同一个列表。现在的合并发生在 client registry 的 key 设计上，而且不仅影响 `FogVeil`，还会影响 `EmberDrift` / `AshFall` 这类其他环境层，属于消费端把合法状态错误压扁。
- **这个 bug 对实际游玩体验的影响**：玩家进入特殊地貌区时，本来应该同时感到“这是焦土/劫区”以及“现在天气在变”，现在却经常只剩最后一层效果。最直接的体感是：焦土雷暴时，区域招牌性的焦土雾色、灰烬/火星底噪会被天气层吃掉，场景看上去像普通雷暴而不是“焦土上的雷暴”；后续如果别的 zone 叠 `HeavyHaze` / `LingMist` / `SnowDrift` 也会继续发生同型塌缩，玩家会误以为天气系统或 zone 视觉没有差异，环境识别与远距离读图能力一起下降。
- **建议修复范围 / 模块**：优先收口 `client/src/main/java/com/bong/client/environment/EnvironmentEffect.java` 与 `EnvironmentEffectRegistry.java`。两条可选修法都需要一次性定清：
  1. **语义键修法**：把 `stableKey()` 扩成“同 kind + 同几何 + 同视觉参数”键，例如 `FogVeil` 带上 `tintRgb + density`，`EmberDrift` 带上 `density + glow`，`AshFall` 带上 `density`，`SnowDrift` 带上 `density + windDir`。
  2. **实例键修法**：不要用 effect 语义做唯一键，改成 `zone + payload index` 或显式 server-side effect id，让“同一 payload 里的多层 effect”天然并存，跨 generation 再做逐项 refresh。
  无论选哪条，都要保证“同一 zone 的多层同 kind effect”不会再被 client 静默压成 1 层。
- **验收抓手**：至少补 4 组 pin。1) 构造一个 `ZoneEnvironmentState`，含两个同 AABB 但不同 tint/density 的 `FogVeil`，client registry 更新后必须保留 2 个 active emitter。2) 同样验证 `EmberDrift(glow/density)` 与 `AshFall(density)` 不再互相覆盖。3) scorch zone + thunderstorm 端到端回归，client 最终同时存在 1 个 scorch fog + 1 个 storm fog、1 个 scorch ember + 1 个 storm ember。4) 旧的“同一逻辑 emitter 跨 generation refresh”行为仍成立，避免修 key 后引入无穷增殖或 fade 泄漏。

## 反方裁决摘要

> 当前会话**没有可用 subagent / delegate tool**；按要求如实记录退化处理。本次两轮反方裁决均由主代理手工执行，先假设“这可能只是预期覆盖”，再逐条用代码证据驳回。

1. **Round 1 反方论点**：`stableKey()` 故意不带强度参数，可能是为了让同一 effect 在 generation 变化时平滑 refresh；因此“后来的 effect 覆盖前面的 effect”也许是预期。
   **驳回理由**：平滑 refresh 只对“同一个逻辑 emitter 跨帧更新”成立；这里是**同一 payload 内并列出现的两个 effect**，而 server 已显式把它们分成两条 `EnvironmentEffect`。client 若在单帧内把第二条覆盖第一条，等于消费端否决了 server 的多层表达能力，不是 refresh。
2. **Round 2 反方论点**：也许视觉上本来就只允许每个 zone 存在一层 fog / 一层 ember，server 发两层只是历史遗留，真正问题在 server 不该叠加。
   **驳回理由**：这条解释无法覆盖 `EmberDrift` / `AshFall` 等多个 kind 一起受影响的事实，也无法解释 `default_effects_for_zone_with_profile()` 为什么在正式代码里稳定 `extend(base)` 再 `extend(weather)`。如果产品意图是“天气完全替换 zone 基底”，正确做法应是 server 端在构造 `effects` 时合成/替换，而不是让 client 通过几何 key 偶然吞掉前一层。当前实现是**未声明的隐式覆盖**，而且覆盖条件依赖 stableKey 是否遗漏参数，属于典型 bug 而非设计。

## 开放问题

1. 修复时更推荐“把视觉参数纳入 `stableKey()`”还是“改成 server 下发显式 effect instance id”？前者改动小，后者语义更稳，但需要双端契约扩展。
2. `LightningPillar.stableKey()` 当前也不带 `strikeRatePerMin`（`client/.../EnvironmentEffect.java:95-130`）；虽然现有 scorch/tribulation 与 thunderstorm 半径不同，不会立刻撞 key，但是否顺手一起补齐，避免后续 profile 改半径后再次踩坑。
3. `EnvironmentParticleHelper` 用 `effect.stableKey().hashCode()` 做粒子随机种子（`client/.../EnvironmentParticleHelper.java:18-20`）；若改 key 语义，需要一并确认粒子分布不会因“同层不同参数”仍共用随机轨迹而继续显得像单层。

## 审计来源

bughunt 定点轮（范围限定为 `server/src/world` / `server/src/lingtian/weather*` / `client/src/main/java/com/bong/client/environment` / `client/src/main/java/com/bong/client/atmosphere` 的天气/环境视觉链路）。当前结论是 **report-only**：只提交 skeleton，把复现路径、根因链路、影响面、修复建议与两轮反方裁决固化，后续由独立 fix PR 落地。
