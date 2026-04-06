# 本地测试环境搭建指南

> MVP 0.1 本地验证所需的完整环境。云端写代码，本地 pull 下来编译 + 测试 + MC 联机。

---

## 一、前置条件检查

| 工具 | 要求 | 本地现状 |
|------|------|----------|
| Rust | stable | 1.89.0 ✅ |
| Java | 17 (Fabric 编译) | 21 ⚠️ 需装 17 |
| Java | 21 (MC 客户端运行) | 21 ✅ |
| MC 客户端 | 1.20.1 + Fabric Loader 0.16.10 | ❌ 需安装 |
| sdkman | 管理多 Java 版本 | ✅ 已有 |

> **注意**: Fabric mod 编译要求 Java 17 (`sourceCompatibility = JavaVersion.VERSION_17`)。
> MC 1.20.1 客户端本身可以用 Java 17 或 21 运行。

---

## 二、Java 17 安装（sdkman）

```bash
# 安装 Java 17（用于 Fabric client 编译）
sdk install java 17.0.18-amzn

# 不设为默认，保留 21 为系统默认
# 编译 client 时临时切换：
cd ~/Code/Bong/client
sdk use java 17.0.18-amzn
./gradlew test build
```

---

## 三��MC 1.20.1 开发客户端

### 推荐：`./gradlew runClient`（零安装）

WSLg 已确认可用（DISPLAY=:0, Wayland + PulseAudio），Fabric Loom 内置开发客户端启动：

```bash
cd ~/Code/Bong/client
sdk use java 17.0.18-amzn
./gradlew runClient
```

Loom 自动处理：MC 1.20.1 assets 下载、Fabric Loader、mod 注入、离线模式。
无需安装任何 MC 启动器，无需 Mojang 账号。

> 首次 `runClient` 会下载 ~500MB MC assets，后续秒开。

### 备选：Prism Launcher（独立测试 release jar）

仅当需要测试打包后的 jar（而非开发态）时使用：
1. Windows 端安装 [Prism Launcher](https://prismlauncher.org/download/windows/)
2. 创建 1.20.1 + Fabric 0.16.10 实例
3. mods 目录放入 Fabric API 0.92.3+1.20.1、owo-lib 0.11.2+1.20、`client/build/libs/bong-client-*.jar`

---

## 四、Rust Server 本地编译 & 启动

```bash
cd ~/Code/Bong/server

# 编译（首次约 1-2 分钟）
cargo build

# 启动服务端（监听 25565）
cargo run

# 预期日志输出：
# [bong][bridge] tokio runtime started
# [bong][world] creating overworld test area (16x16 chunks)
# [bong][player] registering player init/cleanup systems
# [bong][npc] registering spawn/sync/brain systems
```

---

## 五、Fabric Client 本地编译

```bash
cd ~/Code/Bong/client

# 切换 Java 17
sdk use java 17.0.18-amzn

# 编译 + 测试
./gradlew test build

# 产物位置
ls build/libs/*.jar
# → bong-client-0.1.0.jar (或类似名称)
```

### 开发态直接启动（无需部署）

```bash
# 编译 + 直接启动带 mod 的 MC 客户端
cd ~/Code/Bong/client
sdk use java 17.0.18-amzn
./gradlew runClient
# MC 窗口弹出后 → 多人游戏 → localhost:25565
```

---

## 六、联机验证流程

```
1. WSL 终端 A — 启动 Rust 服务端:
   cd ~/Code/Bong/server && cargo run

2. WSL 终端 B �� 启动开发客户端:
   cd ~/Code/Bong/client && sdk use java 17.0.18-amzn && ./gradlew runClient

3. MC 窗口 → 多人游戏 → 添加服务器 → 地址: localhost:25565

4. 验证清单:
   [ ] 能成功连接到服务端
   [ ] 出生在草地平台上 (坐标约 8, 66, 8)
   [ ] 看到一个 zombie NPC (坐标约 14, 66, 14)
   [ ] 聊天栏收到 "Welcome to Bong!"
   [ ] 靠近 NPC (<8格) 时 NPC 逃跑 (Task 7 完成后)
   [ ] 左上角显示 HUD "Bong Client Connected" (Task 9 完成后)
   [ ] 聊天栏收到 bong:server_data 内容 (Task 8+9 完成后)
```

---

## 七、快速验证脚本

一键跑完 fmt/clippy/test/smoke run/client build：

```bash
bash scripts/smoke-test.sh
```

脚本涵盖 4 个阶段：Rust 格式+lint、Rust 测试、服务端 15s 冒烟、Fabric client 构建。

---

## 八、注意事项

- **WSL ↔ Windows 网络**: WSL2 的 `localhost` 默认映射到 Windows，MC 客户端连 `localhost:25565` 即可
- **防火墙**: 如果连不上，检查 Windows 防火墙是否放行 25565
- **Java 版本切换**: `sdk use` 只影响当前终端，不会改变全局默认
- **ConnectionMode::Offline**: 服务端不验证正版，Prism Launcher 离线模式可直接连
- **首次 cargo build**: 需要下载 ~348 个 crate，确保网络通畅
