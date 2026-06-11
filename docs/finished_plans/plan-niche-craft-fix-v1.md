# plan-niche-craft-fix-v1 — 灵龛制作流断链修复

> 一句话：修复 `niche_base` 双断链（配方产出 `niche_base` 但放置 handler 只认 `spirit_niche_stone`，整段制作流形同虚设）+ 为 SpiritNiche 本体补损伤/修补生命周期并给 `niche_repair_kit` 建 use 闭环，顺带收口 `NicheDefenseReactionVfxPlayer` 的视听断链。
>
> 来源：放置类 17 调查 workflow，**红旗 #9（最严重）**：`workbench_recipes.rs:1341` 配方 #98 消耗 `spirit_niche_stone`+`ling_tie`×2+`wood_plank`×4 产出 `niche_base`，而 `handle_spirit_niche_place_requests`（`social/mod.rs:1467`）只认 `SPIRIT_NICHE_ITEM_TEMPLATE_ID="spirit_niche_stone"`（`social/mod.rs:80`），玩家合成的 `niche_base` 放置时被 `mod.rs:1537` 的 `instance.template_id != SPIRIT_NICHE_ITEM_TEMPLATE_ID` 判定 warn 拒绝。

**依赖**：无根 plan 依赖。复用 `plan-niche-defense-v1`（finished，PR #130）的 `SpiritNiche` / `HouseGuardian` / `resolve_intrusion` 实体系统，但 niche-defense **未在 SpiritNiche 上加任何 HP/损伤字段**（`components.rs:228-236` 仅 `owner/pos/placed_at_tick/revealed/revealed_by/guardians`），P1 的损伤字段为本 plan 新增。复用 `plan-workbench-recipes-v1`（finished）已注册的配方 #98（`niche_base`）/ #90（`niche_repair_kit`）。

| 阶段 | 主题 | 状态 | 验收日期 |
|------|------|------|----------|
| P0 | `niche_base` 接通放置（ID 断链收口） | ✅ | 2026-06-10 |
| P1 | SpiritNiche 损伤/修补生命周期 + `niche_repair_kit` use 闭环 + 视听断链收口 | ✅ | 2026-06-10 |

---

## 接入面（防孤岛 checklist）

- **进料**：
  - `server/src/craft/workbench_recipes.rs:1341-1363`（配方 #98，产 `niche_base`）/ `:1223-1233`（配方 #90，产 `niche_repair_kit`）——已注册，可合成
  - `server/src/social/mod.rs:1467` `handle_spirit_niche_place_requests`——已有完整 inventory 消耗 + 注册逻辑，P0 改 `:1537` ID 判定
  - `server/src/social/components.rs:228` `struct SpiritNiche`——P1 在此新增损伤字段
  - `server/src/social/niche_defense.rs:140` `fn resolve_intrusion` / `:238` `fn handle_niche_intrusion_attempts`——P1 损伤触发来源候选
  - `server/assets/items/core.toml:64` `spirit_niche_stone`（category=treasure，"可埋作私有灵龛的冷石"）/ `server/assets/items/workbench_materials.toml:918` `niche_base`（category=misc，rarity=rare，"龛石灵铁木台组合的永久复活点基座。"）/ `:826` `niche_repair_kit`（"碎石灵铁混合料，修补损坏灵龛。"）
- **出料**：
  - 放置成功 → `lifecycle.spawn_anchor = Some(spirit_niche_spawn_anchor(event.pos))`（`social/mod.rs:1586`，`fn` 在 `:1835`）→ 死亡复活于此（`worldview.md:1097` 重生位置：灵龛 > 出生点）
  - 修补成功 → 清除 SpiritNiche 损伤状态 + emit server audio VfxEvent（新建 `niche_repair` recipe）
  - 受损态 → 复活功能降级（见 §8.1 #2 决议）
- **共享类型 / event**：
  - 复用 `social::SpiritNiche`（component）/ `social::SpiritNicheRegistry`（Resource）/ `social::events::SpiritNichePlaceRequest`（`events.rs:127`，P0 不改）
  - P1 新增 `social::events::SpiritNicheRepairRequest { player: Entity, pos: [i32;3], item_instance_id: Option<u64>, tick: u64 }`（仿 `SpiritNichePlaceRequest` 结构），**不复用 PlaceRequest 避免语义混淆**
  - **P1 损伤触发不新增事件**：`handle_niche_intrusion_attempts`（`niche_defense.rs:238`）已持 `Query<&mut SpiritNiche>` + 抄家成功分支（`:282`），就地 `niche.is_damaged = true` + 同步 registry/持久化（见 P1-a）。**取消原计划的 `SpiritNicheDamaged` 事件 + `apply_spirit_niche_damage` system**（避免绕开既有直接通路）；同样**不复用 `NicheGuardianBroken`（`events.rs:200`，守卫层，非本体）**
- **跨仓库契约**：
  - server↔client：P1 新增 `ClientRequestV1::SpiritNicheRepair { v, x, y, z, item_instance_id }`（`server/src/schema/client_request.rs`，仿 `:187` `SpiritNichePlace`）；client 侧新增 `ClientRequestProtocol.encodeSpiritNicheRepair`（仿 `:674`）+ `ClientRequestSender.sendSpiritNicheRepair`（仿 `:164`），wire type `"spirit_niche_repair"`
  - server VfxEvent ID：复用 `gameplay_vfx::SOCIAL_NICHE_ESTABLISH`（`network/gameplay_vfx.rs:25`）模式，P1 新增 `SOCIAL_NICHE_REPAIR = "bong:social_niche_repair"`
  - agent **不参与**（与 niche-defense 一致）
- **worldview 锚点**：
  - §十一.安全空间（灵龛）（`worldview.md:910-920`）：灵龛=永久复活点+藏物+养伤；"消耗一次性道具「龛石」"——`spirit_niche_stone` 即"龛石"，制成 `niche_base` 基座后放置，语义不破坏（§8.1 #1 选 A 的正典依据）
  - §十二.死亡、重生与一生记录（`worldview.md:1097`）："重生位置：灵龛（如有）> 出生点"——`spawn_anchor` 正典支撑
- **qi_physics 锚点**：**无**。放置消耗走 `apply_spirit_niche_negative_qi_cost`（`mod.rs:1569`，social 内部负真元代价，非 ledger 真元流动）；修补消耗为纯物料，无真元流动，本 plan **不触及 qi_physics，不引入任何衰减/挥发常数**。
- **遗留资产红旗顺带修**（调查 #8 + `reminder.md:25`）：`client/.../visual/NicheDefenseReactionVfxPlayer.java` `reactionIdsFor()` 返回 `niche_guardian_broken` / `niche_guardian_fatigue` / `niche_intrusion_ink_mark` 三个 SFX ID，但**零调用方**（全仓仅类定义，无 import），且 client 无 JSON-recipe 运行时 loader——双重断链，P1-d 收口（见 P1-d 架构核实：改 `NicheIntrusionAlertHandler` 本地程序化 `SoundRecipePlayer.play`，而非新建 JSON 资产；`NicheIntrusionAlertHandler` 已是 server→client `SocialServerDataHandler:229/240` 推送的活 callsite）。

---

## P0 — `niche_base` 接通放置（ID 断链收口）

**裁决（§8.1 #1，pre-P0 已收口）**：选 A —— handler 改认 `niche_base`，`spirit_niche_stone` 降为纯制作材料。

**交付物**：

- `server/src/social/mod.rs:80`：`const SPIRIT_NICHE_ITEM_TEMPLATE_ID: &str = "spirit_niche_stone";` → `"niche_base"`
- `server/src/social/mod.rs:2920` 附近 dev give 路径（`spirit_niche_test_item` / dev `/give` 写死 `SPIRIT_NICHE_ITEM_TEMPLATE_ID`）：因 const 改值自动跟随；但 dev give 命令若另有硬编码 `"spirit_niche_stone"` 字符串需同步改为 `"niche_base"`（grep 全仓 `"spirit_niche_stone"` 字面量收口，确认仅留在配方 `workbench_recipes.rs:1341` 作为消耗材料）
- `server/src/social/mod.rs:1537` 的 `instance.template_id != SPIRIT_NICHE_ITEM_TEMPLATE_ID` warn 文案 `"item ... is not a niche stone"` 改为 `"is not a niche base"`，避免误导
- **新手起始灵龛物品收口（关键 wiring，§8.1 #1 补充）**：`server/src/world/spawn_tutorial.rs:442` 开棺 grant 当前给 `SPIRIT_NICHE_STONE_TEMPLATE_ID="spirit_niche_stone"`（`:43`）作为玩家出生起始灵龛物品（worldview §十一"每个玩家出生时获得一个灵龛"）。选 A 后 `spirit_niche_stone` 变非放置材料，**新手将无法用起始物放置灵龛**——必须把开棺 grant 改为 `niche_base`：`spawn_tutorial.rs:43` const 重命名 `SPIRIT_NICHE_STONE_TEMPLATE_ID` → `SPIRIT_NICHE_BASE_TEMPLATE_ID` 并改值 `"niche_base"`（`:442` grant、`:896`/`:1098` 测试 helper 同步），保证"出生获赠可直接放置的灵龛基座"语义不破
- **不改 client**：放置请求走既有 `spirit_niche_place` wire type（`ClientRequestProtocol.encodeSpiritNichePlace`，`client/.../ClientRequestProtocol.java:674`），P0 只换 server handler 认的 template_id；client 放置交互（手持可放置物 → C2S `SpiritNichePlace`）不依赖具体 template_id 字符串

**饱和化测试声明**（`social::mod::tests`）：
- happy path：背包持 `niche_base` 实例 → send `SpiritNichePlaceRequest` → 放置成功 + `lifecycle.spawn_anchor == Some(...)` + `registry` 注册该 niche + `niche_base` 实例被消耗
- 边界：持 `spirit_niche_stone`（旧 ID）放置 → **被拒绝**（断言新 const 生效，旧材料不再可直接埋）
- 错误分支：① 持非灵龛物品（如 `wood_plank`）→ 拒绝且不消耗；② 目标坐标过远（`niche_place_target_is_close` false）→ 拒绝；③ 目标已被他人灵龛占用 → 拒绝；④ `item_instance_id` 缺失 → 拒绝
- 断链回归：新增测试 `niche_base_template_id_is_recipe_output`——断言 `SPIRIT_NICHE_ITEM_TEMPLATE_ID` 与配方 #98（`workbench_recipes.rs:1341`）的 output template_id 字符串相等（grep 抓手，让产物 ID 漂移立刻撞红）
- 起始 grant 回归（`spawn_tutorial::tests`）：开棺 grant 的 template_id == `niche_base`，且该起始物可被 `handle_spirit_niche_place_requests` 接受放置（断言新手出生灵龛 worldview §十一 promise 不破）
- 状态转换：放置前 `spawn_anchor == None` → 放置后 `== Some(spirit_niche_spawn_anchor(pos))`；二次放置（已有灵龛）→ `registry.upsert` 替换旧 niche（owner 唯一性）

**e2e（`social` 集成测试）**：合成 `niche_base`（配方 #98）→ C2S `SpiritNichePlace` → server 注册复活点 → `/kill self` → 复活坐标 == 灵龛坐标。

P0 纯 server 逻辑（无新玩家可感知视听，复用既有 `SOCIAL_NICHE_ESTABLISH` VfxEvent），免视听规格。

---

## P1 — SpiritNiche 损伤/修补生命周期 + `niche_repair_kit` use 闭环 + 视听断链收口

### P1-a 损伤字段与触发（server）

**裁决（§8.1 #2/#3，pre-P0 已收口）**：SpiritNiche 新增 `is_damaged: bool`（**不上多档 enum**，受损=单一降级态）；触发条件采用"成功抄家（入侵者取走灵龛内藏物）→ 灵龛受损"，**不依赖守卫是否全灭**（更直接、符合"龛石被暴力挖掘"直觉）。

**交付物**：

- `server/src/social/components.rs:228` `struct SpiritNiche` 新增 `#[serde(default)] pub is_damaged: bool`（向后兼容旧存档反序列化）
- `server/src/social/mod.rs:1583` 附近 `SpiritNiche { ... }` 构造体补 `is_damaged: false`
- `server/src/social/components.rs` 的 `SpiritNicheSqlRow`（`mod.rs:88` 类型别名）+ 持久化读写路径加 `is_damaged` 列（grep `SpiritNicheSqlRow` 全部命中点同步）
- **触发（就地置位，不绕事件 hop —— source-of-truth 单一化）**：`handle_niche_intrusion_attempts`（`niche_defense.rs:238`）**已持有 `mut niches: Query<&mut SpiritNiche>`**（`:240`）且已有抄家成功分支 `if !outcome.record.items_taken.is_empty()`（`:282`）。在该分支内**就地** `niche.is_damaged = true`（`niche` 即 `:248` 取到的 `&mut SpiritNiche`），无需新事件、无需新 system。**取消 P1 原计划的 `SpiritNicheDamaged` 事件 + `apply_spirit_niche_damage` system**（避免 §四「近义重名/自产自消」变体：新增事件绕过已有直接通路）
- **source-of-truth 一致性（关键，必须写死）**：当前 `niche_defense.rs` 全程只操作 `SpiritNiche` **component**，从不 touch `SpiritNicheRegistry`（grep 确认 `niche_defense.rs` 内零 registry 调用）；而 P0 放置 handler 同时写 `registry.upsert` + `commands.entity().insert` + `persist_social_spirit_niche`（`mod.rs:1588-1613`）——存在 component / registry / SQL 三处。**决议：`SpiritNiche` component（玩家 entity 上）为 `is_damaged` 的唯一权威源（authoritative source）**。`niche_defense.rs` 就地置 component 后：
  1. **同步 registry**：在同一分支调 `registry.upsert(niche.clone())`（给 `handle_niche_intrusion_attempts` 加 `mut registry: ResMut<SpiritNicheRegistry>` 参数）——确保 registry 不漂移
  2. **同步持久化**：调 `persist_social_spirit_niche(persistence, &niche)`（加 `persistence: Option<Res<...>>` 参数，warn-on-error，与放置 handler `mod.rs:1613` 一致）
  3. **复活读取方**：§P1-a 降级链读的是 component（经 `commands.entity(owner).insert` 同步到的 `Lifecycle.spawn_anchor_damaged`）或 registry，二者此处一并写，不留单写
- **双写一致性测试**（`social::niche_defense::tests`）：抄家成功后断言 ① `SpiritNiche` component `is_damaged == true` ② `registry.niches.get(&owner).unwrap().is_damaged == true`（component 与 registry 不漂移）③ 持久化路径被调用（mock persistence 或断言无 panic）
- **受损态复活降级（跨模块 wiring，本阶段真正难点，必须按此交付）**：虚弱机制在 **combat** 模块（`combat/components.rs:17` `pub const REVIVE_WEAKENED_TICKS: u64 = 180 * TICKS_PER_SECOND;`，`Lifecycle::revive(now_tick)` `combat/components.rs:242` 硬编码 `weakened_until_tick = Some(now_tick + REVIVE_WEAKENED_TICKS)`），而 `is_damaged` 在 **social** 模块的 `SpiritNiche` component / `SpiritNicheRegistry` Resource。`revive_lifecycle`（`combat/lifecycle.rs:1269`）当前参数表只有 `Lifecycle/Cultivation/Wounds/...`，**无 SpiritNiche query、无 SpiritNicheRegistry Resource**，且 `lifecycle.spawn_anchor`（`:1376`）只是裸 `[f64;3]` 坐标，无法反查所属 niche 的 `is_damaged`。因此降级**不是"grep 一个字段乘 2"，而是一条跨 combat↔social 的注入链**，交付物精确到行号：
  1. **改 `Lifecycle::revive` 签名**（`combat/components.rs:242`）：新增 `fn revive_weakened(&mut self, now_tick: u64, weakened_multiplier: u64)`（或给 `revive` 加 `multiplier: u64` 参数，默认调用方传 1），内部 `weakened_until_tick = Some(now_tick + REVIVE_WEAKENED_TICKS * multiplier)`。**不写字面秒数**，仍以 `REVIVE_WEAKENED_TICKS` 为基准 × multiplier
  2. **给 `revive_lifecycle` 注入 social 侧权威源**（`combat/lifecycle.rs:1269` 参数表）：新增 `niche_registry: Option<&SpiritNicheRegistry>` 参数（从调用 `revive_lifecycle` 的上游 system 传入，该 system 加 `niche_registry: Option<Res<SpiritNicheRegistry>>`）。在 `:1293`/`:1344` 调 revive 处，用 `lifecycle.spawn_anchor` 坐标 + 玩家 `character_id` 在 `niche_registry` 中匹配该玩家的 niche（`registry.niches.get(&char_id)`，再比对 `spirit_niche_spawn_anchor(niche.pos) == spawn_anchor`），命中且 `niche.is_damaged == true` → 传 multiplier=2，否则 multiplier=1
  3. **跨模块依赖方向**：combat 能否 `use crate::social::SpiritNicheRegistry`？实施时先 grep `server/src/combat` 现有 import 与 social→combat 既有依赖方向；若反向引用会成环，则改为：social 侧在 `handle_niche_intrusion_attempts` 抄家成功分支置 `is_damaged` 的**同一处**，对该玩家 entity `commands.entity(owner).insert` 时同步翻转一个 combat 可读的轻量标记（如在 `Lifecycle` 加 `#[serde(default)] pub spawn_anchor_damaged: bool`），`revive_lifecycle` 只读 `lifecycle.spawn_anchor_damaged`，无需 combat 反向依赖 social registry。**P1-a 实施前必须先 grep 判定方向，二选一并在 commit 说明；无论哪条，复活路径必须真读到该标记（加 revive 读取测试兜半僵尸）**
- **测试**：新增 `revive_lifecycle` 注入测试——构造受损 niche（registry 或 `spawn_anchor_damaged=true`）→ 触发复活 → 断言 `weakened_until_tick - now_tick == REVIVE_WEAKENED_TICKS * 2`（取 const 引用 ×2，不写字面）；对照组完好灵龛 → `== REVIVE_WEAKENED_TICKS`。**不允许只测 `is_damaged` 写入而不测 revive 读取**（防 is_damaged 写入无读取的半僵尸）

**饱和化测试声明**（`social::niche_defense::tests` + `social::mod::tests`）：
- happy path：完好灵龛被成功抄家 → `niche.is_damaged == true`（就地置位，无中间事件）+ registry 同步翻转
- 状态转换：`is_damaged` false→true（首次抄家）/ true→true（再次抄家幂等不报错）/ true→false（修补，见 P1-b）
- 边界：守卫全灭但藏物未被取走 → **不**触发损伤（断言损伤只挂抄家不挂守卫破损，与 `NicheGuardianBroken` 分离）
- serde：`SpiritNiche` 含/不含 `is_damaged` 字段两份 sample 反序列化都成功（旧存档兼容，default=false）
- 复活降级：受损灵龛复活 → 虚弱时长 == 完好灵龛的 2 倍（断言取既有虚弱 const 引用 ×2，不写字面秒数）

### P1-b `niche_repair_kit` use 闭环（server + 跨仓库契约）

**交付物**：

- schema：`server/src/schema/client_request.rs` 新增 `ClientRequestV1::SpiritNicheRepair { v: u8, x: i32, y: i32, z: i32, item_instance_id: u64 }`（仿 `:187` `SpiritNichePlace`），含 roundtrip + reject-unknown-fields 测试（`client_request::tests`，仿 `:764` coffin 系列）
- event：`server/src/social/events.rs` 新增 `SpiritNicheRepairRequest`（结构同接入面）；`mod.rs` `app.add_event::<SpiritNicheRepairRequest>()`（仿 `:166`）
- 请求映射：`server/src/network/client_request_handler.rs` 新增 `ClientRequestV1::SpiritNicheRepair` arm（仿 `:918` SpiritNichePlace），`dispatch.spirit_niche_repair_tx`（仿 `:273`）send `SpiritNicheRepairRequest`；resource 缺失走 warn-drop（**不留 emit-only 孤岛**）
- handler：`server/src/social/mod.rs` 新增 `fn handle_spirit_niche_repair_requests`（仿 `handle_spirit_niche_place_requests`）：① 找玩家背包内 `item_instance_id` 实例；② 校验 `template_id == "niche_repair_kit"`；③ 校验目标坐标命中玩家自己的 `is_damaged == true` 灵龛；④ 消耗 `niche_repair_kit`×1（`consume_item_instance_once`）；⑤ `is_damaged = false` + `registry.upsert` + `commands.entity().insert`；⑥ emit `SOCIAL_NICHE_REPAIR` VfxEvent
- client：`ClientRequestProtocol.encodeSpiritNicheRepair`（仿 `:674`，wire `"spirit_niche_repair"`）+ `ClientRequestSender.sendSpiritNicheRepair`（仿 `:164`）；玩家手持 `niche_repair_kit` 对受损灵龛交互触发

**饱和化测试声明**（`social::mod::tests`）：
- happy path：受损灵龛 + 背包持 `niche_repair_kit` → repair → `is_damaged == false` + kit 被消耗 + emit `SOCIAL_NICHE_REPAIR`
- 错误分支：① 对**完好**灵龛使用 → 拒绝（`is_damaged == false`，不消耗 kit）；② 背包无 `niche_repair_kit`（材料不足）→ 拒绝；③ 目标坐标非自己灵龛 / 非灵龛 → 拒绝；④ 持错误物品（非 repair_kit template_id）→ 拒绝
- 状态转换：受损→修补→完好，修补后再被抄家可再次受损（损伤-修补可循环）
- 跨仓库 pin：schema sample `spirit_niche_repair.json` 双端对拍（参 `agent/packages/schema/samples/` 约定）

### P1-c 视听规格（玩家可感知，内联本阶段）

**修补 SFX**（新建 `server/assets/audio/recipes/niche_repair.json`，结构参 `niche_establish.json`）：
```json
{
  "id": "niche_repair",
  "layers": [
    { "sound": "minecraft:block.smithing_table.use", "volume": 0.6, "pitch": 1.1, "delay_ticks": 0 },
    { "sound": "minecraft:block.stone.place",        "volume": 0.4, "pitch": 0.9, "delay_ticks": 3 }
  ],
  "priority": 74,
  "attenuation": "AREA",
  "category": "BLOCKS",
  "bus": "ENVIRONMENT"
}
```
由 `SOCIAL_NICHE_REPAIR` VfxEvent 触发（server send_spawn 时携带；client `AudioEventEnvelope.parsePlay` 解析后由 `SoundRecipePlayer` 播放，参 `client/.../network/AudioEventRouter.java`）。**放置侧正确**：本 recipe 走 server-driven 路径，故放 `server/assets/audio/recipes/`（与 `niche_establish.json` 同侧），字段用 **server 侧大写枚举** `"attenuation":"AREA"`/`"bus":"ENVIRONMENT"`（server 加载器期望大写，见 `server/assets/audio/recipes/niche_establish.json`）。**与 P1-d 客户端本地播放路径不同侧、不同值域**：P1-c 修补走服务端 emit（玩家放置/修补是 server 权威事件，server 已持 `SOCIAL_NICHE_REPAIR` VfxEvent 出口），P1-d 守卫破损/损耗走客户端本地（见 P1-d 架构核实），两者切勿混用值域。

**修补粒子**（client 新建 `NicheRepairParticlePlayer`，仿 `BotanyAuraPlayer` 的 `BongSpriteParticle` 子类，`client/.../season/SeasonParticleEmitter.java:204`）：
- 基类 `BongSpriteParticle`；数量 6 颗 burst；lifetime 14 tick；spawn 模式 radial（绕灵龛中心半径 0.6 格向上飘）；初速 [0, +0.04, 0]（缓升）；颜色 hex `#B8B0A0`（石屑灰）；贴图复用现有 sprite atlas 灰屑贴图（无则新建 `bong:particle/stone_mend`，1 张 8×8，走 `/gen-image particle`）；VfxPlayer 类名 `NicheRepairParticlePlayer`；触发 VfxEvent ID `bong:social_niche_repair`（`SOCIAL_NICHE_REPAIR`）
- 表意：碎石灵铁混合料"弥合"灵龛裂痕，灰屑向上聚拢

**narration**（修补成功，scope=player，style=perception，2 条任选/随机）：
- "龛石的裂纹被混合料填实，冷石重新泛起温润的微光。"
- "你将碎石灵铁糊进灵龛的伤口，它低低地'合'了一声，像是松了口气。"

### P1-d `NicheDefenseReactionVfxPlayer` 断链收口（顺带，调查 #8 / `reminder.md:25`）

**现状双断链**：`reactionIdsFor()`（`client/.../visual/NicheDefenseReactionVfxPlayer.java`）零调用方；返回的 `niche_guardian_broken` / `niche_guardian_fatigue` / `niche_intrusion_ink_mark` 三 ID 在 `client/.../assets/bong/audio_recipes/` 下**无 JSON 资产**。

**客户端音频架构核实（重要，实施前已 grep 取证，纠正原 B 方案的两处事实错误）**：
1. **客户端确有本地播放 API**：`SoundRecipePlayer.instance().play(new AudioEventPayload.PlaySoundRecipe(recipeId, instanceId, pos, loopFlag, volMul, pitchShift, recipe))` 可被任意 client Java callsite 直接调——`EnvironmentAudioController.java:69` 即如此本地拉起。`SoundRecipePlayer.INSTANCE`（`:33`）已在 `BongClient.bootstrap`（`:112`）初始化。**所以 `NicheIntrusionAlertHandler` 这个 callsite 能本地播放，原 B 方案的 callsite 可达性成立**。
2. **但客户端无"从 JSON 资产文件加载 recipe"的运行时路径**：`AudioRecipe` 在客户端**只能通过两条路构造**——(a) 服务端 over-the-wire 推来的 envelope 经 `AudioEventEnvelope.parsePlay()` 解析（`AudioEventRouter:13`，服务端驱动）；(b) Java 代码**程序化 new**（`EnvironmentAudioController.recipe()` `:105-114` 即手写 `new AudioRecipe(...)`）。`client/.../assets/bong/audio_recipes/*.json` 文件**不被客户端运行时加载**——它们是**测试夹具**（`SwordPathV2AudioRecipeAssetTest:48` 把 JSON 包成 wire envelope 跑 `parsePlay` roundtrip，证明该 recipe 能过服务端→客户端线协议），不存在客户端本地 JSON-recipe loader。**原 B 方案"新建 client audio_recipe JSON 然后本地播"是错的——本地播放必须程序化 `new AudioRecipe(...)`，JSON 文件只在服务端侧（`server/assets/audio/recipes/`）由服务端加载推送，或作为客户端 wire-roundtrip 测试夹具**。

**收口方案（采纳 B-修正：客户端本地程序化播放）**：
- 弃用 `NicheDefenseReactionVfxPlayer`（标 `@Deprecated` 或删类，调研确认零复用价值）。
- 在 `NicheIntrusionAlertHandler.recordGuardianBroken`（**实测 `:52`**，原 plan 写 `:49` 有误）/ `recordGuardianFatigue`（**实测 `:37`**，原 plan 写 `:34` 有误）的既有 `UnifiedEventStore.stream().publish(...)` **之后**，紧跟一行 `SoundRecipePlayer.instance().play(new AudioEventPayload.PlaySoundRecipe("niche_guardian_broken"/"niche_guardian_fatigue", <instanceId 自增>, Optional.empty()/Optional.of(playerPos), Optional.empty(), 1.0f, 0.0f, <程序化 AudioRecipe>))`。
- **程序化 `AudioRecipe` 字段（用客户端 enum 字面，不照抄任何 JSON）**——`new AudioRecipe(id, layers, Optional.empty()/*非 loop*/, priority, AudioAttenuation.WORLD_3D, AudioCategory.BLOCKS, AudioBus.ENVIRONMENT)`：
  - `niche_guardian_broken`：layers `[new AudioLayer(new Identifier("minecraft","block.stone.break"), 0.7f, 0.7f, 0), new AudioLayer(new Identifier("minecraft","entity.wither.shoot"), 0.3f, 1.4f, 2)]`，priority 78
  - `niche_guardian_fatigue`：layers `[new AudioLayer(new Identifier("minecraft","block.grindstone.use"), 0.5f, 0.8f, 0)]`，priority 72
- **可选**（若想与既有体系一致、便于将来改服务端驱动）：另建两份 wire-roundtrip 测试夹具 `client/.../audio_recipes/niche_guardian_broken.json` / `niche_guardian_fatigue.json`，**字段必须用客户端 wire 取值**——`"attenuation":"world_3d"`（小写 wire 形，`AudioAttenuation.fromWire` 仅接受 `world_3d`/`player_local`/...，**不接受 `niche_establish.json` 的大写 `AREA`**，也**不接受 entity_spider_step.json 的 `"world"`**——后者其实是 client 侧 latent 坏夹具，`fromWire("world")` 返回 null 会让 parse 失败，**严禁照抄**）、`"category":"BLOCKS"`、`"bus":"ENVIRONMENT"`，跑 `parsePlay` roundtrip 测试（仿 `SwordPathV2AudioRecipeAssetTest`）。

**测试声明**：
- 断链回归（核心）：单测断言 `NicheIntrusionAlertHandler.recordGuardianBroken/Fatigue` 调用后 `SoundRecipePlayer` 收到对应 recipeId 的 `play(...)`（注入 mock SoundRecipePlayer 或 spy；不再是纯字符串 publish）——**直接锁住"事件→本地音频播放"链路，而非资产文件存在性**
- 若采纳可选 JSON 夹具：wire-roundtrip 断言两 JSON 经 `AudioEventEnvelope.parsePlay` 成功解析、`attenuation`/`category` 非 null（防大写/小写值域踩坑回归）

> **schema 值域差异备忘**（避免照抄踩坑）：server `audio/recipes/*.json` 由 server 加载，`attenuation`/`bus` 用**大写**枚举（`AREA`/`ENVIRONMENT`，见 `niche_establish.json`）；client wire envelope 经 `AudioAttenuation.fromWire` 期望 **`attenuation` 小写 wire 形**（`world_3d`/`player_local`，大写 `AREA`/`WORLD` 也被 `fromWire` 兼容但 client 既有夹具惯用小写），`category` 大写 `valueOf`（`BLOCKS`/`PLAYERS`/`HOSTILE`），`bus` 大写 `valueOf` 且**仅 `COMBAT`/`ENVIRONMENT`/`UI` 三值**（entity_spider_step.json 的 `"bus":"SFX"` 是无效值，`fromWire` 返回 null——所幸 bus 字段 optional 才没炸）。本 plan 客户端侧一律用 `world_3d` / `BLOCKS` / `ENVIRONMENT`。

---

## §8 开放问题（原表保留以备追溯，**实施时以 §8.1 决议为准**）

1. 断链方向：handler 认 `niche_base`（A）vs 配方改产 `spirit_niche_stone`（B）
2. 损坏态行为：复活功能完全停用 vs 降级
3. 灵龛 HP 模型：单 bool vs 多档 enum
4. `NicheDefenseReactionVfxPlayer` 收口：补调用（A）vs 弃类直播 audio（B）

全部已在 §8.1 收口。

## §8.1 决议（pre-P0 收口，2026-06-10）

### #1 断链方向 → 选 A（handler 认 `niche_base`）

**决议**：
1. `niche_base` 为最终放置品，`spirit_niche_stone` 降为纯制作材料（配方 #98 的消耗料之一）。
2. 改 `social/mod.rs:80` const 为 `"niche_base"`；全仓 grep `"spirit_niche_stone"` 字面量收口——确认 `inventory/mod.rs:5309`（测试断言）/ `craft/workbench_recipes.rs:1344`（配方消耗料，保留）/ `world/spawn_tutorial.rs:43,442,896,1098`（开棺起始 grant，**改为 `niche_base`**）各点都已处理。
3. 拒绝 B：B 需删 `niche_base` 配方 #98 + 清理 `workbench_materials.toml:918` 模板 + 改产物，改动点分散且违背 `niche_base` description "永久复活点基座" 语义；worldview §十一"消耗一次性道具「龛石」"中"龛石"=`spirit_niche_stone`，制成基座再放置语义不破坏。

**落点**：`server/src/social/mod.rs:80`（const）/ `:1537`（warn 文案）/ `:2920` 附近 dev give / `server/src/world/spawn_tutorial.rs:43,442`（开棺起始 grant 改 `niche_base`）/ plan §P0

### #2 损坏态行为 → 降级（不停用）

**决议**：
1. 受损灵龛**仍可复活**（不停用），但复活后"虚弱"debuff 时长翻倍。
2. 复用 combat 既有虚弱机制（worldview §十二 重生本带 3 分钟虚弱 = `combat/components.rs:17` `REVIVE_WEAKENED_TICKS = 180*TICKS_PER_SECOND`），受损态 ×2，**取 const 引用不写字面**。
3. **跨模块 wiring 是真正难点**（已 grep 取证）：虚弱在 combat（`Lifecycle::revive` `combat/components.rs:242` 硬编 `REVIVE_WEAKENED_TICKS`），`is_damaged` 在 social；`revive_lifecycle`（`combat/lifecycle.rs:1269`）无 SpiritNiche/registry 入参，`spawn_anchor`（`:1376`）裸坐标无法反查 niche。方案见 §P1-a 三步（改 `Lifecycle::revive` 加 multiplier + 给 revive 链注入 social 损伤态，或在 `Lifecycle` 加 `spawn_anchor_damaged` bool 由 social 同步翻转避免 combat→social 反向依赖成环），并加 revive 读取测试（断言虚弱时长 ×2，防 is_damaged 写入无读取）。
4. 拒绝完全停用：停用会让玩家在被抄家后失去复活点回退到出生点，惩罚过重且与"养伤"语义冲突；降级（重伤复活）更贴合"灵龛受损但仍庇护"。

**落点**：`server/src/social/components.rs:228`（`is_damaged` 字段）/ `server/src/combat/components.rs:242`（`Lifecycle::revive` 加 multiplier）+ `:17`（`REVIVE_WEAKENED_TICKS` 基准）/ `server/src/combat/lifecycle.rs:1269`（`revive_lifecycle` 注入损伤态）+ `:1293,1344,1376`（revive 调用 + spawn_anchor）/ plan §P1-a

### #3 灵龛 HP 模型 → 单 `is_damaged: bool`

**决议**：
1. niche-defense **未在 SpiritNiche 加任何 HP 字段**（`components.rs:228-236` 已确认），本 plan 从零加单 bool。
2. 单 bool（完好/受损）足够支撑 §8.1 #2 的二态降级；多档 enum 无对应玩法需求，YAGNI。
3. `#[serde(default)]` 保证旧存档反序列化 default=false。

**落点**：`server/src/social/components.rs:228` / plan §P1-a

### #4 `NicheDefenseReactionVfxPlayer` 收口 → 选 B（弃类直播 audio）

**决议**：
1. `reactionIdsFor()` 零调用 + 无资产（双断链，调研确认），保留无价值。
2. 弃用该类，改在 `NicheIntrusionAlertHandler`（实测 `recordGuardianFatigue`=`:37` / `recordGuardianBroken`=`:52`，**原写 `:34`/`:49` 已纠正**）既有 publish 处之后，调 `SoundRecipePlayer.instance().play(...)` 本地播放。
3. **音频以程序化 `new AudioRecipe(...)` 构造**（客户端无 JSON-recipe 运行时 loader，详见 §P1-d 架构核实）；JSON 文件仅作可选 wire-roundtrip 测试夹具，字段用客户端 wire 取值（`world_3d`/`BLOCKS`/`ENVIRONMENT`），**禁止照抄 entity_spider_step.json 的 `"world"`/`"SFX"`（latent 坏夹具）或 server 侧大写 `AREA`**。

**落点**：`client/.../visual/NicheDefenseReactionVfxPlayer.java`（弃用）/ `client/.../social/NicheIntrusionAlertHandler.java:37,52`（接入本地播放）/ `client/.../audio/SoundRecipePlayer.java:72`（`play` API）/ `client/.../audio/AudioRecipe.java`（程序化构造）/ plan §P1-d

---

## §10 实施工作流

本 plan scope = 2 PR（P0 + P1），低于 §六 的 4 PR 门槛，§10 取轻量版（保留 subagent + CR 等待要点，省多 PR 序列化模板）。

### §10.1 PR 拆分点

- **PR-1（P0）**：纯 server，const 改值 + warn 文案 + 断链回归测试。改动极小，独立成 PR 让 review 聚焦"ID 收口"。前置 merge 后再开 PR-2。
- **PR-2（P1）**：server 损伤就地置位（`niche_defense.rs` component+registry+持久化三同步）+ 复活降级跨模块 wiring（combat `Lifecycle::revive` multiplier + revive 链读损伤态）+ 修补 handler + schema variant + client 协议/sender + P1-c server 修补 SFX recipe + 粒子 player + P1-d 客户端本地音频播放断链收口（`NicheIntrusionAlertHandler`→`SoundRecipePlayer.play`）。依赖 PR-1（同改 `mod.rs` 灵龛路径，避免 merge conflict）。

### §10.2 subagent 配置

每 PR 起独立 subagent（context 隔离，主线只收 result）：
```
Agent(subagent_type:"claude", model:"opus", prompt:"<本 PR 范围 + §10.3 测试要求>\n\nultrathink")
```
- PR-2 含视听资产（audio_recipe JSON + 粒子贴图）：贴图若需新建走 `/gen-image particle`，audio_recipe 为纯 JSON 逻辑资产**不适用 3 轮 PROMISE**（非 NBT 建筑/layout/复杂视觉资产，按常规 atomic commit + 资产存在性测试全绿即可）。

### §10.3 测试要求

每 PR 测试必须覆盖 happy path + 所有边界 + 所有错误分支 + 所有状态转换（见各阶段块饱和化测试声明）。server 跑 `cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`；client 跑 `cd client && ./gradlew test`；schema 改动跑 `cd agent/packages/schema && npm test`。

### §10.4 CodeRabbit 等待协议

`gh pr checks <PR>` 看状态：`pass`→merge；`pending`→`ScheduleWakeup delaySeconds=1200`（≤3 回合 = 60min 卡死交人工）；`fail`→按严重性桶处理。修完 review 必须重等 CR re-review，不自判已过。PR-1 收敛/APPROVED 前不开 PR-2。

### §10.5 单次 consume-plan 全自动到 merge

用户提交 `/consume-plan plan-niche-craft-fix-v1` 后即可下班，醒来看 plan 是否在 `docs/finished_plans/`。

## Finish Evidence

### 落地清单

- **P0：`niche_base` 放置闭环**：
  - `server/src/social/mod.rs` 将 `SPIRIT_NICHE_ITEM_TEMPLATE_ID` 收口为 `niche_base`，放置成功写入 `Lifecycle.spawn_anchor`、`SpiritNicheRegistry` 与持久化。
  - `server/src/world/spawn_tutorial.rs` 将新手起始灵龛 grant 改为可直接放置的 `niche_base`。
  - `server/src/social/mod.rs` 增加 `niche_base_template_id_is_recipe_output` 等断链回归，锁住配方产物与 handler 可接受 ID 一致。
- **P1：损伤/修补生命周期**：
  - `server/src/social/components.rs` 为 `SpiritNiche` 增加 `is_damaged`；`server/src/persistence/mod.rs` 增加 `social_spirit_niches.is_damaged` 迁移、读写与回归。
  - `server/src/social/niche_defense.rs` 在成功抄家后就地置损伤，并同步 component / registry / persistence / `Lifecycle.spawn_anchor_damaged`。
  - `server/src/combat/components.rs` 与 `server/src/combat/lifecycle.rs` 接入受损灵龛复活虚弱时长翻倍，测试断言使用 `REVIVE_WEAKENED_TICKS * 2`。
  - `server/src/social/events.rs`、`server/src/network/client_request_handler.rs`、`server/src/schema/client_request.rs`、`proto/bong/envelope.proto` 接通 `SpiritNicheRepair` C2S 请求与事件。
  - `server/src/social/mod.rs` 增加 `handle_spirit_niche_repair_requests`，校验 `niche_repair_kit`、自有受损灵龛、距离与实例 ID，成功后消耗修补料、清除损伤并发出 VFX/SFX/narration。
  - `server/assets/audio/recipes/niche_repair.json` 与 `server/src/network/gameplay_vfx.rs` 增加修补音效和 `bong:social_niche_repair` 粒子事件。
- **P1：client / schema 接线**：
  - `client/src/main/java/com/bong/client/inventory/InspectScreen.java` 增加 `niche_repair_kit` 右键菜单与 `ClientRequestSender.sendSpiritNicheRepair`。
  - `client/src/main/java/com/bong/client/network/ClientRequestProtocol.java`、`ClientRequestSender.java` 增加 `spirit_niche_repair` 编码/发送。
  - `client/src/main/java/com/bong/client/visual/particle/NicheRepairParticlePlayer.java` 与 `VfxBootstrap.java` 注册修补粒子。
  - `client/src/main/java/com/bong/client/social/NicheIntrusionAlertHandler.java` 将守家破损/损耗本地程序化音频播放接到真实 callsite，并将旧 `NicheDefenseReactionVfxPlayer` 标记弃用。
  - `agent/packages/schema/src/client-request.ts`、`schema-registry.ts`、`generated/client-request-spirit-niche-repair-v1.json` 与 sample 文件补齐 `SpiritNicheRepairRequestV1`。

### 关键 commit / PR

- `d5142417981beab1aee5e387ba779edab466922e`（2026-06-10）— `plan-niche-craft-fix-v1 P0: 接通 niche_base 灵龛放置`，PR #478。
- `2c75e7e208bd0ccaaf02343c2c6018c67ca23e18`（2026-06-10）— `plan-niche-craft-fix-v1 P1: 灵龛损伤修补闭环`，PR #480。

### 测试结果

- PR #478：GitHub `e2e` 通过（E2E Redis Smoke，job `80491916091`）；GitHub `snapshot` 通过（Worldgen Preview Snapshot，job `80491916185`）。CodeRabbit 为额度失败，非代码 blocker。
- PR #480：本地通过 `cd agent/packages/schema && npm test`（595 passed）、`cd agent && npm run build -w @bong/schema`、`cd server && cargo fmt && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`（8126 passed, 0 failed, 1 ignored）、`JAVA_HOME=$HOME/.sdkman/candidates/java/17.0.18-amzn cd client && ./gradlew test build`。GitHub `e2e` 通过（E2E Redis Smoke，job `80519271178`）。CodeRabbit 为额度失败，非代码 blocker。

### 跨仓库核验

- **server**：`SPIRIT_NICHE_ITEM_TEMPLATE_ID = "niche_base"`、`SpiritNiche.is_damaged`、`Lifecycle.spawn_anchor_damaged`、`handle_spirit_niche_repair_requests`、`SOCIAL_NICHE_REPAIR`、`niche_repair` audio recipe 均可 grep 命中。
- **client**：`ClientRequestProtocol.encodeSpiritNicheRepair`、`ClientRequestSender.sendSpiritNicheRepair`、`InspectScreen.isSpiritNicheRepairKit`、`NicheRepairParticlePlayer.EVENT_ID`、`NicheIntrusionAlertHandler` 本地守家音频播放均可 grep 命中。
- **agent/schema**：`SpiritNicheRepairRequestV1`、`clientRequestSpiritNicheRepairV1`、`client-request.spirit-niche-repair.sample.json` 与 generated schema 均可 grep 命中。
- **proto**：`ClientRequestEnvelope.Payload.spirit_niche_repair = 92` 与 `message SpiritNicheRepair` 已落地。

### 遗留 / 后续

- 无本 plan 范围内遗留阻塞；后续若要让已揭露/已拆除灵龛也可被修补，需要另立玩法 plan 明确语义。
