# Bong

AI-Native Xianxia (修仙) sandbox on Minecraft. Three-layer architecture:

- **server/** — Rust 无头 MC 服务器（Valence on Bevy 0.14 ECS，MC 1.20.1 协议 763）
- **client/** — Fabric 1.20.1 微端（Java 17，owo-lib UI）
- **agent/** — LLM "天道" agent 层（TypeScript，三 Agent 并发推演）
- **worldgen/** — Python 地形生成流水线（blueprint 驱动，terrain_gen 模块，LAYER_REGISTRY 统一 16 层地形）
- **library-web/** — 末法残土图书馆前端（Astro，静态站点）

## Quick commands

```bash
# Server
cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
cd server && cargo run              # 监听 :25565，offline mode

# Client
cd client && ./gradlew test build   # jar 在 build/libs/
cd client && ./gradlew runClient    # 通过 WSLg 启动 MC

# Agent（天道）
cd agent && npm run build                          # 编译 TS
cd agent/packages/tiandao && npm start             # 启动天道 Agent
cd agent/packages/tiandao && npm run start:mock    # mock 模式（无需真实 LLM）
cd agent/packages/tiandao && npm test              # 类型检查 + vitest

# Schema
cd agent/packages/schema && npm test

# Worldgen
cd worldgen && python -m scripts.terrain_gen       # 地形生成主流程
bash worldgen/pipeline.sh                          # 默认导出 raster + 预览

# Dev reload (regen + validate + rebuild + restart)
bash scripts/dev-reload.sh
bash scripts/dev-reload.sh --skip-regen            # rebuild only
bash scripts/dev-reload.sh --skip-validate         # 跳过 raster 校验

# Full smoke test
bash scripts/smoke-test.sh
```

## Key dependencies & versions

- Valence: git rev `2b705351`（pinned in Cargo.toml）
- big-brain `0.21`，bevy_transform `0.14.2`，pathfinding `4`
- Fabric: MC 1.20.1，Loader 0.16.10，owo-lib 0.11.2+1.20
- Schema: @sinclair/typebox 0.34
- Agent: openai ^4，ioredis ^5，tsx ^4，vitest ^3

## Architecture notes

- **Server ↔ Agent IPC**：Redis（`bong:world_state` 发布，`bong:agent_cmd` 订阅，`bong:player_chat` 队列）
- **IPC schema**：TypeBox（TS source of truth）→ JSON Schema export → Rust serde structs；共享 `agent/packages/schema/samples/*.json` 双端校验
- **天道 Agent**：三 Agent 并发推演（灾劫/变化/演绎时代），Arbiter 仲裁层负责合并与冲突消解
- **NPC AI**：big-brain Utility AI（Scorer → Action 模式），Position ↔ Transform 同步桥
- **Worldgen 流水线**：blueprint 定义固定坐标大地图 → terrain_gen 生成区域 field → stitcher 负责 zone→wilderness 过渡（按 LAYER_REGISTRY blend_mode）→ raster_export 导出 little-endian float32/uint8 二进制（mmap-friendly）→ Rust server 运行时按需生成 chunk
- **LAYER_REGISTRY**（`worldgen/scripts/terrain_gen/fields.py`）：16 层地形统一注册表，每层定义 `LayerSpec(safe_default, blend_mode, export_type)`；stitcher 和 raster_export 均从此派生配置
- **Dev harness**：`scripts/dev-reload.sh` 一键 regen+validate+rebuild+restart；`worldgen/scripts/terrain_gen/harness/raster_check.py` 做 raster 后验（rift_axis_sdf 默认值、height range、water depth）
- **Terrain profiles**：qingyun_peaks、spring_marsh、rift_valley/blood_valley、spawn、north_wastes、lingquan_marsh 均已完成
- `#[allow(dead_code)]` on `mod schema` in main.rs — schema 模块用于 IPC 对齐，尚未接入运行时

## Current milestone

**M1 — 天道闭环**（Agent 指令在游戏内可见）

| 层 | 状态 |
|----|------|
| Server | MVP 0.1 ✅（草地平台、玩家连接、僵尸 NPC、Redis IPC） |
| Agent | 骨架 ✅（三 Agent 并发、Context Assembler、Redis 订阅/发布） |
| Client | MVP 0.1 ✅（Fabric 微端、CustomPayload、HUD 渲染） |
| Schema | ✅ 双端对齐 |
| Worldgen | Phase A ✅，LAYER_REGISTRY refactor ✅，Phase B ✅（巨树/洞穴/水体/子表面/平滑/结构物/群系细化） |

验证标准：server + agent + client 联跑，玩家在游戏内行走，30 秒内聊天栏出现天道 narration，server 日志显示 agent command 被执行。

## Conventions

- 使用中文沟通
- 云端开发，拉到本地 WSL 测试
- `cargo run` 使用 offline mode（无需 Mojang 认证）
- Client 测试通过 `./gradlew runClient`（WSLg，无需单独启动器）
- Java 17 用于 Fabric，系统默认 Java 21（sdkman）
- docs/ 目录存放架构设计文档和路线图，修改前可参考
- Python 文件保存后自动 ruff 格式化（PostToolUse hook，见 `.claude/settings.local.json`）
