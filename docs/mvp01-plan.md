# MVP 0.1 Plan — Valence Server Scaffold + NPC 实验 + Fabric Client 最简端

## Context

基于 `docs/tech-audit.md` 完成的全量技术验证（编译通过、API 确认、版本对齐），现在进入代码落地阶段。MVP 0.1 的目标是**跑通三界分立架构的最小闭环**：Rust 服务端能站人、NPC 能动、Fabric 客户端能连接并接收自定义数据包。

用户会在云端编写代码，pull 到本地测试。本计划需要足够详细以支撑远程实现。

---

## 项目结构

```
Bong/
├── server/                     # Rust Valence 服务端
│   ├── Cargo.toml
│   ├── rust-toolchain.toml     # 锁定 Rust 版本
│   └── src/
│       ├── main.rs             # 入口：双 Runtime 启动
│       ├── world.rs            # 世界初始化（基岩平台）
│       ├── player.rs           # 玩家连接/断开处理
│       ├── npc/
│       │   ├── mod.rs          # NPC 插件注册
│       │   ├── spawn.rs        # NPC 生成
│       │   ├── brain.rs        # big-brain Scorer + Action
│       │   └── sync.rs         # Position <-> Transform 桥接
│       └── network/
│           ├── mod.rs           # 网络插件注册
│           └── agent_bridge.rs  # Tokio 线程 + crossbeam channel
├── client/                     # Fabric 1.20.1 微端
│   ├── build.gradle
│   ├── settings.gradle
│   ├── gradle.properties
│   └── src/main/
│       ├── java/com/bong/client/
│       │   ├── BongClient.java          # Mod 入口 (ClientModInitializer)
│       │   ├── BongNetworkHandler.java   # CustomPayload 监听
│       │   └── BongHud.java             # 简单 HUD 渲染验证
│       └── resources/
│           ├── fabric.mod.json
│           └── bong-client.mixins.json   # 留空，暂不需要 Mixin
└── docs/
```

---

## 模块一：Rust 服务端脚手架

### 1.1 Cargo.toml（版本已验证）

```toml
[package]
name = "bong-server"
version = "0.1.0"
edition = "2021"

[dependencies]
valence = { git = "https://github.com/valence-rs/valence" }
big-brain = "0.21"
bevy_transform = "0.14.2"
pathfinding = "4"
seldom_state = "0.11"

tokio = { version = "1", features = ["rt-multi-thread", "macros", "net"] }
crossbeam-channel = "0.5"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
tracing = "0.1"
tracing-subscriber = "0.3"

[profile.dev]
opt-level = 1          # 加速开发模式运行（Bevy 推荐）

[profile.dev.package."*"]
opt-level = 3          # 依赖库用 O3 编译
```

### 1.2 rust-toolchain.toml

```toml
[toolchain]
channel = "stable"
```

### 1.3 main.rs — 双 Runtime 入口

- 创建 `crossbeam_channel` 双向通道 (tx_to_game, rx_from_agent) + (tx_to_agent, rx_from_game)
- `std::thread::spawn` 启动 Tokio Runtime 守护线程（模拟 Agent 指令，每 10 秒发一条 mock 命令）
- `App::new()` 启动 Valence：
  - `.insert_resource(NetworkSettings { connection_mode: ConnectionMode::Offline, .. })`
  - `.add_plugins(DefaultPlugins)`
  - 注册 world、player、npc、network 模块的 systems

### 1.4 world.rs — 世界初始化

- `setup_world` system (Startup)：
  - 创建 `LayerBundle` (overworld)
  - 生成 16x16 区块的基岩平台 (Y=64, BlockState::BEDROCK)
  - 平台上铺一层草方块 (Y=65, BlockState::GRASS_BLOCK)

### 1.5 player.rs — 玩家处理

- `init_clients` system (Update)：
  - Query `Added<Client>`
  - 设置 GameMode::Adventure, Position [8.0, 66.0, 8.0]
  - 发送欢迎聊天消息
- `despawn_disconnected_clients` system (Update)

---

## 模块二：NPC 实验 (big-brain Utility AI)

### 2.1 sync.rs — Position <-> Transform 桥接

```rust
fn sync_position_to_transform(
    mut query: Query<(&Position, &mut Transform), Changed<Position>>,
) {
    for (pos, mut transform) in &mut query {
        transform.translation = pos.0.as_vec3();
    }
}

fn sync_transform_to_position(
    mut query: Query<(&Transform, &mut Position), (Changed<Transform>, With<NpcMarker>)>,
) {
    for (transform, mut pos) in &mut query {
        pos.0 = transform.translation.as_dvec3();
    }
}
```

双向同步：big-brain 修改 Transform → 写回 Position → Valence 协议广播给客户端。

### 2.2 spawn.rs — NPC 生成

- `NpcMarker` component（空标记）
- `NpcBlackboard` component：`nearest_player: Option<Entity>`, `player_distance: f32`
- `spawn_npc` system (Startup)：
  - 在平台中央生成一个僵尸外观实体 (EntityKind::Zombie)
  - 附加：`Position`, `Transform`, `GlobalTransform`, `NpcMarker`, `NpcBlackboard`
  - 附加 big-brain 的 `Thinker` bundle

### 2.3 brain.rs — 决策逻辑

**Scorer**：`PlayerProximityScorer`
- 读取 `NpcBlackboard.player_distance`
- 距离 < 8 格时输出高分

**Sensor system**：`update_npc_blackboard`
- 每 tick 遍历所有 NPC，找最近的玩家，更新 `NpcBlackboard`

**Action**：`FleeAction`
- 计算与玩家的反方向向量
- 每 tick 沿反方向移动 NPC 的 Transform（速度 0.15 blocks/tick）
- 当距离 > 16 格时 Action 完成

**Thinker 配置**：
```rust
Thinker::build()
    .picker(FirstToScore { threshold: 0.6 })
    .when(PlayerProximityScorer, FleeAction)
```

---

## 模块三：最简 Fabric Client

### 3.1 gradle.properties

```properties
minecraft_version=1.20.1
yarn_mappings=1.20.1+build.10
loader_version=0.16.10
fabric_version=0.92.3+1.20.1
owo_version=0.11.2+1.20
```

### 3.2 build.gradle（关键依赖）

```groovy
dependencies {
    minecraft "com.mojang:minecraft:${project.minecraft_version}"
    mappings "net.fabricmc:yarn:${project.yarn_mappings}:v2"
    modImplementation "net.fabricmc:fabric-loader:${project.loader_version}"
    modImplementation "net.fabricmc.fabric-api:fabric-api:${project.fabric_version}"
    modImplementation "io.wispforest:owo-lib:${project.owo_version}"
    annotationProcessor "io.wispforest:owo-lib:${project.owo_version}"
}
```

需要添加 wisp-forest Maven 仓库：`https://maven.wispforest.io/releases`

### 3.3 BongClient.java — Mod 入口

```java
public class BongClient implements ClientModInitializer {
    @Override
    public void onInitializeClient() {
        BongNetworkHandler.register();
    }
}
```

### 3.4 BongNetworkHandler.java — CustomPayload 监听

- 注册自定义 channel `bong:server_data`
- 使用 Fabric Networking API: `ClientPlayNetworking.registerGlobalReceiver`
- 收到包后解析 JSON，在聊天栏打印内容（MVP 验证通信）

### 3.5 BongHud.java — HUD 渲染（可选）

- 用 `HudRenderCallback` 在屏幕左上角渲染一行文字："Bong Client Connected"
- 验证客户端 mod 正常加载

### 3.6 fabric.mod.json

```json
{
  "schemaVersion": 1,
  "id": "bong-client",
  "version": "0.1.0",
  "name": "Bong Client",
  "environment": "client",
  "entrypoints": {
    "client": ["com.bong.client.BongClient"]
  },
  "depends": {
    "fabricloader": ">=0.16.0",
    "minecraft": "~1.20.1",
    "owo-lib": "*"
  }
}
```

---

## 验证流程

### 服务端验证

```bash
cd server && cargo run
```

预期输出：
1. `[天道网络层] Tokio 异步线程启动`
2. 服务器监听 25565 端口
3. 用 MC 1.20.1 客户端连接（离线模式），能看到草地平台
4. 平台上有一只僵尸，走近它 8 格内会逃跑

### 客户端验证

1. `cd client && ./gradlew build`
2. 将 jar 放入 `.minecraft/mods/`（需要 Fabric Loader + owo-lib）
3. 连接服务器，左上角显示 "Bong Client Connected"
4. 后续：服务端调用 `client.send_custom_payload(ident!("bong:server_data"), json.as_bytes())`，客户端聊天栏打印内容

### NPC 行为验证

- 僵尸在玩家 > 8 格时静止
- 玩家靠近 < 8 格时僵尸向反方向移动
- 玩家拉开距离 > 16 格后僵尸停止
- 服务端 TPS 稳定在 20

---

## 不做的事（MVP 0.1 范围外）

- 不接入真实 LLM / Redis
- 不做 Anvil 地形加载
- 不做热更新 Bootstrapper
- 不做 owo-ui XML 动态渲染
- 不做复杂战斗/真元系统
- 不做多 NPC / 寻路
