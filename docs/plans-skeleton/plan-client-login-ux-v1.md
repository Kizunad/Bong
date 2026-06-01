# Bong · plan-client-login-ux-v1 · 骨架

**连接/登录体验主题化**——将玩家从"点击进入服务器"到"站在世界中"的整段体验改写为末法残土风格：自定义"灵识共鸣"连接动画、资源包下载进度 HUD 覆盖（取代原版 "Downloading resource pack..."）、版本校验 SHA1 客户端缓存（避免每次连接重下），以及若资源包下载失败时的末法风格提示（而非原版错误框）。

**来源**：`docs/scribble.md` §"第三阶段：运维与分发——动态热更微端" → "客户端在加载界面通过 HTTP 自动下载最新的资源包" / "类似原神的热更新体验"

**补充说明**：scribble.md 原文提及"动态注入 Class 代码到 JVM"——该做法在 Minecraft Fabric 环境下存在 ClassLoader 隔离与安全限制，不纳入 v1 范围。v1 专注资源包侧的体验质量。

**交叉引用**：
- `plan-resourcepack-v1.md` ⬜ skeleton（**必须先完成**，本 plan 依赖其 `manifest.json` 格式 + SHA1 字段）
- `plan-client.md` ✅ — Fabric 微端基础设施（CustomPayload 框架、HudRenderLayer）
- `plan-audio-v1.md` ✅ — 连接动画音效（ambient.soul_speed_loop 等）
- `plan-vfx-v1.md` ✅ — BongSpriteParticle 粒子基础设施

**worldview 锚点**：无直接世界观锚（纯基础设施 plan）。主题包装：连接 = "灵识初触天道"；资源包下载 = "末法法则记忆注入意识海"；连接失败 = "灵海动荡，难以稳固神识"。

**qi_physics 锚点**：不涉及真元 / 灵气计算。

**前置依赖**：
- `plan-resourcepack-v1.md` ⬜ — 必须完成 P0/P1（`manifest.json` 格式 + `ResourcePackPrompt` 推送逻辑确定）才能开本 plan P0

---

## 接入面 Checklist

- **进料**：
  - `client/resourcepack/manifest.json`（`plan-resourcepack-v1` P0 产出的格式）：`{ version, sha1, url, ... }`
  - Fabric `ClientPlayNetworkingCallback`（监听 `ResourcePackSendPacket`）
  - Fabric `ScreenEvents` / `HudRenderCallback`（连接过程中的 HUD 覆盖）
- **出料**：
  - `LocalManifestCache.java`（`~/.bong/manifest_cache.json`）：存储上次已接受的 `{ sha1, version }`
  - `BongConnectScreen.java`：主题化连接/加载屏幕，替换连接阶段 HUD
  - `ResourcePackProgressOverlay.java`：资源包下载进度条 HUD，在 `DownloadingPacketPhase` 期间覆盖显示
  - `ResourcePackErrorScreen.java`：下载失败时的末法风格提示屏幕
- **共享类型**：无新 IPC schema（纯 client-side；server 只需保证 `manifest.json` endpoint 存在）
- **跨仓库契约**：
  - server 暴露 HTTP `GET /api/manifest.json`（host 与游戏服务器同 IP，独立 HTTP port，默认 25580）
  - client 在 `ServerConnectCallback` 时 fetch 该 endpoint（同步检查，200ms 超时后跳过校验继续连接）

---

## §0 SHA1 缓存逻辑

```
连接服务器时：
1. fetch GET http://<server-ip>:25580/api/manifest.json（200ms timeout）
2. 读 LocalManifestCache（~/.bong/manifest_cache.json）
3. if cache.sha1 == manifest.sha1 → 标记"已是最新"，Valence ResourcePackPrompt 来时静默接受，不重下
4. if 不同 or 无缓存 → 允许正常下载；下载成功后更新 cache
5. if fetch 超时 → 静默跳过（fallback 到原版 RP prompt 行为）
```

---

## 阶段总览

| 阶段 | 内容 | 状态 | 验收标准 |
|------|------|------|---------|
| **P0** | `LocalManifestCache`（SHA1 缓存）+ server `GET /api/manifest.json` HTTP endpoint | ⬜ | 单测：已缓存 SHA1 命中 → 不重下；SHA1 不同 → 触发下载；server HTTP endpoint 返回正确 JSON |
| **P1** | `BongConnectScreen`：主题化连接动画（灵识共鸣粒子 + 文案）替换原版连接弹窗 | ⬜ | 视觉验收：连接时出现主题动画；连接失败时出现末法风格错误屏幕（3 条 narration 文案轮显） |
| **P2** | `ResourcePackProgressOverlay`：下载进度 HUD 覆盖（进度条 + "法则记忆注入中" 文案 + 百分比） | ⬜ | 集成测试：触发 RP 下载 → 进度条从 0→100 正确更新；下载完成淡出；失败显示错误屏幕 |
| **P3** | 重试机制（下载失败最多重试 2 次，interval 3s）+ 降级策略（2 次失败后继续进入世界但标记"资源包缺失"）| ⬜ | 单测：mock 失败 → 重试逻辑 → 降级进入；降级状态下某些 VFX 静默降级为 vanilla 粒子 |

---

## §1 P0：SHA1 缓存 + server 端 HTTP endpoint

- [ ] `server/src/network/manifest_http.rs`：启动 `tokio::runtime` HTTP 服务（port 25580，与 game 端口隔离），`GET /api/manifest.json` 读取 `ResourcePackConfig` 返回 JSON；托管在 game server startup 时并行启动
- [ ] `client/.../LocalManifestCache.java`：`~/.bong/manifest_cache.json` 读写（Gson）；`checkAndCache(manifestUrl, sha1)` 返回 `CacheHit / CacheMiss / FetchTimeout`
- [ ] Fabric `ClientLoginNetworkHandler` mixin：在 `ResourcePackSendPacket` 处理前插入 `LocalManifestCache.checkAndCache`；CacheHit → 直接发 `ResourcePackStatusPacket(ACCEPTED)` 跳过实际下载
- [ ] ≥ 10 单测：CacheHit / CacheMiss / FetchTimeout 三路径 / SHA1 更新写入 / 并发访问不写坏缓存文件

---

## §2 P1：主题化连接屏幕

- [ ] `BongConnectScreen.java`（Fabric Screen）：在 `MultiplayerServerListScreen` 点击"连接"后、正式进入 `ConnectScreen` 前渲染；包含：
  - 背景：纯黑 `#0A0A0A` + 边缘粒子（`BongSpriteParticle`，颜色 `#3A2020`，数量 16，向内漂移）
  - 中心文案轮播（每 2s 切换）：
    1. "灵识触碰天道边界..."
    2. "残破的法则在涌动——感知正在校准..."
    3. "末法的世界等待你踏入。"
  - 连接状态文字（小字，底部，颜色 `#666666`）：显示"正在连接 <ip>" / "正在握手" / "正在加载世界"
- [ ] `ResourcePackErrorScreen.java`：下载失败时替换原版错误框，显示末法风格文案（3 条随机）：
  - "灵海动荡，法则碎片无法完整传输。（资源包下载失败）"
  - "天道的壁垒阻断了传承。请重试，或检查网络连接。"
  - "世界的记忆正在流失……稍后再试。"
- [ ] 音效（连接阶段 ambient）：`ambient.soul_speed_loop` pitch 0.5，volume 0.15，loop until 进入世界
- [ ] ≥ 6 单测 / 视觉测试（Minecraft test harness render screenshot 验证布局）

---

## §3 P2：资源包下载进度 HUD

- [ ] `ResourcePackProgressOverlay.java`（`HudRenderCallback`）：仅在 `DownloadingPacketPhase` 期间渲染：
  - 顶部横幅（颜色 `#1A0A0A` 半透明）：高度 28px，全宽
  - 进度条：颜色 `#C8A060`（金黄，象征灵气流动），宽度跟随 download progress 0→100%
  - 左侧文案："法则记忆注入中..."  右侧：百分比（例 "37%"）
  - 粒子特效：沿进度条右端持续 emit `BongLineParticle`，颜色 `#FFD070`，数量 2/tick，向右飞散，lifetime 20t
- [ ] 完成时：progress overlay fade out 20t，播放一声 `entity.player.levelup`（pitch 1.2，volume 0.5）
- [ ] ≥ 8 单测：进度 0→50→100 正确 / 完成后 overlay 消失 / 粒子 emit 频率

---

## §4 P3：重试 + 降级策略

- [ ] 重试：下载失败 → 显示"重试中（1/2）..." → 等 3s → 再试；2 次全失败 → 进入"降级模式"
- [ ] 降级模式：`ResourcePackMissingComponent`（Bevy Resource，client-side flag）：
  - VFX 系统检测到此 flag → 将 `bong:` 专属粒子贴图替换为最接近的 vanilla 粒子（smoke / portal / totem 等）
  - HUD 显示小图标 `⚠` 提示"资源包未加载"
- [ ] 降级恢复：玩家断开重连 + 成功下载资源包后，清除 `ResourcePackMissingComponent`，VFX 恢复
- [ ] ≥ 8 单测：重试次数 / 降级 flag 设置 / 恢复逻辑 / VFX fallback 映射表

---

## §8 开放问题（P0 决策门前需收口）

1. **server HTTP port**：25580 是否与现有端口冲突？需 Explore agent 检查 server 已用端口列表（game server 25565、可能的 Prometheus metrics port）
2. **Fabric mixin 注入点**：`ClientLoginNetworkHandler` 的 `ResourcePackSendPacket` 处理方法在 1.20.1 的具体方法签名，需 grep `client/` 或查 Fabric decompile 确认 mixin target
3. **SHA1 缓存文件位置**：`~/.bong/` 是否合理？Minecraft 通常把数据存 `.minecraft/`。需确认 Fabric 推荐的 config dir（`FabricLoader.getConfigDir()`）
4. **下载绕过是否安全**：CacheHit 时直接发 `ACCEPTED` 但不实际下载——如果服务器端 SHA1 与包内容不匹配（构建 bug），会导致玩家用旧包进入服务器。需要在 P0 决议中明确是否加 size 二次校验
