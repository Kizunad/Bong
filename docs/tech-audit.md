# 技术调研审计报告

> 基于 2026-04-05 对 scribble.md 中架构方案的实际验证。
> 修正了 Gemini 对话中基于训练数据推断的版本号和假设。

---

## 一、依赖版本真实状态

### Valence (Rust MC 服务端框架)

| 项目 | scribble.md 假设 | 真实情况 |
|------|-----------------|---------|
| Bevy 版本 | 0.13 | **0.14.2** (bevy_app, bevy_ecs, bevy_hierarchy, bevy_log, bevy_utils) |
| MC 协议版本 | 未明确 | **1.20.1** (协议号 763) |
| crates.io 最新 | 稳定发布 | **0.2.0-alpha.1** (2023-08-11，此后无发布) |
| 维护状态 | 活跃 | **半活跃**，最近提交 2026-01-15，间歇性开发 |
| valence_anvil | 存在 | **存在**，作为默认 feature |
| 核心 API | DefaultPlugins 等 | **确认存在**：DefaultPlugins, NetworkSettings, DimensionTypeRegistry, BiomeRegistry, Server, Client |

**必须使用 git 依赖**，crates.io 版本过旧：
```toml
valence = { git = "https://github.com/valence-rs/valence" }
```

### AI 生态库

| 库 | scribble.md 假设 | 匹配 Bevy 0.14 的正确版本 | 最新版本 |
|---|-----------------|----------------------|---------|
| big-brain | 0.19 (错误) | **0.21.x** | 0.22.0 (Bevy 0.15) |
| seldom_state | 0.11 | **0.11** (正确) | 0.16.0 (Bevy 0.18) |
| pathfinding | 4.6 | **4.15.0** (无 Bevy 依赖) | 4.15.0 |

### 修正后的兼容依赖组合

```toml
[dependencies]
# 法则层
valence = { git = "https://github.com/valence-rs/valence" }
# 不要显式引入 bevy 全量包，Valence 只用子 crate，避免 feature 冲突

# NPC AI
big-brain = "0.21"          # Utility AI, ^bevy 0.14.0
seldom_state = "0.11"       # 状态机, ^bevy 0.14
pathfinding = "4.15"        # 纯算法库，无 Bevy 依赖

# 桥接层（见下文 Position <-> Transform 问题）
bevy_transform = "0.14.2"

# 通信层
tokio = { version = "1", features = ["rt-multi-thread", "macros", "net"] }
crossbeam-channel = "0.5"
redis = { version = "0.25", features = ["tokio-comp"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# 观测
tracing = "0.1"
tracing-subscriber = "0.3"
```

---

## 二、Position <-> Transform 桥接问题

### 问题

Valence **故意不依赖 `bevy_transform`**，用自己的空间类型：
- `Position(DVec3)` — 双精度世界坐标
- `Look { yaw, pitch }` — 角度制（非四元数）

而 big-brain / seldom_state 等 Bevy 生态 AI 库依赖标准 `Transform` 做空间感知（距离计算等）。

**没有现成的桥接方案。** GitHub 零结果，Valence 社区无人提过此需求。

### 解决方案

给需要 AI 的实体（NPC）额外挂载 `Transform` + `GlobalTransform`，每 tick 同步：

```rust
fn sync_position_to_transform(
    mut query: Query<(&Position, &Look, &mut Transform), Changed<Position>>,
) {
    for (pos, look, mut transform) in &mut query {
        transform.translation = pos.0.as_vec3(); // DVec3 -> Vec3，丢失精度
        transform.rotation = Quat::from_euler(
            EulerRot::YXZ,
            look.yaw.to_radians(),
            look.pitch.to_radians(),
            0.0,
        );
    }
}
```

### 精度风险

`DVec3 (f64) -> Vec3 (f32)` 在远离原点时精度下降。MC 世界边界 ±30,000,000 格，`f32` 在该量级精度约 ±2 格。

**缓解**：对 AI 距离计算（"离目标多远"）而言误差可忽略；或直接绕过 Transform，在 Scorer 中基于 `Position` 手算距离。

### 替代路径

完全不用 big-brain 的 Scorer 空间计算，自己基于 Valence 的 `Position` 写：
```rust
fn distance_scorer(npc: &Position, target: &Position) -> f64 {
    npc.0.distance(target.0)
}
```
big-brain 的 Action 部分不需要 Transform，只有空间感知需要。

---

## 三、已识别的宏观风险

### 风险 1：Valence 项目存续 — 严重

- 半活跃开发，提交间隔数月
- crates.io 上自 2023-08 后无发布
- Issue #620 讨论过用 `evenio`（作者自研 ECS）替换 Bevy ECS，若发生则**所有 Bevy 生态库兼容性归零**
- MC 协议锁定在 1.20.1，无人推进更新

**影响**：如果 Valence 停更或 ECS 重写，整个技术栈的底座塌了。
**应对**：早期 fork，锁定 commit hash。做好自行维护协议层的准备。

### 风险 2：Bevy 0.14 生态窗口正在关闭 — 中等

- Bevy 当前已到 0.15+，生态库逐步停止支持 0.14
- big-brain 0.22 已经要求 Bevy 0.15，seldom_state 0.12+ 也是
- 留在 Bevy 0.14 意味着**被锁死在当前版本的 AI 库**，无法享受后续 bugfix

**影响**：依赖冻结，长期无法升级。
**应对**：接受冻结（MVP 阶段够用），或自行推动 Valence fork 升级到 Bevy 0.15。

### 风险 3：MC 1.20.1 客户端限制 — 中等

- 玩家必须使用 1.20.1 版本的 Minecraft 客户端
- Fabric 模组（owo-ui 等）需要确认有 1.20.1 版本
- 1.20.2+ 改了网络编解码层（Registry Codec 重构），Valence 不支持

**影响**：玩家需要降级客户端；部分新版 Fabric 模组不可用。
**应对**：对修仙沙盒来说 MC 版本不敏感（内容全是自定义的），1.20.1 生态足够成熟。

### 风险 4：Fabric 微端热更新的 JVM 安全性 — 中等

- scribble.md 中提出的 `URLClassLoader` 动态注入方案有安全隐患
- Java 9+ 的模块系统 (JPMS) 对动态类加载有更严格的限制
- Fabric 本身的 Mixin/ClassLoader 机制与自定义 ClassLoader 可能冲突

**影响**：热更新机制可能比预期复杂，或需要降级到"重启加载"模式。
**应对**：MVP 阶段不做热更新，先手动分发 mod jar。后续可参考 Velocity/BungeeCord 的插件加载机制。

### 风险 5：owo-ui 动态 XML 渲染能力 — 已验证，可行

- **MC 1.20.1 版本存在**：`io.wispforest:owo-lib:0.11.2+1.20`
- **运行时动态 XML 解析已确认**：`UIModel.load(InputStream)` 接受任意 InputStream，不依赖资源包
- Gemini 提到的 API 名称不完全准确（不是 `OwoUIAdapter.createFromXML`，而是 `UIModel.load()` + `model.createAdapter()`），但能力是真的

**可行的动态 UI 链路**：
```
Agent 生成 XML/JSON → Valence CustomPayload → Fabric 收包
→ ByteArrayInputStream → UIModel.load() → createAdapter() → 打开 Screen
```

**限制**：
- XML 只定义结构，事件回调必须在 Java 中用 `childById(id)` 绑定
- 自定义 XML 标签需要客户端预先注册 (`UIParsing.registerFactory()`)
- `DocumentBuilder` 未配置 XXE 防护，需自行加安全措施

**推荐设计模式**：客户端预置 UI 模板 + Agent 下发数据 JSON，仅在需要动态布局时才发完整 XML。

---

## 四、CustomPayload 协议详情（已验证）

### 服务端发包 API

```rust
// 便捷方法
client.send_custom_payload(ident!("mymod:channel"), json_bytes.as_bytes());

// 或直接构造包
client.write_packet(&CustomPayloadS2c {
    channel: ident!("mymod:xml_data").into(),
    data: Bounded(payload_bytes.as_slice().into()),
});
```

### 包结构 (`CustomPayloadS2c`)

```rust
pub struct CustomPayloadS2c<'a> {
    pub channel: Ident<Cow<'a, str>>,              // "namespace:path" 格式
    pub data: Bounded<RawBytes<'a>, 0x100000>,     // 最大 1 MiB
}
```

### 线路格式 (MC 1.20.1, Packet ID 0x17)

| 字段 | 类型 | 说明 |
|------|------|------|
| Packet ID | VarInt | `0x17` |
| Channel | VarInt-prefixed UTF-8 | 如 `"mymod:ui_data"` |
| Data | 裸字节（无长度前缀） | 消耗包内剩余所有字节 |

### 接收客户端包 (C2S)

Valence 自动解码为 Bevy Event：
```rust
fn handle_client_payload(mut events: EventReader<CustomPayloadEvent>) {
    for event in events.read() {
        // event.client: Entity
        // event.channel: Ident<String>
        // event.data: Box<[u8]>  (最大 32 KiB)
    }
}
```

---

## 五、Anvil/MCA 加载详情（已验证）

### WorldPainter 兼容性：可行

`valence_anvil` 读取标准 Anvil 格式 `.mca` 文件，WorldPainter 输出的就是这个格式。

**条件**：WorldPainter 导出时必须选择 **MC 1.18+ 格式**（post-1.18 的 `sections[]` 数组结构）。

### 支持项

| 内容 | 状态 |
|------|------|
| 方块状态 (Block States) | **完整支持**，含所有 Properties |
| 生物群系 (Biomes) | **完整支持**，4x4x4 分辨率 |
| 方块实体 (Block Entities) | **支持**（箱子、告示牌等） |
| 压缩格式 | Gzip / Zlib / 无压缩 / 外部 .mcc |
| 超大区块 | 支持 (.mcc 文件) |

### 不支持项

| 内容 | 说明 |
|------|------|
| 实体 (Entities) | **不加载**（生物、盔甲架等），需要 Valence ECS 自己生成 |
| 光照数据 | 忽略，客户端自行计算 |
| 高度图 | 忽略 |
| Tick 数据 | block_ticks / fluid_ticks 不解析 |
| pre-1.18 格式 | **不支持**，必须是 1.18+ 的区块格式 |

### 加载机制

- 专用 worker 线程异步加载，不阻塞游戏主循环
- 按玩家距离优先级排序（近处先加载）
- 通过 `ChunkLoadEvent` / `ChunkUnloadEvent` Bevy Event 通知
- 只读 — 修改后的区块不会回写磁盘（但底层 `RegionFolder` 有 `set_chunk()` API）

### 使用方式

```rust
// 在 LayerBundle 上附加 AnvilLevel 组件
let anvil_level = AnvilLevel::new(&world_path, &biomes);
commands.spawn((layer_bundle, anvil_level));
```

---

## 六、编译验证（已通过）

### 测试结果：通过

`cargo check` 在以下依赖组合下**编译成功**，348 个包，耗时 ~52 秒。

### 实际解析版本

| 依赖 | 解析版本 |
|------|---------|
| valence (git) | 0.2.0-alpha.1+mc.1.20.1 (commit `2b705351`) |
| big-brain | **0.21.1** |
| bevy | **0.14.2** (共享依赖，版本一致) |
| bevy_transform | **0.14.2** |
| seldom_state | **0.11.0** |
| pathfinding | **4.15.0** |

### 验证用 Cargo.toml

```toml
[dependencies]
valence = { git = "https://github.com/valence-rs/valence" }
big-brain = "0.21"
bevy_transform = "0.14.2"
pathfinding = "4"
seldom_state = "0.11"
```

---

## 七、MVP0 路线（修正版）

编译已验证通过，可以直接从 Step 1 开始。

### Step 1：基岩平台 + 玩家连接

跑通 Valence 官方 `building.rs` 级别的代码：
- 生成基岩平台
- 玩家连接、行走

### Step 2：引入 big-brain

- 加入 Position -> Transform 同步 system
- 实现最简 Utility AI：假人检测玩家距离 → 逃跑

### Step 3：CustomPayload 通信验证

- Valence 发送 JSON 到 Fabric 客户端
- 客户端解析并渲染（owo-ui 或简单的聊天消息）

### Step 4：Anvil 地形加载

- WorldPainter 制作小型测试地图（1.18+ 格式）
- Valence 通过 valence_anvil 加载

---

## 八、待验证清单

- [x] `cargo check` Valence git + big-brain 0.21 → **通过，Bevy 0.14.2 对齐**
- [x] owo-ui 1.20.1 Fabric 版本 → **有，0.11.2+1.20**
- [x] owo-ui 运行时动态 XML → **支持，`UIModel.load(InputStream)`**
- [x] CustomPayload 格式 → **`Client::send_custom_payload(channel, &[u8])`，S2C 最大 1 MiB**
- [x] Anvil 加载 WorldPainter MCA → **可行，需 1.18+ 格式导出，实体不加载**
