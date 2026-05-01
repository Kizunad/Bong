# Bong · plan-particle-system-v1 · 模板

**粒子与世界内 VFX 系统专项**。定义 Bong 的粒子/实体/BlockEntity 渲染基类、Server↔Client VFX 触发协议、以及首批美术资产清单。

**交叉引用**：`plan-vfx-v1.md`（VFX 基础栈总纲）· `../plan-HUD-v1.md` · `plan-narrative-v1.md` · `plan-tribulation-v1.md` · `../plan-combat-no_ui.md` / `../plan-combat-ui_impl.md`。

**范围**：本 plan 只管**世界内**的 VFX（B 层：粒子 / 实体 / BE）。屏幕级滤镜（血月、顿悟白屏、水墨）归 `plan-vfx-v1.md §3/§4`。

---

## §0 设计轴心

- [x] 粒子必须**避开原版 "*" 感**：自定义 `buildGeometry`、非 billboard、贴图禁用点/星形
- [x] **Server 权威触发**：VFX 事件由服务端发起（技能释放、境界突破、NPC 反应），客户端只负责播放
- [x] **客户端纯表演**：无状态广播、无物理影响（粒子/实体不做判定，判定在 server）
- [x] 所有 VFX 对 Iris 光影零冲突（走标准 `RenderLayer`，不碰 shader program）
- [x] 分类基类复用：`LineParticle` / `RibbonParticle` / `GroundDecalParticle` / 默认 billboard

---

## §1 渲染基类架构（`client/visual/particle/`）

### 1.1 `BongLineParticle`

拉伸 quad：沿速度方向长、垂直方向窄。用于**剑气、刀罡、掌风线条、暗器轨迹**。

- [x] 重写 `buildGeometry`：取 velocity，构造一个朝速度方向拉长的四边形
- [x] 支持可配置"长度 = 速度 × factor"
- [x] UV 沿长度方向（支持贴图流动动画）
- [x] 发光层走 `RenderLayer.getEntityTranslucentEmissive`（`BongParticleSheets.LINE_EMISSIVE`）

### 1.2 `BongRibbonParticle`

记录过去 N 帧位置，拼接带状几何。用于**飞剑拖尾、雷电、丝带法宝**。

- [x] 内部维护位置环形缓冲（默认 16 帧）
- [x] 每帧构造 N-1 个连续四边形（ribbon 段）
- [x] UV 沿 ribbon 长度流动
- [x] 头尾 alpha 渐隐
- [x] 开放"ribbon 宽度随生命周期变化"回调

### 1.3 `BongGroundDecalParticle`

贴地固定朝向，法线朝上。用于**脚下符圈、血迹、脚印、结界投影**。

- [x] 重写 billboard 逻辑：四边形法线锁定为 `(0, 1, 0)`
- [x] 贴图 UV 可旋转（符阵自转）
- [x] 高度微抬（避免 z-fighting）
- [ ] 支持地形贴合（按下方方块 bounding box 微调 Y）

### 1.4 `BongModelParticle`（可选，后期）

用 `ModelPart` 或自定义 mesh 代替 quad。用于**花瓣、硬币、符牌、卦象立体字**。

- [ ] 加载模型资源
- [ ] 粒子生命周期控制旋转/缩放
- [ ] 仅在需要时实现，优先用贴图方案

### 1.5 默认 `SpriteBillboardParticle` 扩展

原版基类，但**贴图资源规范**严格：

- [x] 禁用任何点/星/方块贴图
- [x] 规范：符文（汉字、卦象）、花瓣、墨迹、光球（带 alpha 光晕）、剑形剪影
- [x] 尺寸：32×32 / 64×64，必须带 alpha

---

## §2 Server → Client VFX 触发协议

### 2.1 为什么要协议

VFX 的触发来源是**服务端的游戏逻辑**：
- 技能释放：server 判定 → 通知 client 播"剑气"粒子
- 境界突破：server 状态变更 → 通知 client 播"破境光柱"
- NPC 死亡：server NPC 逻辑 → 通知 client 播"魂散"粒子
- 天劫降临：天道 Agent → server → client 画面

客户端**不应自己决定"什么时候该播 VFX"**（否则无法联网、不一致）。

### 2.2 协议分流：两条通道（基于 Pumpkin/Valence 调研，2026-04-13）

**不要只走一条通道**。根据效果类型分流：

| 触发 | 通道 | 能力 | 何时用 |
|------|------|------|--------|
| **vanilla 粒子** | Valence `ChunkLayer::play_particle` | 自动 chunk viewer 过滤、自带 `particle_count` 合批 | 能用原版粒子类型表达的（烟雾、火花、末地烛、sweep_attack、end_rod 等） |
| **Bong 自定义粒子** | `bong:vfx_event` CustomPayload（JSON） | 任意 payload、Fabric 客户端走 `BongParticle` 注册表 | 独有的：剑气 ribbon、汉字符文、水墨、飞剑拖尾 |

**为什么分流**：
- vanilla `CParticle` 包的 `particle_id` 必须在 vanilla 注册表里，协议层硬限制（Pumpkin `pumpkin-data/src/generated/particle.rs` codegen，Valence 同理）—— **自定义粒子类型不能走 vanilla 包**
- 80% 的氛围效果用 vanilla 粒子够用，省掉协议工作
- Valence `ChunkLayer::play_particle` 已做好 viewer 过滤，不需要我们在 §2.4 自己实现距离过滤

**自定义粒子事件格式**：
```json
{
  "type": "vfx_event",
  "event": "sword_qi_slash",
  "origin": [x, y, z],
  "direction": [dx, dy, dz],
  "params": { "color": "#88ccff", "intensity": 0.8 },
  "duration_ticks": 20
}
```

- [x] 定义 `VfxEvent` TypeBox schema（`agent/packages/schema/src/vfx.ts`）
- [x] 导出 JSON Schema → Rust serde struct
- [x] 客户端 `BongNetworkHandler` 分发到 `VfxDispatcher`
- [x] `VfxDispatcher` 按 `event` 字符串查注册表 → 调用对应 `VfxPlayer.play(params)`

### 2.3 服务端 VFX 分发器架构（不学 Pumpkin）

**反例**：Pumpkin 把 `World::spawn_particle` 当主入口，每次调用即时发包，无节流、无合批、全服广播。战斗密集场景会炸带宽。

**Bong 的做法**：Bevy 系统不直接发包，而是写入 queue，由分发器每 tick flush：

```rust
// 业务代码（Bevy system）
events.send(VfxEvent::SwordQi { origin, direction, color });

// VFX 分发器（每 tick 一次）
fn flush_vfx_queue(
    mut events: EventReader<VfxEvent>,
    mut layers: Query<&mut ChunkLayer>,
    clients: Query<&mut Client, Near>,
) {
    // 1. 合批同类型同位置的事件（利用 particle_count 字段）
    // 2. vanilla 粒子 → ChunkLayer::play_particle（自动 viewer 过滤）
    // 3. 自定义粒子 → CustomPayload 发给附近玩家
}
```

好处：
- 合批：同帧 10 次剑气 → 合并成 1 个 `particle_count=10` 的包
- 统一节流点：可以加速率限制、优先级丢弃
- 测试可插桩：queue 是 Bevy resource，易 mock

### 2.4 客户端自演 vs 服务端广播（关键原则）

参考 Pumpkin `EndermanEntity::mob_tick` 的注释智慧（"从服务端发会造成巨大网络开销"）：

| 粒子类型 | 归属 |
|---------|------|
| **持续状态类**（运功真气环绕、入魔黑雾、中毒酸气、境界光环） | **客户端自演** —— 读 `server_data.player_state` 状态位自己生成，**不广播** |
| **一次性事件类**（剑气斩击、破境光柱、天劫落雷、死亡魂散） | 服务端广播（§2.2 两条通道择一） |
| **环境氛围类**（灵脉微光、阴煞雾气、晨露蒸腾） | **客户端自演** —— 读区域状态 / 方块元数据自行生成 |

凡是"每 tick 都要有的"都客户端自演。凡是"一次性触发"才广播。

### 2.5 合批与节流规则

- [x] 同一 tick 内同 `event id` 且 origin 距离 <1m 的事件 → 合并为 `count=N`
- [x] 单 chunk 内每 tick VFX 事件上限（默认 8 个，超出按优先级丢弃）
- [x] 每玩家每 tick 收到的 VFX 包上限（默认 32 个）
- [x] 优先级：突破/天劫 > 玩家技能 > NPC 技能 > 环境

### 2.6 范围控制

- [ ] vanilla 粒子：`ChunkLayer::play_particle` 自动按 chunk viewer 过滤（默认 view distance）
- [x] 自定义 `vfx_event`：服务端按距离过滤，默认 64 格；大型事件（天劫、破境）整服广播
- [ ] 实现参考 Valence `view_writer(position)` 模式

### 2.7 事件 → 播放器注册表（客户端）

```java
VfxRegistry.register("sword_qi_slash", SwordQiSlashPlayer::new);
VfxRegistry.register("breakthrough_pillar", BreakthroughPillarPlayer::new);
// ...
```

每个 `VfxPlayer` 封装一次特效的完整编排（粒子 + 实体生成 + 声音 + HUD 叠色）。

### 2.8 确定性 vs 随机性

- [x] **参数由 server 决定**（颜色、强度、方向）
- [x] **细节由 client 自由**（单个粒子的抖动、随机旋转）
- [x] 不保证严格跨客户端视觉一致（没必要），只保证"语义一致"

---

## §3 Entity / BlockEntity 集成

### 3.1 自定义实体（飞剑、法宝、投掷物）

- [ ] Entity 本身在 server 注册（有碰撞/物理/寿命）
- [ ] Renderer 客户端单独注册（`EntityRendererFactories`）
- [ ] 跟随 entity tracking 自动同步位置（走 vanilla entity packet）
- [ ] **不占用 §2 的 vfx_event 通道**（entity 机制已处理）

### 3.2 BlockEntity（符阵、结界、灵脉节点）

- [ ] Server 侧 BlockEntity 携带状态（激活与否、强度、朝向）
- [ ] BlockEntityRenderer 读状态渲染
- [ ] 状态变更走 BlockEntity sync packet（vanilla 机制）
- [ ] **也不占用 vfx_event 通道**

### 3.3 短生命 VFX vs 持久实体的取舍

| 效果 | 用哪种 |
|------|--------|
| 一次性攻击光效 | 粒子（vfx_event） |
| 持续 1-5 秒的短暂发光物 | 粒子 ribbon |
| 5+ 秒的可见物 | Entity（有位置/寿命） |
| 固定位置的结构物 | BlockEntity |

---

## §4 首批资产与事件清单（Phase 1 备料）

### 4.1 粒子类型注册

| id | 基类 | 用途 | 触发源 |
|----|------|------|--------|
| `bong:sword_qi_trail` | Line | 剑气拖尾 | 剑修攻击 |
| `bong:sword_slash_arc` | Line | 剑气斩击弧 | 剑招释放 |
| `bong:qi_aura` | Sprite | 真气环绕 | 玩家/NPC 运功状态位 |
| `bong:rune_char` | Sprite | 符文字符飘浮 | 符箓激活 |
| `bong:lingqi_ripple` | GroundDecal | 灵压涟漪 | 破境、威压 |
| `bong:breakthrough_pillar` | Line | 破境光柱 | 境界突破 |
| `bong:enlightenment_dust` | Sprite | 顿悟星屑 | 顿悟事件 |
| `bong:tribulation_spark` | Line | 天劫电弧 | 天劫阶段 |
| `bong:flying_sword_trail` | Ribbon | 飞剑拖尾 | 飞剑实体携带 |

### 4.2 实体（独立注册，`../plan-combat-ui_impl.md` 等处定义细节）

- [ ] 飞剑 `flying_sword`
- [ ] 符箓（飞行）`talisman`

### 4.3 BlockEntity

- [ ] 符阵中心 `formation_core`
- [ ] 结界节点 `barrier_node`

### 4.4 Server 端事件清单（首批）

| event id | 触发时机 | 参数 |
|----------|---------|------|
| `sword_qi_slash` | 剑修普通攻击命中 | origin, direction, color |
| `breakthrough_pillar` | 境界突破完成 | origin, realm_level |
| `enlightenment_aura` | 顿悟开始/结束 | player_id, phase, intensity |
| `tribulation_lightning` | 天劫落雷 | origin, strength |
| `formation_activate` | 符阵激活 | bpos, formation_type |
| `death_soul_dissipate` | 实体死亡 | origin, entity_type |

---

## §5 实施节点

### 5.1 Phase 0 — 服务端 VFX 事件 schema 与范围过滤（新增于 2026-04-13 审计）✅

**已有基础（亲眼核实）**：
- `bong:server_data` CustomPayload 发送通道已在用：`server/src/network/agent_bridge.rs:7` 定义 `SERVER_DATA_CHANNEL = "bong:server_data"`，`server/src/network/cultivation_detail_emit.rs:91` 调用 `client.send_custom_payload(ident!("bong:server_data"), &bytes)`
- `ServerDataPayloadV1` 已有 8 个 variant（Welcome/Heartbeat/Narration/ZoneInfo/EventAlert/PlayerState/UiOpen/CultivationDetail）
- 客户端 `BongNetworkHandler` 已有 dispatch 架构

**待补（Phase 0 实际工作）**：
- [x] **决策 A**：VFX 事件**复用** `bong:server_data` + 新 `VfxEvent` payload variant，**vs. 独立** `bong:vfx_event` channel（对应 §7 第一条开放问题）
  - 复用利：schema 集中、dispatcher 现成、IPC 版本化一致
  - 独立利：高频 VFX 不挤占状态同步，可独立节流
- [x] 新增 `VfxEventV1`（`agent/packages/schema` TypeBox → Rust 双端对齐）：`event_id` / `origin` / `direction` / `color` / `strength` / `duration_ticks`
- [x] Bevy 端新增 VFX 事件队列（`Events<VfxEvent>` 或自建 `Resource`）+ tick flush system
- [x] **范围过滤新抽象**（关键）：基于 `Position` + 视距过滤收件人。现有 `emit_player_state_payloads` 是"每在线 client 一份"，VFX 需要"按发源点筛订阅者"——属本 phase 新增系统
- [x] 客户端 `BongNetworkHandler` 注册 VFX 事件分发器，复用现有 dispatch 模式

### 5.2 Phase 1 — 三个渲染基类 + 最小端到端链路 ✅

- [x] §1 三个基类（Line/Ribbon/GroundDecal）原型 + 单元测试渲染
- [x] 最小链路打通：server 发一个 `sword_qi_slash` → client 播 Line 粒子

### 5.3 Phase 2 — 首批资产与扩展 ✅

- [x] §4.1 首批粒子贴图资源制作（28 张 PNG：9 类基础贴图 + 20 张符文字符变体）
- [x] §4.4 event player 注册（VfxBootstrap 实际注册 9 个：sword_qi_slash / breakthrough_pillar / enlightenment_aura / tribulation_lightning / formation_activate / death_soul_dissipate + FlyingSwordDemoPlayer / FormationCoreDemoPlayer / BurstMeridianBengQuanPlayer，超出原计划 6 个）
- [~] 飞剑实体 + Ribbon 拖尾 demo（FlyingSwordDemoPlayer 已注册作为粒子 demo；正式 entity 集成留 plan-treasure-v1）
- [~] 符阵 BlockEntity + GroundDecal 粒子 demo（FormationCoreDemoPlayer 已注册作为粒子 demo；BE 正式集成留 plan-zhenfa-v1）

### 5.4 Phase 3 — 规模化与收敛

- [ ] §2.4 范围过滤 & 状态位 vs 事件流分流确认（Phase 0 落地后回归验证）
- [ ] 整体性能压测（100 粒子 / 500 粒子 / 1000 粒子）

---

## §6 已知风险

- **SEUS PTGI 等 path-tracing shader 下粒子隐形**（Iris #2499）—— README 声明推荐 shader 列表
- **Ribbon 粒子位置缓冲开销**：每粒子 16 帧 × Vec3，需评估批量上限
- **VFX 事件广播洪水**：大规模战斗可能每 tick 数十个 event，需节流/合并
- **客户端缺失事件 id**：server 发了新 event 但 client 未注册 → 静默忽略 + 日志警告

---

## §7 开放问题

- [x] `vfx_event` 是否要独立 CustomPayload channel？**已选独立 channel `bong:vfx_event`**（见 `server/src/network/vfx_event_emit.rs`），与 `bong:server_data` 解耦，节流逻辑集中在 `coalesce_requests` + `enforce_per_chunk_cap`
- [ ] 粒子贴图谁做？AI 生成 vs 外包 vs 自绘？
- [ ] Ribbon 粒子跨 tick 插值方案（避免 20Hz 卡顿感）？
- [ ] 是否做"VFX 预加载"机制，还是首次触发时 lazy load？
- [ ] 多人场景下同 event 去重策略（5 个玩家同时放技能 → 5 个 event 还是合并）？
- [ ] 客户端调试开关：显示粒子 bounding box / 事件日志叠加？

---

## §8 参考

**客户端（Fabric）**：
- Fabric Particle API 文档：https://fabricmc.net/wiki/tutorial:particles
- Ars Nouveau 粒子源码（参考 ribbon/trail 实现）
- Botania 魔法线条粒子
- Iron's Spells 'n Spellbooks 施法光环

**服务端（Valence / Rust）—— 2026-04-13 调研结论**：
- **Valence `ChunkLayer::play_particle`**：天然 chunk viewer 过滤，推荐作为 vanilla 粒子主入口（路径：`crates/valence_layer/src/chunk/…`，注释有 `TODO: move to valence_particle`）
- **Valence `Client::play_particle`**：单客户端 API，用于需要点对点的场景
- **Pumpkin 反面教材**（https://github.com/Pumpkin-MC/Pumpkin）：
  - `World::spawn_particle` 全服广播无过滤（`pumpkin/src/world/mod.rs`）
  - 无合批无节流，逐事件发包
  - 粒子枚举 codegen 对齐 vanilla，**不支持自定义粒子类型**（`pumpkin-data/src/generated/particle.rs`）—— 印证"自定义粒子必须走 CustomPayload"
  - `EndermanEntity::mob_tick` 注释明确"环境粒子从服务端发会造成巨大网络开销" → 支撑 §2.4 客户端自演原则

**姊妹文档**：
- `plan-vfx-v1.md`（光影栈总纲与 Iris 集成策略）

---

## §9 进度日志

- 2026-04-25：核对实装——P0 ✅、P1 ✅（三基类 Line/Ribbon/GroundDecal + Sprite 全 extends `SpriteBillboardParticle`，无 shader 冲突）、P2 部分 ✅（9 类粒子贴图齐 + 6 个 event player 已注册 `VfxBootstrap`，飞剑/符阵 demo 待办）；emissive RenderLayer、entity/BlockEntity 集成、§7 开放问题仍未动。
- 2026-04-30：实地核验补全——§1.1 emissive `LINE_EMISSIVE` RenderLayer 已实装；VfxBootstrap 实际注册 9 个 player（含 FlyingSwordDemo / FormationCoreDemo / BurstMeridianBengQuan）；§7 第一条开放问题已选独立 `bong:vfx_event` channel。P3（性能压测 + §2.4 范围过滤回归）保留为遗留；entity / BlockEntity 正式集成移交 plan-treasure-v1 / plan-zhenfa-v1。归档至 `docs/finished_plans/`。

---

## Finish Evidence

**归档时间**：2026-04-30

### 落地清单

| 阶段 | 关键 symbol（实际路径） |
|---|---|
| **P0** VFX schema + 范围过滤 | `server/src/network/vfx_event_emit.rs`（`VfxEventRequest` / `emit_vfx_event_payloads` / `coalesce_requests` / `enforce_per_chunk_cap` / 64 格距离过滤）；独立 `bong:vfx_event` channel |
| **P1** 三渲染基类 + 端到端 | `BongLineParticle.java` (含 `LINE_EMISSIVE` RenderLayer) · `BongRibbonParticle.java` · `BongGroundDecalParticle.java` · `BongParticleSheets.java` · `SwordQiSlashPlayer` 端到端链路 |
| **P2** 首批资产 + player | 28 张 PNG（sword_qi_trail / breakthrough_pillar / qi_aura / enlightenment_dust / tribulation_spark / flying_sword_trail / lingqi_ripple / sword_slash_arc + 20 张符文字符）；VfxBootstrap 注册 9 个 VfxPlayer |

### 关键 commit

- `8ca1a2dd` feat(vfx): plan-particle-system-v1 §1-§5 端到端落地
- `669d1c8e` plan-particle-system-v1: 收口粒子与世界 VFX
- `b10128bc` feat(player-animation): 支持 inline 动画 JSON 注入（含 BurstMeridianBengQuanPlayer 接入）
- `b0302396` feat: 落地爆脉崩拳真实结算（含粒子 demo player 注册）
- PR #17 主体合并

### 跨仓库核验

- **server**：`vfx_event_emit.rs` 全套（VfxEventRequest / coalesce / per-chunk cap / 距离过滤）；`bong:vfx_event` channel 常量
- **agent**：schema `agent/packages/schema/src/vfx.ts` TypeBox 定义 + 双端 sample
- **client**：`com.bong.client.vfx.particle/`（3 基类 + emissive RenderLayer）+ `com.bong.client.vfx/VfxBootstrap.java`（9 player 注册）+ 28 张粒子贴图

### 遗留 / 后续

- **P3 性能压测 + §2.4 回归验证**：100/500/1000 粒子规模化测试未跑；§2.4 持续状态类客户端自演 vs 一次性事件类服务端广播的分流回归未做。规模化战斗压测留 P3 后续 PR / 或合到 `plan-tribulation-v1` 三波 AOE 实装时一并验。
- **§3.1 Entity（飞剑 / 符箓投掷物）**：FlyingSwordDemoPlayer 仅作粒子 demo，正式 entity 注册 + 物理碰撞留 `plan-treasure-v1`
- **§3.2 BlockEntity（符阵中心 / 结界节点）**：FormationCoreDemoPlayer 仅作粒子 demo，正式 BE 注册 + 状态同步留 `plan-zhenfa-v1`
- **§1.3 GroundDecal 地形贴合**：仅高度微抬避 z-fighting，未按下方 bbox 微调 Y。无业务紧迫性，留作后续优化
- **§1.4 BongModelParticle**：明确为 optional 后期，未启动
- **§7 开放问题（剩 5 条）**：贴图来源 / Ribbon 跨 tick 插值 / VFX 预加载 / 多人去重 / 客户端调试开关，业务驱动时再处理
