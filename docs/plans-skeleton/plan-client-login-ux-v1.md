# Bong · plan-client-login-ux-v1 · 骨架

**连接/登录体验主题化**——将玩家从"点击进入服务器"到"站在世界中"的整段体验改写为末法残土风格：自定义"灵识共鸣"连接动画、资源包下载进度 HUD 覆盖（取代原版 "Downloading resource pack..."）、版本校验 SHA1+size 客户端缓存（避免每次连接重下），以及若资源包下载失败时的末法风格提示（而非原版错误框）。

**来源**：`docs/scribble.md` §"第三阶段：运维与分发——动态热更微端"

**补充说明**：scribble.md 原文提及"HTTP manifest 直连 + JAR 注入"——均不纳入 v1：HTTP 通道违反跨层通信唯一约束（只走 CustomPayload / Redis IPC），JAR 注入存在 ClassLoader 安全限制。v1 专注资源包侧的体验质量，manifest 信息走 `bong:resourcepack_manifest` CustomPayload。

> ⚠️ **P0 阶段前置**：本 plan 依赖 `plan-resourcepack-v1.md` P0/P1 完成（`manifest.json` sha1+size 字段 + `ResourcePackPrompt` 推送逻辑确定）。`plan-resourcepack-v1` 必须合入后本 plan 方可生效开工。

**交叉引用**：
- `plan-resourcepack-v1.md` ⬜ skeleton（**必须先完成**，本 plan 依赖其 manifest 格式 + SHA1+size 字段 + P1 推送逻辑）
- `plan-ipc-schema-v1.md` ✅ — 新增 `ResourcePackManifestPayloadV1` TypeBox schema
- `plan-client.md` ✅ — Fabric 微端基础设施（CustomPayload 框架、HudRenderLayer）
- `plan-audio-v1.md` ✅ — 连接动画音效（ambient.soul_speed_loop 等）
- `plan-vfx-v1.md` ✅ — BongSpriteParticle 粒子基础设施

**worldview 锚点**：无直接世界观锚（纯基础设施 plan）。主题包装：连接 = "灵识初触天道"；资源包下载 = "末法法则记忆注入意识海"；连接失败 = "灵海动荡，难以稳固神识"。

**qi_physics 锚点**：不涉及真元 / 灵气计算。

**前置依赖**：
- `plan-resourcepack-v1.md` ⬜ — **必须**完成 P0/P1（`manifest.json` sha1+size 字段 + `ResourcePackPrompt` 推送逻辑确定）才能开本 plan P0

---

## 接入面 Checklist

- **进料**：
  - `client/resourcepack/manifest.json`（`plan-resourcepack-v1` P0 产出）：`{ version, sha1, size, url, ... }`
  - `bong:resourcepack_manifest` CustomPayload（server → client，login 完成后立即推送）：`ResourcePackManifestPayloadV1 { sha1: String, version: String, size: u64, required: bool }`
  - Fabric `ClientPlayNetworkingCallback`（监听 `bong:resourcepack_manifest` + `ResourcePackSendPacket`）
  - Fabric `ScreenEvents` / `HudRenderCallback`（连接过程中的 HUD 覆盖）
- **出料**：
  - `LocalManifestCache.java`（`FabricLoader.getInstance().getConfigDir().resolve("bong/manifest_cache.json")`）：存储上次已接受并成功加载的 `{ sha1, version, size, localPackPath }`
  - `BongConnectScreen.java`：主题化连接/加载屏幕，替换连接阶段 HUD
  - `ResourcePackProgressOverlay.java`：资源包下载进度条 HUD，在 `DownloadingPacketPhase` 期间覆盖显示
  - `ResourcePackErrorScreen.java`：下载失败时按失败类型区分的末法风格提示屏幕
- **共享类型**：新增 `ResourcePackManifestPayloadV1`（TypeBox schema + Rust serde，agent 不涉及）
- **跨仓库契约**：
  - server 在玩家 login 完成（`PlayerSpawnEvent` 后 1 tick）推送 `bong:resourcepack_manifest`；随后 Valence 继续推送原有 `ResourcePackPrompt`
  - client 接收 `bong:resourcepack_manifest` → 本地 SHA1+size 校验 → 决策是否提前接受
  - agent 不涉及此 plan

---

## §0 SHA1+size 缓存逻辑（CustomPayload，不走 HTTP）

```text
Login 完成后（server 侧 PlayerSpawnEvent + 1 tick）：
  server 推送 bong:resourcepack_manifest { sha1, version, size, required }

客户端收到 bong:resourcepack_manifest 后：
1. 读 LocalManifestCache（FabricLoader.getInstance().getConfigDir().resolve("bong/manifest_cache.json")）
2. CacheHit 条件（全部满足）：
   ① cache.sha1 == manifest.sha1
   ② cache.size == manifest.size（快速预筛，字节级匹配）
   ③ localPackPath 文件存在且可读（文件大小 > 0）
   ④ 实际文件大小 == manifest.size（OS 层二次确认）
   ⑤ DigestUtils.sha1Hex(localPackFile) == manifest.sha1（内容完整性）
   → 全部通过：CACHE_HIT（Valence ResourcePackPrompt 来时通过 mixin 静默发 ACCEPTED）
   注意：ACCEPTED 后 Minecraft 仍需完成本地加载流程，不能跳过
3. 任意条件不满足 → CACHE_MISS：走正常下载流程
4. bong:resourcepack_manifest 超时未收到（500ms）→ TIMEOUT：fallback 到原版 RP prompt
5. required=false 且失败 → 降级模式（允许进入世界）
   required=true 且失败 → 最多重试 2 次（3s interval）→ 全部失败后断开连接并显示错误屏幕

缓存写入：
- 下载成功并本地加载完成后，原子写入 cache 文件（写临时文件 → rename），
  使用 synchronized(LocalManifestCache.class) 防止并发写入 torn write
```

---

## 阶段总览

| 阶段 | 内容 | 状态 | 验收标准 |
|------|------|------|---------|
| **P0** | `ResourcePackManifestPayloadV1` schema + server 推送 + client SHA1+size 缓存校验 | ⬜ | 单测 ≥15：CacheHit（正常）/ CacheMiss-sha1不同 / CacheMiss-size不同 / CacheMiss-文件缺失 / CacheMiss-文件损坏（sha1验证失败）/ CacheMiss-localPackPath空 / TIMEOUT / 并发写不损坏（原子 rename）/ required=true-失败-断开 / required=false-失败-降级 / schema roundtrip / size字段校验 |
| **P1** | `BongConnectScreen`：主题化连接动画（灵识共鸣粒子 + 文案）| ⬜ | 视觉验收：连接时出现主题动画；连接失败时末法风格错误屏幕（按失败类型 3 条文案）；required=true 失败 → 断开 |
| **P2** | `ResourcePackProgressOverlay`：下载进度 HUD（进度条 + "法则记忆注入中" + 百分比）| ⬜ | 集成测试：触发 RP 下载 → 进度条 0→100 正确；完成淡出；失败显示对应失败类型错误屏幕 |
| **P3** | 重试机制（最多 2 次，3s interval）+ required-pack 策略 + 降级模式（含定时自动重试）| ⬜ | 单测：重试逻辑 / required=true 失败断开 / required=false 降级 / `BongClientRuntimeFlags.resourcePackMissing` 设置与恢复 / 定时重试（300s）成功后恢复 |

---

## §1 P0：ResourcePackManifestPayloadV1 + SHA1+size 缓存

### Schema 定义

```typescript
// agent/packages/schema/src/payloads/resourcepack.ts
const ResourcePackManifestPayloadV1 = Type.Object({
  sha1: Type.String({ pattern: '^[0-9a-f]{40}$' }),  // 40-char hex SHA1
  version: Type.String(),
  size: Type.Integer({ minimum: 0 }),  // 资源包字节大小，用于 CacheHit 预筛
  required: Type.Boolean(),
})
```

### 交付物

- [ ] TypeBox `ResourcePackManifestPayloadV1`（`agent/packages/schema/src/payloads/resourcepack.ts`）+ JSON sample（含 sha1 / version / size / required 字段）
- [ ] `server/src/network/resourcepack_manifest.rs`：`system_send_resourcepack_manifest`（PlayerSpawnEvent +1t → CustomPayload `bong:resourcepack_manifest`）；读取 `ResourcePackConfig { sha1, version, size, required }`
- [ ] `client/.../ResourcePackManifestHandler.java`：监听 `bong:resourcepack_manifest` CustomPayload → 触发 `LocalManifestCache.check(sha1, version, size)`；500ms 超时 fallback 到原版 RP prompt
- [ ] `client/.../LocalManifestCache.java`：
  - `CacheEntry { sha1, version, size, localPackPath }`（Gson 序列化）
  - `check(sha1, version, size)` → 5-step 校验：① entry 存在 ② sha1 一致 ③ size 一致（快速预筛）④ localPackPath 文件存在、可读、文件大小 == size ⑤ `DigestUtils.sha1Hex(localPackFile) == sha1`（内容完整性）→ 返回 `CacheHit / CacheMiss`
  - 写入：原子写（写临时 `.tmp` 文件 → `Files.move(ATOMIC_MOVE)`）+ `synchronized(LocalManifestCache.class)` 防并发
  - CacheHit 时通过 mixin 在 `ResourcePackSendPacket` handler 静默发 ACCEPTED（客户端仍走本地加载流程）
- [ ] ≥ 15 单测：
  - CacheHit（sha1 + size + 文件 + sha1验证全部通过）
  - CacheMiss-sha1不同（manifest sha1 与 cache 不同）
  - CacheMiss-size不同（manifest size 与 cache 不同）
  - CacheMiss-文件缺失（localPackPath 不存在）
  - CacheMiss-文件大小不匹配（文件存在但 size != manifest.size）
  - CacheMiss-文件损坏（sha1 验证失败，size 匹配但内容被篡改）
  - CacheMiss-localPackPath 空（entry 存在但路径为空字符串）
  - TIMEOUT（500ms 内未收到 CustomPayload → fallback）
  - 并发写不损坏（两线程同时 write，结果文件可正常 parse）
  - required=true + 所有重试失败 → 断开连接 + `ResourcePackErrorScreen` 显示
  - required=false + 所有重试失败 → 降级模式 + `BongClientRuntimeFlags.resourcePackMissing = true`
  - schema roundtrip（正样本 + sha1 格式非法 → 拒绝 + size < 0 → 拒绝）
  - sha1 格式非法（39位/非 hex）→ cache check 立即 MISS，不进入 sha1 计算
  - cache 文件不可读（权限不足）→ 静默 MISS，不抛异常
  - CacheHit 后 `ResourcePackSendPacket` mixin 静默 ACCEPTED → 不弹原版对话框

---

## §2 P1：主题化连接屏幕

- [ ] `BongConnectScreen.java`（Fabric Screen）：在 `MultiplayerServerListScreen` 点击"连接"后渲染；包含：
  - 背景：纯黑 `#0A0A0A` + 边缘粒子（`BongSpriteParticle`，颜色 `#3A2020`，数量 16，向内漂移）
  - 中心文案轮播（每 2s 切换）：
    1. "灵识触碰天道边界..."
    2. "残破的法则在涌动——感知正在校准..."
    3. "末法的世界等待你踏入。"
  - 连接状态文字（小字，底部，颜色 `#666666`）：显示"正在连接 &lt;ip&gt;" / "正在握手" / "正在加载世界"
- [ ] `ResourcePackErrorScreen.java`：下载失败 / required=true 失败时替换原版错误框，按**失败类型**显示不同文案：
  - 网络超时：`"灵海动荡，法则碎片无法完整传输。（连接超时）"`
  - 资源包校验失败：`"天道的壁垒阻断了传承。法则残片无法通过校验。"`
  - 服务端拒绝：`"世界的记忆正在流失……当前状态无法继续传输。"`
  - required=true 失败时额外显示："此次连接需要完整法则记忆，无法以残缺状态踏入。" 并断开连接
- [ ] 音效（连接阶段 ambient）：`ambient.soul_speed_loop` pitch 0.5，volume 0.15，loop 直至进入世界

---

## §3 P2：资源包下载进度 HUD

- [ ] `ResourcePackProgressOverlay.java`（`HudRenderCallback`）：仅在 `DownloadingPacketPhase` 期间渲染：
  - 顶部横幅（颜色 `#1A0A0A` 半透明）：高度 28px，全宽
  - 进度条：颜色 `#C8A060`（金黄），宽度跟随 download progress 0→100%
  - 左侧文案："法则记忆注入中..."  右侧：百分比（例 "37%"）
  - 粒子特效：沿进度条右端持续 emit `BongLineParticle`，颜色 `#FFD070`，数量 2/tick，向右飞散，lifetime 20t；数量上限 32 个同屏，超出时跳过 emit（防低帧率设备掉帧）
- [ ] 完成时：progress overlay fade out 20t，播放一声 `entity.player.levelup`（pitch 1.2，volume 0.5）
- [ ] 下载失败时：关闭 overlay → 根据失败类型（超时 / 校验失败 / 服务端拒绝）打开对应 `ResourcePackErrorScreen`

---

## §4 P3：重试 + required-pack 策略 + 降级

**Server required-pack 策略**（server 端在 `ResourcePackConfig` 中决定）：
- `required=false`（v1 默认）：下载失败 → 最多重试 2 次（3s interval）→ 全失败后进入降级模式
- `required=true`：下载失败 → 重试 2 次 → 全失败后发送 `ResourcePackDeclinedPacket` 并断开连接，显示 `ResourcePackErrorScreen`（required=true 专属文案）

**降级模式**（仅当 `required=false` 时）：
- [ ] `BongClientRuntimeFlags.resourcePackMissing`（Java 静态 boolean，**不是** Bevy Resource）：在 Fabric client Java 侧设置；VFX 渲染系统检测此 flag → 将 `bong:` 专属粒子贴图替换为最接近的 vanilla 粒子（smoke / portal / totem）
- [ ] HUD 显示小图标 `⚠` 提示"资源包未加载，视觉效果已降级"
- [ ] **降级自动重试**：降级模式下每 300s（5 分钟）自动尝试一次重新下载（不断开当前连接）；失败则维持降级，成功则清除 `BongClientRuntimeFlags.resourcePackMissing`，VFX 恢复，通知 HUD 撤销警告图标
- [ ] **降级恢复（手动）**：玩家断开重连 + 成功加载资源包后，清除 `BongClientRuntimeFlags.resourcePackMissing`
- [ ] ≥ 12 单测：
  - required=true 失败断开 + 专属错误文案
  - required=false 重试 2 次后降级（不断开）
  - 降级 flag 设置 → VFX fallback 映射表正确
  - 自动重试（300s）成功 → flag 清除 + VFX 恢复
  - 自动重试（300s）失败 → 维持降级，flag 不变
  - 手动重连成功 → flag 清除
  - VFX fallback 映射表（`bong:soul_spark` → vanilla `portal`，etc.）
  - 下载失败 manifest 非 200 状态（CustomPayload 侧标记失败）
  - sha1 格式非法（40 位但含非 hex 字符）→ 立即 CacheMiss，不进 sha1 计算
  - cache 文件不可读 → 静默 MISS
  - 并发重试（两次重试 timer 同时触发）→ 幂等，不发起两次下载

---

## §8 开放问题（P0 决策门前需收口）

1. **Fabric mixin 注入点**：`ClientLoginNetworkHandler` 的 `ResourcePackSendPacket` 处理方法在 MC 1.20.1 的具体方法签名，需 grep `client/` 或查 Fabric decompile 确认 mixin target；"静默接受"是否会绕过 Minecraft 本地资源包校验（需核查 MC 1.20.1 resource pack loading pipeline）
2. **`bong:resourcepack_manifest` 推送时机**：PlayerSpawnEvent +1t 是否会在 ResourcePackPrompt 之前到达 client？需确认 Valence 发包顺序（ResourcePackPrompt 发送时机在 plan-resourcepack-v1 §P1 中定义，本 plan 依赖其在 PlayerSpawnEvent 之后）
3. **required=true 失败后 server 主动断开**：client 发 ResourcePackDeclinedPacket 后 server 是否自动 kick，还是需要 server system 订阅 ResourcePackStatusEvent 并 kick？需 grep `server/src/network/` 确认 Valence 处理逻辑
4. **`manifest.size` 字段来源**：`plan-resourcepack-v1` 的 P0 产出 `manifest.json` 需包含 `size` 字段（字节大小）；本 plan 的 SHA1+size 双重缓存校验依赖此字段，需在 plan-resourcepack-v1 schema 中确认并对齐
