# Bong · plan-npc-skin-v1

**NPC 视觉识别专项**。当前 NPC 全部借用 vanilla `EntityKind`（Zombie / Villager），100 散修视觉与凡人无异；本 plan 将 NPC 从 mob entity 切换为**假玩家实体 + 自定义 skin**，通过 MineSkin 拉取签名 skin property 注入 PlayerInfoUpdate 包。

**世界观锚点**：修真 NPC 视觉区分度缺失 → 玩家无法靠外观识别散修/弟子/妖兽/遗种；长袍、金甲、兽化等典型修仙意象需要 skin 承载。vanilla mob 模型解决不了（村民鼻子太大、僵尸是怪物动画）。

**交叉引用**：`plan-npc-ai-v1.md §1` archetype 分类 / §4 派系门派 · `CLAUDE.md` ConnectionMode::Offline（跳过签名校验前提）· `plan-combat-no_ui.md`（玩家视觉反馈）。

---

## §-1 现状

_截至 plan-npc-ai-v1 §§1-3 合并后；详见 `server/src/npc/spawn.rs`。_

| Archetype | EntityKind | 视觉 |
|-----------|-----------|------|
| Zombie | `ZOMBIE` | 僵尸 skin（startup NPC + scenario/duel 用） |
| Commoner | `VILLAGER` | 村民 skin（大鼻子、浅棕袍） |
| Rogue | `VILLAGER` | 同 Commoner，**完全无法区分** |
| Beast / Disciple / GuardianRelic | 未实装 | — |

100 散修启动种群虽已铺到 6 个 zone，但玩家在 lingquan_marsh 看到一片村民，无法判断这是修炼中的散修还是普通凡人。

---

## §0 设计轴心

- [ ] **NPC = 假玩家**：`PlayerEntityBundle` 替代 mob bundle；vanilla 协议里**只有** Player 实体能承载自定义 skin
- [ ] **Skin = 数据**：skin 作为 `SignedSkin { value, signature }` 数据流转，与实体解耦；同一 skin 可复用到多个 NPC
- [ ] **Offline mode 跳过签名**：`CLAUDE.md` 指定 `ConnectionMode::Offline` → 不做 Mojang 真实签名验证 → 可以 fake signature 或用 MineSkin 自签值
- [ ] **Skin 来源外包**：MineSkin.org API + 手工 skin pack 二选一或并用；**不自己搭 skin 托管**
- [ ] **池化复用**：每 archetype 预加载 20-50 张 skin，NPC 从池子抽；100 个散修不 = 100 张独立 skin
- [ ] **零素材洁癖**：first version 可复用任何公开 skin（MineSkin 公有池）；长期由艺术资源替换，但协议层不变
- [ ] **UUID 稳定确定性**：NPC UUID 从 `Entity::to_bits()` 衍生，同一 NPC 重连期间 UUID 不变（客户端缓存不失效）
- [ ] **Fallback 优雅降级**：MineSkin 不通 / API key 失效 / 限流 → 退回到 vanilla `EntityKind::WITCH / VILLAGER / ZOMBIE`；启动不阻塞

## §1 Skin 数据流

```
MineSkin API        Bong Server                  MC Client
──────────────      ───────────────────────      ──────────────
/v2/generate  ───→  SkinPool::prefetch()
                       ↓ SignedSkin{ value, signature }
                    SkinPool Resource
                       ↓ SignedSkin::clone()
                    NPC spawn
                       ↓ PlayerEntityBundle { uuid }
                       ↓ PlayerInfoUpdateS2c{ actions: AddPlayer,
                       ↓   players: [{ uuid, name, properties:[
                       ↓     { name:"textures", value, signature }
                       ↓   ], game_mode, listed:false, latency:0, display_name:None }]}
                                                 → client 渲染散修 skin
```

### §1.1 SignedSkin 结构

```rust
#[derive(Clone, Debug)]
pub struct SignedSkin {
    /// Mojang-format base64-encoded textures JSON
    pub value: String,
    /// Offline mode 下不验证；MineSkin 仍提供自签值
    pub signature: String,
    /// 记录来源便于 debug / 刷池
    pub source: SkinSource,
}

pub enum SkinSource {
    MineSkinGenerate { uuid: String, timestamp: u64 },
    MineSkinRandom { hash: String },
    LocalPack { path: PathBuf },
    Fallback, // 用于池子空时的哨兵，引用 vanilla EntityKind
}
```

### §1.2 textures JSON payload（signed value 解码后）

```json
{
  "timestamp": 1728300000000,
  "profileId": "aaaaaaaaaaaaaaaa...",
  "profileName": "bong_rogue_001",
  "textures": {
    "SKIN": {
      "url": "https://textures.minecraft.net/texture/<hash>"
    }
  }
}
```

Bong 不需要解码 value；只是透传。

## §2 MineSkin 接入

### §2.1 API 选型

| Endpoint | 用途 | 配额 |
|---|---|---|
| `POST /v2/generate/url` | URL → signed skin | 有；每账户秒级限 |
| `POST /v2/generate/upload` | 上传 PNG → signed skin | 较紧 |
| `GET /v2/skins?size=20&type=random` | 从 MineSkin 公共池抽 | 较松，适合启动预热 |
| `GET /v2/skins/<id>` | 按 id 取特定 skin | 无限 |

- [ ] 启动预热用 `/v2/skins?type=random`（公有池）
- [ ] 特定艺术资源上线后用 `/v2/generate/upload` 固化到本地 skin pack，运行时不再调 API
- [ ] **不做** per-NPC 实时 generate（那会打死 rate limit）

### §2.2 Secret 管理

- [ ] API key 放 `server/.env`：`MINESKIN_API_KEY=msk_***`
- [ ] `.env` 进 `.gitignore`
- [ ] `server/src/main.rs` 启动时 `std::env::var("MINESKIN_API_KEY")` 读取
- [ ] 缺失 key → `SkinPool` 进 fallback 模式（Log warn，不 panic）
- [ ] 不把 key 写 ron/json 配置（避免截图/日志泄漏）

### §2.3 HTTP Client

- [ ] 新 crate dep：`reqwest = { version = "0.12", features = ["json", "rustls-tls"] }`
- [ ] `tokio` 已有；直接借用
- [ ] bevy app ↔ tokio runtime 桥：启动时 `tokio::runtime::Handle::current()`，后台 task `spawn` 跑预热，完成后通过 `crossbeam_channel` 回传 `SignedSkin` 到 ECS Resource
- [ ] **不在 ECS system 里同步 await**（会卡 tick）

### §2.4 预热策略

- [ ] 启动后台 task：`prefetch_skins(pool, count=20)`
- [ ] 进度由 `SkinPool::ready_count()` 暴露，seed 100 散修系统等待 `ready_count >= MIN_READY_BEFORE_SPAWN(=5)` 再开始 spawn
- [ ] 边拉边用：拉到第 5 张就开始 spawn，剩余 15 张后台继续；NPC 循环取池子（`index % ready_count`）
- [ ] 超时（30s）未达 MIN → fallback，直接 spawn vanilla Witch

## §3 协议层 —— PlayerInfoUpdateS2c 手写

_Valence `2b705351` alpha 没内置 PlayerList API，本节最重。_

### §3.1 Packet 结构（MC 1.20.1 · protocol 763）

```
PlayerInfoUpdate (0x3C):
  actions: BitSet<8>    // AddPlayer | InitializeChat | UpdateGameMode |
                        // UpdateListed | UpdateLatency | UpdateDisplayName
  players: VarInt len
    for each:
      uuid: UUID
      (when AddPlayer)   name: String<=16>
                         properties: Array<PlayerProperty>
      (when Listed)      listed: bool
      (when GameMode)    game_mode: VarInt
      (when Latency)     latency: VarInt (ms)
      (when DisplayName) display_name: Option<TextComponent>

PlayerProperty:
  name: String<=64>       // "textures"
  value: String<=32767>   // base64
  signature: Option<String<=1024>>  // offline 可 None 或任意
```

### §3.2 Bong 实现策略

- [ ] 新模块 `server/src/skin/packet.rs`
- [ ] 手搓 `NpcPlayerInfoUpdateS2c` struct + `Encode` impl
- [ ] 直接用 `Client::write_packet` 发给每个在线客户端
- [ ] 不污染 valence 现有 Client 类型；通过 `Query<&mut Client>` 遍历广播

### §3.3 两类时机

1. **新 NPC spawn 时**：广播 AddPlayer（含 properties）+ UpdateListed(listed=false) 给**所有在线**客户端
2. **玩家 join 时**：为该客户端**补发**当前所有活跃假玩家的 AddPlayer 批量包

第 2 点是时序坑：新客户端连接后若不补发，它看 NPC 是默认 Steve skin（因为本地没有对应 profile）。需要在 `ClientMarker` `Added` 时机跑一个 `send_skin_catchup_to_new_client` 系统。

### §3.4 Tab 列表隐藏

- [ ] `AddPlayer` 动作后立刻同批跟 `UpdateListed(false)` —— 客户端收到 listed=false 的 entry 不显示在 tab
- [ ] 另一种方案：压根不发 AddPlayer（只 spawn entity），但这样 skin 不生效；所以只能 AddPlayer + hide
- [ ] 客户端设置里"Show Spectators"等选项可能暴露；接受

### §3.5 NPC despawn 时

- [ ] 发 `PlayerInfoRemoveS2c(0x3D)` 把 uuid 从 list 移除
- [ ] 否则客户端内部 profile 缓存残留；下次同 uuid 上线时可能用旧 skin

## §4 UUID 稳定生成

### §4.1 衍生规则

```rust
fn npc_uuid(entity: Entity) -> Uuid {
    let (index, gen) = (entity.index(), entity.generation());
    // namespace 固定，确保不与 Mojang 真 UUID 冲突
    let namespace = Uuid::from_u128(0xbong_npc_u128_constant);
    Uuid::new_v5(&namespace, &format!("npc_{index}v{gen}").into_bytes())
}
```

- [ ] 与 `canonical_npc_id(entity)` 一致的 key 做 UUID v5
- [ ] 同 NPC 重启后 entity.index 会变 → UUID 变；接受（启动时重发 AddPlayer）
- [ ] **不要** 让 NPC UUID 碰撞 Mojang namespace；v5 +专属 namespace 足够

### §4.2 死亡 / 换代处理

- [ ] NPC Despawned 时调用 `remove_skin_for_npc(uuid)` 发 PlayerInfoRemove
- [ ] Commoner 老死 → 新生代 spawn 拿新 entity，会得到新 UUID + 新 skin；不复用死者 profile
- [ ] 避免"同坐标同时刻有两个同 uuid 的假玩家"卡客户端

## §5 Archetype → SkinPool 分桶

### §5.1 桶结构

```rust
pub struct SkinPool {
    by_archetype: HashMap<NpcArchetype, VecDeque<SignedSkin>>,
    failover: Vec<SignedSkin>, // 任何 archetype 都可抽
}
```

- [ ] Rogue / Commoner 各一桶；Zombie 保留 vanilla（战斗怪不走假玩家，避免 AddPlayer 污染战斗逻辑）
- [ ] 首版只做 Rogue + Commoner
- [ ] Disciple / GuardianRelic 留接口，后续 plan §4 派系实装时填

### §5.2 抽取策略

- [ ] `SkinPool::next_for(archetype)` 返回 `SignedSkin`：轮转 + entity.index 作盐防止同桶内 NPC 连续相同
- [ ] 池子 <= 5 张时触发后台补充（MineSkin 追加拉）
- [ ] 池子空 → `SkinSource::Fallback`，spawn 退回 vanilla EntityKind

## §6 Fallback 行为

- [ ] MineSkin API 失效 / 网络不通：启动时 try 3 次，失败后**全量 fallback**
- [ ] 启动日志：`[bong][skin] MineSkin unavailable (error=<>), falling back to vanilla entity kinds for 100 rogues`
- [ ] fallback 不影响 plan-npc-ai-v1 玩法闭环；只是视觉降级
- [ ] 运行时部分失败（某几张拉不到）：用 failover 桶补；继续 spawn
- [ ] 客户端也要能容忍：若 skin property 缺失，客户端自动用默认 Steve；不崩

## §7 实施节点

**Phase 0 — Secret + dep**
- [ ] 加 `reqwest` dep
- [ ] `server/.env` 模板 + `.gitignore`
- [ ] `std::env::var("MINESKIN_API_KEY")` 读取
- [ ] 缺失 key 走 fallback 分支（warn，启动不阻塞）

**Phase 1 — MineSkin HTTP client**
- [ ] 新 `server/src/skin/mineskin.rs`
- [ ] `async fn fetch_random(count: usize) -> Result<Vec<SignedSkin>>`
- [ ] retry + jittered backoff
- [ ] 单测用 `wiremock` 打桩 API 响应

**Phase 2 — SkinPool**
- [ ] 新 `server/src/skin/pool.rs`
- [ ] `SkinPool` Resource + `insert` / `next_for` / `len_for`
- [ ] 启动 task：`tokio::spawn` 拉取 + `crossbeam` 回传填充
- [ ] 单测：pool 为空时 next_for 返回 fallback；填充后 round-robin

**Phase 3 — PlayerInfoUpdate packet**
- [ ] 新 `server/src/skin/packet.rs`
- [ ] 手写 `NpcPlayerInfoUpdateS2c` + `Encode`
- [ ] 辅助 `broadcast_add_player(clients, npc_uuid, name, skin)` / `broadcast_remove_player(clients, npc_uuid)`
- [ ] 单测：packet 字节级固定（和 wiki 比对）

**Phase 4 — Spawn 切 PlayerEntityBundle**
- [ ] `spawn_rogue_npc_at` 替换 `VillagerEntityBundle` → `PlayerEntityBundle`
- [ ] 生成 uuid = `npc_uuid(entity)`
- [ ] 抽 skin 并 `broadcast_add_player`
- [ ] `spawn_commoner_npc_at` 同上
- [ ] Zombie 不动

**Phase 5 — 新客户端 join 补发**
- [ ] 新系统 `send_skin_catchup_to_new_client`
- [ ] `Query<Entity, Added<ClientMarker>>` + `Query<(Entity, &NpcPlayerSkin)>`（新 Component）
- [ ] 对每个新 client 批量发 AddPlayer 覆盖所有活跃 NPC

**Phase 6 — Despawn 时 remove**
- [ ] `handle_npc_terminated` + `process_npc_retire_requests` 路径接入 `broadcast_remove_player`
- [ ] DuelScenario Clear 也要 remove

**Phase 7 — 集成到 seed 100 散修**
- [ ] `seed_initial_rogue_population_on_startup` 等 pool ready 5 张后开工
- [ ] 超时回 fallback
- [ ] log 记录 actual skin distribution

**Phase 8 — Per-archetype skin pack**（艺术侧）
- [ ] 手工/AI 生成 20 张 Rogue skin（修真长袍）+ 20 张 Commoner skin
- [ ] 上传 MineSkin 得 signed value，固化到 `server/data/skin_pack/*.json`
- [ ] 启动优先读本地 pack，MineSkin 只做补充

**Phase 9 — 境界 / 派系变体**（可选深化）
- [ ] Rogue realm=Void 换金甲 skin
- [ ] Disciple 按 FactionId 换门派袍（Attack 红 / Defend 灰 / Neutral 青）
- [ ] 运行时 realm 变化 → re-broadcast AddPlayer with new skin

## §8 已决定

- ✅ **假玩家路线**：vanilla Player entity 是唯一承载自定义 skin 的通道；接受它的 PlayerList 注册成本
- ✅ **Offline 模式下不校验签名**：跳过 Mojang 验证链路，MineSkin 自签或空签均可
- ✅ **Zombie 保留 mob bundle**：战斗怪不走假玩家，避免 AddPlayer 污染 scenario/duel 调度逻辑
- ✅ **UUID v5 + Bong namespace**：确定性、不与真人 UUID 冲突
- ✅ **Fallback 不阻塞启动**：key 缺失 / API 挂 / pool 超时 → 退 vanilla，游戏继续
- ✅ **Tab 列表隐藏**：listed=false 而非完全不发 AddPlayer（后者 skin 不生效）
- ✅ **首版 Rogue + Commoner**：Beast / Disciple / GuardianRelic 等 archetype 落地后再接
- ✅ **池化**：20-50 张 skin 服务 100-500 NPC；艺术侧独立推进，不阻塞协议层
- ✅ **.env 管 key**：不进代码、不进配置文件、不进 log

## §9 剩余开放问题

- **新客户端 join 时机**：valence 的 `ClientMarker Added` 和 join 完成的时序差几 tick？补发过早会被初始化包盖住。需要 Phase 5 实测。
- **MineSkin 公有池 skin 风格参差**：可能抽到现代玩家抖音 skin；不修真。Phase 8 自制 pack 是长期方案。
- **per-realm 变身的客户端行为**：同一 uuid 重发 AddPlayer with 不同 textures，客户端是否 hot-reload skin 还是保留旧？需查 MC 协议行为。
- **Entity.generation 变化后的 UUID 漂移**：NPC 短暂死后再次 spawn 成新 entity → 新 UUID → 旧 profile 泄漏在客户端缓存。Phase 6 remove 时机是否足够覆盖？
- **Render distance 外的 NPC 何时不发 AddPlayer**：为性能考虑应分桶；但 500 NPC × 多客户端的 packet 量需压测。

## §10 后续派生 plan

- [ ] `plan-npc-skin-pack-v1` —— 手工/AI skin pack 生产流水线
- [ ] `plan-npc-nametag-v1` —— 头顶浮标（境界 / 派系 / 名号）；与 skin 互补，先于本 plan 可交付
- [ ] `plan-npc-anim-v1` —— 修真姿态（打坐 / 飞剑 / 御空）；需要 PlayerAnimator 接入，见 `third_party/` vendored 参考

## §11 零工作量替代路线（本 plan 未采用）

存档以备决策回溯：

- **Vanilla `EntityKind::WITCH`** 换皮：Rogue = 巫师模型；零代码、零 HTTP、零协议。视觉区分有限，境界/派系不分，但 1-2 小时即可。
- **EntityCustomName 浮标**：头顶 `[散修·凝气四重]` 文字，与现 EntityKind 组合。不换 skin 但信息层到位。

如果本 plan Phase 3-5 的协议层实测超出预期复杂度，回退到 `plan-npc-nametag-v1` + Witch 换皮的组合拳。

---

## §12 进度日志

- 2026-04-25：调研完成，代码层零实装。`server/src/skin/` 模块未建，无 `reqwest` 依赖，无 `.env` 模板，无 `SignedSkin`/`SkinPool`/`NpcPlayerInfoUpdateS2c`/`npc_uuid` 任何符号；`server/src/npc/spawn.rs` 仍用 `VillagerEntityBundle`(Rogue/Commoner) + `ZombieEntityBundle`(Beast) 视觉占位（见 spawn.rs:558/617/674）。Phase 0-9 全部 `[ ]` 保持未勾选，等待开工。

## Finish Evidence

- 合并 commit：`b46245e2 feat(npc-skin): 接入 MineSkin 假玩家外观 (#73)`（2026-04-28），落地 `server/src/skin/`（MineSkin random fetch + retry/backoff、`SkinPool` resource、`NpcPlayerInfoUpdateS2c`/remove packet wrapper、`npc_uuid`、`.env.example` 与依赖锁定），并将 Rogue/Commoner spawn 接入 `PlayerEntityBundle` + `NpcPlayerSkin`（保留 Zombie mob 路径），覆盖启动 100 散修、agent spawn、繁衍 spawn、despawn remove 广播。
- Fallback：MineSkin key 缺失、拉取失败或池空时不阻塞启动；Rogue/Commoner 会降级到 vanilla fallback，死亡/clear 等 `Despawned` 路径由统一系统发 `PlayerInfoRemove`，避免双发。
- 验证：在 `server/` 运行 `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` 通过；`cargo test` 结果 `1616 passed; 0 failed; 0 ignored`。
- 跨栈核验：本 plan 为 server-only；client/agent/worldgen 未改动，未运行跨栈命令。
- 遗留：艺术侧固定 skin pack、境界/派系变体、运行时 hot-reload skin、render-distance 分桶优化仍按 §8/§9/§10 后续 plan 处理，不阻塞本 plan 归档。
