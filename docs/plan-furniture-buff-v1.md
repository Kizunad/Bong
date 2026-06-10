# plan-furniture-buff-v1 — 家具/光源放置与安居恢复 buff

> 一句话：放置类僵尸物品「家具光源建造」消杀——纯方块 4 个（火把/灯笼/门闩/窗栅）直接落地放置，家具 4 个（床/蒲团/灵石架/防潮架）放置后在小范围内给玩家「养伤/歇息」恢复加速 buff，并按用户硬要求加限制（同效不叠 / 半径小 / 静止生效 / 多放无收益）。
>
> 它从 `world/block_place.rs` 放置管线 + lifecycle P4 的 client `interactBlock` 放置分支进料，向 `combat`（新增 `HealthRegenBoost` 恢复体系 + 激活死字段 `healing_rate_multiplier`）/ `cultivation`（蒲团复用 `CultivationAcceleration`；**`practice_session_tick` 不接活——守恒红旗，留专门 plan，见 §8.1 #6**）/ `shelflife`（防潮架接保鲜——**依赖世界放置容器 entity 未落地，本 plan 降级 follow-up**）出料，对应 worldview §十一:916「灵龛只能藏东西和养伤」+ §十四「这个世界的一天」作息恢复 + §九 骨币经济。
>
> 来源：放置类 17 调查 workflow 2026-06-10（opus 抽查 7/7 证据属实）；用户拍板：家具给体力/血量恢复 +x% 并限制。

**依赖**：plan-block-lifecycle-v1 P4（client 放置 wiring）。**实测核查（2026-06-10）**：`ClientRequestSender.sendBlockPlace`（`ClientRequestSender.java:156`）+ mixin 放置分支（`MixinClientPlayerInteractionManagerAlchemy.java:126`）已在 main，P4 核心 wiring 已就位。本 plan P0 不需要等 lifecycle P4 整个 PR 合入——只差 `BlockVanillaIconMap` 扩条目 + TOML `category` 改写（见 §8.1 #5）。P0 实施前 grep 确认 lifecycle P4 worktree 分支与 main 的精确差异即可。

| 阶段 | 主题 | 状态 |
|------|------|------|
| P0 | 纯方块 4 个落地（category 改写 + vanilla state 映射 + client icon） | ⬜ |
| P1 | FurnitureRegistry + 家具方块放置（bong_blocks 扩） | ⬜ |
| P2 | 血量自然恢复 tick（从零建）+ `HealthRegenBoost` 变体 | ⬜ |
| P3 | 床/蒲团 aura buff + 限制 + 视听 | ⬜ |
| P4 | 防潮架接保鲜 / 灵石架降级装饰 / 收尾验收 | ⬜ |

---

## 接入面（防孤岛 checklist）

- **进料**：
  - 放置管线：`world/block_place.rs:65` `handle_block_place_requests`（消费 `BlockPlaceRequest` 的主 system）+ `:212` category 闸一（`template.category != ItemCategory::Block` 拒）+ `:220` 闸二（`block_item_to_state(...).is_none()` 拒）+ `:243-253` `block_item_to_state`（现仅 6 条 vanilla 映射）+ `:284` `write_block_state`（bong/vanilla 分流）
  - 方块常量：`world/terrain/blocks.rs:63` `BlockState::TORCH` / `:203` `BlockState::LANTERN` / `:200` `BlockState::IRON_BARS`（窗栅 vanilla 占位）
  - client wiring：`ClientRequestSender.java:156` `sendBlockPlace`（已落，**本 plan 不重复实现**）+ `BlockVanillaIconMap.java:22-29` `HOST_ITEMS`（现 6 条，`isKnownBlockItem` 控制 mixin `bong$selectedBlockItem` 过滤——4+4 个目标物品全不在列，client 放置链对它们完全断开，**P0/P1 必须扩条目**）+ `MixinClientPlayerInteractionManagerAlchemy.java:126` 放置分支
  - 恢复系统先例：`combat/lifecycle.rs:207` `stamina_tick`（体力自然恢复，`health_regen_tick` 结构先例）+ `combat/lifecycle.rs:158` `wound_bleed_tick`（每 tick 减 `Wounds.health_current`，P2 必须显式与之分优先级）；血量**无自然恢复 tick**（全仓 grep 无 `health_regen`）
  - 血量数据落点：`combat/components.rs:67` `Wounds { health_current, health_max }`（HP 是物理量，非真元，不走 qi ledger）
  - status 乘区先例：`combat/status.rs:278-343` `StaminaRecovBoost` 乘区计算（读 `StatusEffects.active` 取 magnitude 折叠成 multiplier，P2 `HealthRegenBoost` 照此模式）
  - 死字段：`combat/components.rs:297` `DerivedAttrs.healing_rate_multiplier`（default 1.0，`baomai_v4/scar_circuit.rs:227-228` SpleenKidney 写入但全仓无读取——P2 在 `health_regen_tick` 接入乘区即激活，对齐 reminder.md:22）
  - 打坐：`cultivation/practice_session.rs:69` `practice_session_tick` / `:85` `check_practice_session_exit`（两者 `#[allow(dead_code)]`，mod.rs:66 仅 `pub mod`，调用全在自身测试内）——**本 plan 不接它**：是纯叶子函数（裸值参数无 Query/Res）+ 第 78 行直扣 `*current_qi` 无 zone credit 不走 ledger（守恒红旗），蒲团改走既有 `CultivationAcceleration`（守恒安全）。接活留专门 plan，见 §8.1 #6 + reminder.md
  - aura 范围先例：`craft/workbench.rs:47` `is_within_workbench_range`（Chebyshev 3 格，P3 furniture aura 参照，半径收小到 2 格）
  - registry 先例：`zhenfa/mod.rs:405-414` `ZhenfaRegistry`（`HashMap<[i32;3], u64>` 双向索引，`FurnitureRegistry` 简化为单层）
  - 保鲜接口：`shelflife/compute.rs:33` `combine_storage_and_zone_multiplier(storage_multiplier, zone_qi_density)` + `shelflife/container.rs:30` `container_storage_multiplier`（`Freeze` → time-based 0.0 / **Stepwise 1.0**）——防潮架"湿度衰减归零"=把容器 `ContainerFreshnessBehavior` 升级为 `Freeze`。**但 shelflife 无世界坐标/范围概念（item-in-container 纯函数），"范围内的容器"需世界放置容器 entity（归 `plan-placeable-container-blocks-v1`，仍 skeleton 未落地）→ 本 plan 防潮架保鲜降级 follow-up，见 §8.1 #3**
  - 修炼加速消费端：`cultivation/tick.rs:181,327-331` `cultivation_acceleration_multiplier`（从 `StatusEffects` 读 `CultivationAcceleration` 叠乘——蒲团 P3 走它，无需新通道）
- **出料**：
  - 新 `FurnitureRegistry`（resource，仿 `ZhenfaRegistry`，`HashMap<[i32;3], FurnitureKind>`；放置登记 / 破坏移除）——落 `server/src/world/furniture.rs`（新文件）
  - 新 `combat/lifecycle.rs::health_regen_tick`（基准 HP 恢复系统）+ `StatusEffectKind::HealthRegenBoost`（`combat/events.rs` 新变体）
  - buff 施加一律 `combat/events.rs:160` `ApplyStatusEffectIntent`（**禁止直写 `DerivedAttrs` / `Stamina.recover_per_sec`**——`movement/mod.rs:183-187` `sync_stamina_regen_from_realm` 每帧硬写会覆盖直写值；HP 恢复走 `health_regen_tick` 读 status 乘区，绕开此风险）
  - 激活死字段 `healing_rate_multiplier`：从静默失效→ SpleenKidney 经脉养伤加成生效
  - `moisture_base` 接 shelflife：**本 plan 降级为 follow-up**（依赖 `plan-placeable-container-blocks-v1` 世界放置容器 entity，仍 skeleton 未落地）；实装时改容器实例 `ContainerFreshnessBehavior` 为 `Freeze`（非直写 storage_multiplier）。见 §8.1 #3 + P4 + reminder.md
- **共享类型 / event**：复用 `StatusEffectKind` 乘区体系（新增 `HealthRegenBoost` 一个变体）；蒲团复用既有 `CultivationAcceleration`（`events.rs:147`，**不另造 QiRegenBoost 路径**——`alchemy/pill.rs:1041-1042` 已定调"修炼加速语境统一归 CultivationAcceleration"）；放置复用 lifecycle `BlockPlaceRequest`，**不另造放置协议**
- **跨仓库契约**：
  - server：`BlockPlaceRequest` consumer（`block_place.rs:65`，已落）；`block_item_to_state` 扩 8 条（P0 4 条 vanilla + P1 4 条 bong arm）；`StatusEffectKind::HealthRegenBoost` 新变体的客户端 wire 序列化点是 `server/src/network/status_snapshot_emit.rs` 的 6 个穷尽 match（**实测：`proto_convert.rs` / `combat_bridge.rs` 对 `StatusEffectKind` 引用数均为 0，不是 wire 锚点**）：`status_effect_id`(:58)/`status_effect_name`(:75)/`status_effect_category`(:118)/`status_effect_source_label`(:161)/`status_effect_color`(:175)/`status_effect_dispel`(:185)——其中 source_label(:171)/color(:181)/dispel(:190) 用 `_ =>` 兜底，新增变体不补显式 arm 会被静默归默认（source_label→"战场丹药"、color→灰 0xFFA0A0A0、dispel→1），HUD 事件流对新 buff 展示错误。具体取值见 P2
  - client：`BlockVanillaIconMap.HOST_ITEMS` 扩 8 条（`Map.of` 上限 10，6+8=14 → 必须改 `Map.ofEntries`）；放置走 lifecycle P4 的 `interactBlock` 分支；**P1 的 4 个 bong 家具方块须按 README 6 步在 `client/.../block/BongBlocks.java` 注册 + `./gradlew generateBongBlockIds` 对齐 `BongBlockIds`（跨仓库契约 client 面 symbol：`BongBlocks` 注册项 / `BongBlockIds` 对齐 / `BongBlocksTest`），否则 server 有 state 而 client 渲染/raw-id 不对齐**；P3 视听走 `bong:vfx_event` channel（新增两个 VfxPlayer）
  - agent：**不参与**（家具放置/恢复 buff 不入天道推演链）
- **worldview 锚点**：§十一:916「灵龛不提供灵气——你不能在里面修炼，只能藏东西和养伤」（家具是玩家**主动布置**的养伤设施，与"灵龛不自动给恢复"正典兼容）；§十四「这个世界的一天」（灵龛醒来→出门修炼的作息循环，床/蒲团服务于此）；§十一:915「灵龛方圆 5 格内…其他玩家无法破坏方块」（门闩/窗栅在 NPC 破门系统未实装下=物理屏障，§8.1 #4 已定案）；§九:838 封灵骨币（凡物家具定价锚）
- **qi_physics 锚点**：**无新增通道**。HP/体力是物理量，`health_regen_tick` 改的是 `Wounds.health_current`，不走 qi ledger、不需守恒处理。蒲团给 `CultivationAcceleration`，其底层消费 `cultivation/tick.rs:88` `qi_regen_and_zone_drain_tick` 已走 `qi_physics::regen_from_zone` 守恒链，本 plan 不引入任何 `*_DECAY*` / `*_DRAIN*` 常数或衰减函数。**守恒红旗规避**：`practice_session_tick`（`practice_session.rs:78`）自带一条 `*current_qi -= cost` 的 qi 流——**无 zone credit、不走 ledger**，违反 worldview §二/§十守恒律；本 plan **明确不接它**（蒲团只走守恒安全的 CultivationAcceleration），接活留专门 plan 并届时补 ledger 归还路径（扣 qi 必有 zone 等额变化），见 §8.1 #6。

---

## P0 — 纯方块 4 个落地 ⬜

**目标**：`torch_item`/`lantern_item`/`door_bolt`/`window_grate` 从"category=misc 被放置闸拒"变成可右键放置的实体方块。

- **TOML**（`server/assets/items/workbench_materials.toml`）：`torch_item`(:852)/`lantern_item`(:863)/`door_bolt`(:874)/`window_grate`(:907) 的 `category = "misc"` → `category = "block"`
- **server `block_item_to_state`**（`world/block_place.rs:243`）加 4 条 vanilla 映射：
  - `"torch_item" => BlockState::TORCH`（光照自动传播，`blocks.rs:63` 已用此常量）
  - `"lantern_item" => BlockState::LANTERN`（光照 15，`blocks.rs:203`）
  - `"door_bolt" => BlockState::IRON_DOOR`（门闩=可阻挡的铁门占位；按 §8.1 #4 实体屏障即可）
  - `"window_grate" => BlockState::IRON_BARS`（窗栅=铁栏杆占位，`blocks.rs:200`）
- **client `BlockVanillaIconMap`**（`BlockVanillaIconMap.java:22`）：`Map.of` → `Map.ofEntries`，加 4 条 host item（`torch_item`→`minecraft:torch`、`lantern_item`→`minecraft:lantern`、`door_bolt`→`minecraft:iron_door`、`window_grate`→`minecraft:iron_bars`），使 `isKnownBlockItem` 返回 true、mixin 放置链打通
- **测试**（server）：
  - `block_item_to_state_maps_all_v1_block_items`（`block_place.rs:334`）扩 4 个 case（happy path：每物品→对应 BlockState）
  - 边界：category 改写后 4 物品 `grid_w/grid_h/weight` 回归（确保改 category 不破坏库存网格 pin 测试）
  - 错误分支：保留 `block_item_to_state_rejects_non_placeable_materials`（`:354`）红线——非放置物仍返回 None
  - e2e：`give torch_item` → client 放置请求 → `handle_block_place_requests` 落方块 → `block_break` 掉落（对齐 lifecycle 的放置/破坏 e2e 模式）
- **测试**（client）：`BlockVanillaIconMap` 4 条新 host item `createStackFor` 非空 + `isKnownBlockItem` true（参 `ClientRequestSenderTest.java:231` 既有 sendBlockPlace 测试）
- **视听**：复用 vanilla 方块放置（torch/lantern 自带原版光照与放置音），P0 无需新增 VFX/SFX——纯 vanilla state 占位，玩家可感知行为完全由原版方块承载。

## P1 — FurnitureRegistry + 家具方块放置 ⬜

**目标**：`simple_bed`/`meditation_mat`/`moisture_base`/`spirit_stone_rack` 放置为带坐标登记的家具方块，供 P3/P4 的 aura/保鲜扫描查询。

- **TOML**：4 个家具物品（`meditation_mat`:454 / `spirit_stone_rack`:520 / `moisture_base`:885 / `simple_bed`:896）`category = "misc"` → `category = "block"`
- **多格表示决策**（§8.1 #2 已定案）：**P1 一律按 1×1 单格方块落地**——`simple_bed`(grid 2×2)/`moisture_base`(grid 2×1) 在世界里占单格"床头/底座"，多格联动方块（bed_head+bed_foot）留 v2。这避免 `bong_blocks` 多格支持缺位的阻塞，且让 `FurnitureRegistry` 坐标键保持单 `[i32;3]`
- **bong_blocks 扩（按 `server/crates/valence_generated_bong/README.md` 官方 6 步，4 个 bong 方块）**——**实测：`world/bong_blocks.rs` 无 `enum BongBlock`，bong 自定义方块是保留 raw-id 区间内的 `BlockState`（`BONG_BLOCK_STATE_START..=BONG_BLOCK_STATE_END`，`bong_blocks.rs:5-6`），不存在可加 variant 的 Rust enum，原"新 BongBlock 变体"措辞错误**。正确流程：
  1. `bong_blocks.json` 追加 4 条定义（codegen 后产出 `BlockState::BONG_*` 常量 + `BlockKind::Bong*` 变体，由 file order 分配 raw-state id）
  2. `cargo test` in `server/crates/valence_generated_bong` 验生成常量
  3. **客户端 `client/src/main/java/com/bong/client/block/BongBlocks.java` 注册 4 个方块**（跨仓库契约 client 面，原骨架漏）
  4. 客户端 `assets/bong/blockstates|models/block|textures/block` 资产（P1 可先用占位，bbmodel 真长相 P4 收尾）
  5. **`cd client && ./gradlew generateBongBlockIds test --tests com.bong.client.block.BongBlocksTest`（Java 17）对齐客户端 `BongBlockIds`——原骨架漏，不做会出 server 有 state、client 渲染/ID 不对齐的方块**
  6. 经 `world::bong_blocks::place_bong_block` 落地验证（Fabric client 无 raw ID mismatch）
- `block_item_to_state`(`block_place.rs:243`) 加 bong arm（4 条 → `BlockState::BONG_*`）；`write_block_state`(`:284`) 的 `is_bong_block` 分流（`bong_blocks.rs:52` raw-id 区间判定）自动走 `place_bong_block`
- **新 `FurnitureRegistry`**（`server/src/world/furniture.rs` 新文件）：
  - `enum FurnitureKind { SimpleBed, MeditationMat, MoistureBase, SpiritStoneRack }`
  - `struct FurnitureRegistry { by_pos: HashMap<[i32;3], FurnitureKind> }`（仿 `ZhenfaRegistry` `zhenfa/mod.rs:405`，单层即可——家具无 entity 行为，只需坐标→kind 查询）
  - `fn register(pos, kind)` / `fn remove(pos) -> Option<FurnitureKind>` / `fn kinds_in_range(center, radius) -> impl Iterator`（P3 aura / P4 保鲜共用）
  - 放置 system（hook 进 `handle_block_place_requests` 落方块后）：若 template→FurnitureKind 命中则 `registry.register`；破坏 system（hook block_break）：`registry.remove`
- **测试**：
  - registry 增删一致性：register 后 `kinds_in_range` 命中、remove 后不命中
  - 每个 `FurnitureKind` 变体专属 pin 测试（template_id → kind 映射 4 条，含未知 template 返回 None 的错误分支）
  - 状态转换：同坐标 register→remove→register（覆盖 re-place）
  - 重启持久化：对齐 lifecycle 的方块持久化机制（放置→存盘→重启 registry 重建——若 lifecycle 持久化由方块状态驱动，registry 在加载时扫 `FurnitureKind` 方块重建）
  - 客户端（按 README 第 2/5 步）：`server/crates/valence_generated_bong` `cargo test` 验 `BlockState::BONG_*` 常量；`client` `BongBlocksTest` 验 4 方块注册 + `BongBlockIds` raw-id 与 server 对齐（防 raw ID mismatch）
- **视听**：放置走 P3 各家具 bbmodel/vanilla 占位 + vanilla 放置音（bbmodel 资产产出见 §8.1 #2 与 §10——P1 先用 bong_blocks 默认渲染，bbmodel 留 P4 收尾或 follow-up）。

## P2 — 血量自然恢复 tick（从零）+ HealthRegenBoost ⬜

**目标**：从零建血量自然恢复系统（现仅有流血减血、无回血），并新增 `HealthRegenBoost` 状态变体供 P3 床 aura 施加。

- **新 `health_regen_tick`**（`combat/lifecycle.rs`，仿 `stamina_tick:207` 结构）：
  - 每 `HEALTH_REGEN_TICK_INTERVAL_TICKS` 触发（建议复用 stamina/bleed 同量级间隔）
  - 基准恢复速率 `BASE_HEALTH_REGEN_PER_SEC`（§8.1 #1：0.5 HP/s 量级慢回，clamp 到 `Wounds.health_max`）
  - 乘区：`effective = base * healing_rate_multiplier(DerivedAttrs) * health_regen_boost_multiplier(StatusEffects)`，**激活死字段** `healing_rate_multiplier`（`components.rs:297`）
  - **流血压制（§8.1 #1 关键边界，研究 #4 红旗）**：显式判断 `Wounds.entries` 是否有 `bleeding_per_sec > 0` 的活跃流血——有则本 tick 不回血（不靠系统排序自然成立，必须代码内 explicit short-circuit）
  - NearDeath/Terminated lifecycle 不回血（对齐 `wound_bleed_tick:175`）
- **`StatusEffectKind::HealthRegenBoost`**（`combat/events.rs:81` 新变体，附 doc 注释 `/// plan-furniture-buff-v1 P2：养伤恢复加速，magnitude N → HP 回复 ×(1+N)。`）：
  - `combat/status.rs` 加乘区分支：仿 `StaminaRecovBoost` 的 magnitude 折叠（`status.rs:323-331` 模式），但**不直写**，由 `health_regen_tick` 读 `StatusEffects.active` 计算 `health_regen_boost_multiplier`
  - **wire 映射（实测锚点：`server/src/network/status_snapshot_emit.rs` 6 个穷尽 match，非 proto_convert/combat_bridge——后两者对 StatusEffectKind 引用数为 0）**，须逐个补 `HealthRegenBoost` arm（不能落 `_ =>` 兜底误归默认丹药色）：
    - `status_effect_id`(:58) → `"health_regen_boost".to_string()`
    - `status_effect_name`(:75) → `"养伤".to_string()`
    - `status_effect_category`(:118) → 归入既有 `=> "buff"` 合并臂（`status_effect_color` 由 category 派生，buff→绿 0xFF55CC66，**无需单独改 color match**，但必须确保 category 命中 buff 而非漏到末臂）
    - `status_effect_source_label`(:161)：显式补 `HealthRegenBoost => "养伤设施"`（**否则落 :171 `_ => "战场丹药"` 兜底，语义错——床/蒲团非战场丹药**）
    - `status_effect_color`(:175)：由 category 派生，确认 category=buff 即得绿；**无须新 arm，但 review 须核 :181 `_ =>` 灰兜底未被命中**
    - `status_effect_dispel`(:185)：养伤 buff 属可自然过期、不可驱散级别，落 :190 `_ => 1` 兜底语义正确，**显式注释说明而非漏判**
- **测试**（饱和化）：
  - happy path：基准速率每 N tick 回 X HP，clamp 到 max
  - 边界：满血不溢出（off-by-one：current==max 时 delta=0）；空血（current<=0）不回（死亡态由 lifecycle 接管）
  - 错误分支/优先级：活跃流血时 `health_regen_tick` 净效果不加血（构造 bleeding wound + 同 tick 断言 HP 不升）
  - 乘区叠加 clamp：`healing_rate_multiplier=1.5` + `HealthRegenBoost magnitude=0.5` → 复合 multiplier 正确、不超合理上限
  - 变体 pin 测试：`HealthRegenBoost` serde round-trip（正反 sample 对拍）+ `status_snapshot_emit` 6 个 match 各一条命中断言（id/name/category/source_label/color/dispel 取值锁定，专测 `_ =>` 兜底**未**被 HealthRegenBoost 命中——id/name/source_label 必须是显式 arm，color/category 必须归 buff 绿）
  - 状态转换：buff 施加→生效→过期回基准（`duration_ticks` 耗尽后 multiplier 回 1.0）
- **视听**：纯 server 逻辑（恢复 buff 的玩家感知视听在 P3 床 aura 上 buff 瞬间触发，见 P3）。

## P3 — 床/蒲团 aura buff + 限制 + 视听 ⬜

**目标**：床/蒲团放置后在小范围内给静止玩家恢复加速 buff，带用户硬要求的 4 条限制，配上 buff 瞬间的视听反馈。

- **furniture aura tick**（`world/furniture.rs` 或新 `combat`/`cultivation` 桥接 system）：
  - 每 N tick 扫 `FurnitureRegistry.kinds_in_range(player_pos, FURNITURE_AURA_RADIUS)`（Chebyshev **2 格**，参 `is_within_workbench_range:47` 收小到单间屋量级）
  - `SimpleBed` → `send(ApplyStatusEffectIntent { kind: HealthRegenBoost, magnitude: 0.5, duration_ticks: 短续期 })`（§8.1 #1：床 +50% HP 恢复）
  - `MeditationMat` → `send(ApplyStatusEffectIntent { kind: CultivationAcceleration, magnitude: 0.2, ... })`（§8.1 #1：蒲团 +20% 修炼速度，凡物弱效）。`CultivationAcceleration` 已被 `cultivation/tick.rs:181,327-331` `cultivation_acceleration_multiplier` 消费，其底层走 `qi_regen_and_zone_drain_tick` 守恒链——**蒲团 buff 守恒安全，无新增 qi 流**
  - **本 plan 明确不接 `practice_session_tick`**（实测 §8.1 #6 决议）：该函数（`practice_session.rs:69`，`#[allow(dead_code)]`）是纯叶子函数（参数 `zone_qi`/`current_qi` 为裸值，无 Query/Res），全模块仅 `pub mod practice_session;`（mod.rs:66）无任何 ECS 注册，三处调用全在自身测试内——"注册进 ECS schedule"并非一行能完成，需新写供给 qi/zone/proficiency 的包装 system。更关键：其第 78 行直接 `*current_qi = (*current_qi - cost).max(0.0)` 扣玩家真元，**无对应 zone credit、不走 `qi_physics::ledger`**，接成 live system 即触发 worldview §二/§十「修炼消耗=别人少掉」守恒律红旗（真元凭空消失、无 zone 归账）。接活 = 守恒敏感 + 体量大，**登记到 `reminder.md` 留专门 plan**，不混入本 plan
- **限制**（用户硬要求，全部需专属测试）：
  1. **同效 buff 不叠加**：aura tick 施加前检查 target 是否已有同 kind 活跃 status，有则 refresh duration 而非叠 magnitude（去重在 aura 侧，不依赖 status.rs 折叠）
  2. **辐射半径小**：Chebyshev 2 格（单间屋）
  3. **仅静止/坐卧姿态生效**：玩家移动（位移超阈值 / 非 Idle 体力态）即不续期 → buff 自然过期掉落（"移动即掉 buff"）
  4. **凡物不设数量上限但效果不叠**：多放床只 refresh 不增益 → 多放无收益（限制 #1 的直接推论）
- **视听**（buff 上身瞬间一次性触发，走 `bong:vfx_event` channel，server 侧 `EventWriter<VfxEventRequest>` 仿 `npc/skull_fiend.rs:258`，schema `VfxEventPayloadV1::spawn_particle`）：
  - **床（HealthRegenBoost 上身）**：新 VfxPlayer `BedRestAuraPlayer`（仿 `EnlightenmentAuraPlayer.java:64` `spawnSprite`），`vfx_event` ID `bong:furniture_bed_rest`；`BongSpriteParticle` burst 模式 6 颗，颜色 `#E8C97A`（暖黄羽絮），lifetime 12 tick，向上缓速 0.02 b/t 飘散，复用 `BongParticles.enlightenmentDustSprites` 贴图（无需新贴图）
  - **蒲团（CultivationAcceleration 上身）**：新 VfxPlayer `MeditationAuraPlayer`，`vfx_event` ID `bong:furniture_meditation`；`BongSpriteParticle` radial 模式 8 颗绕身 0.6 半径，颜色 `#BFD8C8`（青白雾圈），lifetime 16 tick，复用 enlightenmentDust 贴图
  - 两个 VfxPlayer 在客户端 bootstrap 走 `VfxRegistry.register(eventId, player)`（`VfxRegistry.java:38`）
  - **HUD**：复用既有 HUD 事件流（非常驻 UI，对齐 HUD 极简原则），上 buff 时推一条文案，无 overlay/vignette/tint
  - **音效**：buff 上身一次 `audio_recipe` 单层 `entity.player.levelup`（pitch 1.6, volume 0.3, delay 0t）轻提示音——区别于战斗音，柔和歇息感；无需多层
- **narration**（scope=player，style=perception，2 条示例）：
  - 床：「你倚着床榻歇下，伤处的钝痛慢慢退去。」
  - 蒲团：「盘膝坐定，气息渐匀，周身灵机似乎流转得快了些。」
- **测试**：
  - 范围边界 off-by-one：Chebyshev 2 格内命中、3 格不命中（边界格精确）
  - 移动掉 buff：玩家从静止→移动后 aura 停止续期、buff 在 duration 后过期
  - 双床去重：范围内 2 张床，target 只持一份 HealthRegenBoost（magnitude 不翻倍）
  - buff 过期回基准：移出范围 / 停止 → status 过期 → `health_regen_tick` 回 base multiplier
  - 状态转换：进范围（无 buff→有）/ 续期（有→刷新）/ 出范围（有→过期）三转换各一 case
  - 蒲团 `CultivationAcceleration` 经 `cultivation_acceleration_multiplier`（`tick.rs:181`）正确叠乘
  - vfx：上 buff 时 emit 对应 `VfxEventRequest`（event_id 断言 `bong:furniture_bed_rest` / `bong:furniture_meditation`）

## P4 — 防潮架接保鲜 / 灵石架降级装饰 / 收尾验收 ⬜

**目标**：灵石架降级为纯装饰（系统未实装）；端到端验收 + bbmodel 收尾。**防潮架接 shelflife 降级为 follow-up**——依赖 `plan-placeable-container-blocks-v1`（世界放置容器 entity）未落地，本 plan 不做悬空接线（见下文 + reminder.md）。

- **`moisture_base` 接 shelflife**（§8.1 #3 + 实测：**依赖 `plan-placeable-container-blocks-v1`（世界放置容器 entity）未落地，本 plan 内降级为 follow-up，不做悬空接线**）：
  - **实测阻塞**：shelflife 是 **item-in-container** 模型——`container_storage_multiplier(behavior, profile)`（`container.rs:30`）是按容器自身 `ContainerFreshnessBehavior` 算 multiplier 的纯函数，**无任何世界坐标/范围概念**；当前 `ContainerFreshnessBehavior` 是在 `compute`/`inventory_snapshot_emit`（`inventory_snapshot_emit.rs:344` 等）调用时按容器类型传入，**没有"世界放置容器 entity"这种可被坐标命中的对象**。而同族 `plan-placeable-container-blocks-v1` **仍是 skeleton（未 active、未 merge）**，世界放置容器 entity 尚未实装——所以"moisture_base 范围内的容器"此刻根本不存在可命中的目标。
  - **决议（避免悬空孤岛）**：本 plan **不实装** moisture_base→保鲜接线，`moisture_base` P1 照常落地为登记进 `FurnitureRegistry` 的家具方块，但 P4 不给它保鲜行为；保鲜接线**降级为 follow-up，登记 reminder.md**，待 `plan-placeable-container-blocks-v1` merge（世界放置容器 entity 就位）后另立或并入。
  - **届时（依赖落地后）的完整接线链路**（写清供 follow-up 直接落地，不留含糊）：
    1. 触发事件：物品 **enter_container**（`container.rs:82` `enter_container`）或玩家把容器 entity 放置进 moisture_base 范围
    2. 范围判定：用 `FurnitureRegistry.kinds_in_range(container_entity_pos, MOISTURE_RANGE)` 判该容器 entity 是否落在某个 `MoistureBase` 方块范围内
    3. 命中后：把该容器实例的 `ContainerFreshnessBehavior` 改为 `Freeze`（**不是直接把 `storage_multiplier` 设 0.0**——multiplier 由 `container_storage_multiplier(&Freeze, profile)` 派生：time-based→0.0、**Stepwise→1.0**，`container.rs:46` / `types.rs:248`；直写 0.0 会把 Stepwise 物品瞬间归零，`container.rs:21` Codex review r#34 P1 教训）
    4. 移出范围 / 容器被破坏：还原 `ContainerFreshnessBehavior` 为原值，并按 `exit_container` 维护 `frozen_since_tick`/`frozen_accumulated`（lazy eval 正确性）
  - 仍**不做**每 tick 扫所有容器物品的实时覆盖（研究 #3：与 shelflife "进容器时确定 multiplier" 架构冲突）
- **`spirit_stone_rack` 降级纯装饰**：灵石存储/磨损系统不存在 → 改 `description` 为装饰方块说明，FurnitureRegistry 仍登记（供将来灵石系统 plan 复活），但 P3 aura tick 不给它任何 buff 分支。复活留 follow-up（reminder 登记）
- **`niche_repair_kit` 不在本 plan**（见 [[plan-niche-craft-fix-v1]]）
- **bbmodel 收尾**（4 家具的 bbmodel 资产，按 §10 三轮打磨 + PROMISE；P1-P3 先用 bong_blocks 默认渲染，此处补真长相）：床=矮榻铺草席、蒲团=圆形坐垫、防潮架=带托盘的木架、灵石架=空槽石架
- **测试**：
  - moisture_base：本 plan **仅测**它作为家具方块放置/破坏 + FurnitureRegistry 登记/移除回归（保鲜行为是 follow-up，依赖未落地，不在本 plan 测——避免测试手塞掩盖悬空接线）
  - 装饰方块：spirit_stone_rack 放置/破坏回归 + aura tick 不给它 buff（断言无 ApplyStatusEffectIntent）；moisture_base 同样断言 P4 不给它任何保鲜行为/intent
  - e2e：合成→放置 4 家具→站旁验床/蒲团 buff→移动掉 buff→破坏回收→registry 清空（保鲜不入本 e2e）

---

## §8 开放问题（P0 决策门前需收口）

> 全部已在 §8.1 收口。原表保留以备追溯，**实施时以 §8.1 决议为准**。

1. **数值**：health_regen 基准速率、床 +x%、蒲团 qi regen +x%
2. **床的使用形态**：纯 aura vs 必须"躺"（交互姿态）
3. **moisture_base "架上"判定**：范围内所有容器 vs 直接放其上的容器
4. **门闩/窗栅的"防贼"**：实体方块阻挡是否够
5. **与 plan-block-lifecycle-v1 P4/P5 的边界**
6. **qi_regen_multiplier 死字段是否顺带激活**

## §8.1 决议（pre-P0 收口，2026-06-10）

### #1 数值与流血压制

**决议**：
1. `BASE_HEALTH_REGEN_PER_SEC` = 0.5 HP/s（慢回，养伤而非战斗回血）；床 `HealthRegenBoost magnitude = 0.5`（+50%）；蒲团 `CultivationAcceleration magnitude = 0.2`（+20%，凡物弱效，弱于丹药/经脉）
2. `health_regen_tick` 内必须 explicit 判断 `Wounds.entries` 有 `bleeding_per_sec > 0` 则本 tick short-circuit 不回血——**不依赖系统执行顺序**（`wound_bleed_tick` 与 `health_regen_tick` 并行，靠 net 效果会留隐患）
3. 拒绝"流血时也回血只是净减慢"——正典养伤语义是流血必须先止血才恢复

**落点**：`server/src/combat/lifecycle.rs`（新 `health_regen_tick`，仿 `:207` `stamina_tick`）/ plan P2、P3 数值行

### #2 床的使用形态 + 家具多格表示

**决议**：
1. P3 床/蒲团一律**纯 aura**（站/坐范围内即生效），不做"躺/坐"交互姿态——姿态系统工作量大，留 v2
2. P1 家具方块一律 **1×1 单格落地**（simple_bed 2×2 / moisture_base 2×1 在世界占单格），`FurnitureRegistry` 坐标键保持单 `[i32;3]`；多格联动方块（bed_head+bed_foot）留 v2
3. 拒绝走 vanilla BEDS 状态机（带睡眠/重生点语义，与本 plan aura 语义冲突）

**落点**：`server/src/world/furniture.rs`（新 `FurnitureRegistry`，仿 `zhenfa/mod.rs:405`）/ `server/crates/valence_generated_bong/bong_blocks.json`（4 条家具方块定义，codegen 出 `BlockState::BONG_*`）+ `client/.../block/BongBlocks.java`（客户端注册，按 README 6 步）/ plan P1、P3

### #3 moisture_base "架上"判定（实测修正：依赖未落地，降级 follow-up）

**决议**：
1. **本 plan 不实装 moisture_base→保鲜接线**——实测 shelflife 是 item-in-container 模型（`container_storage_multiplier` 纯函数无世界坐标/范围概念），"范围内的容器"需要**世界放置容器 entity** 作为可被坐标命中的对象，而该 entity 归 `plan-placeable-container-blocks-v1`（**仍 skeleton，未 active/merge**）。在依赖落地前接线 = 命中不到任何对象的悬空孤岛（极易被测试手塞掩盖）。
2. 降级路径若届时实装：改容器实例 `ContainerFreshnessBehavior` 为 `Freeze`（**不是直写 storage_multiplier**），multiplier 由 `container_storage_multiplier(&Freeze, profile)` 派生（time-based→0.0 / **Stepwise→1.0** 防瞬间归零，`container.rs:46`），移出还原 + `exit_container` 维护 frozen 字段——不做每 tick 全范围实时扫描（与 "进容器时确定 multiplier" 架构冲突）。
3. moisture_base 在本 plan 仍 P1 落地为家具方块 + 进 `FurnitureRegistry`（供 follow-up 复活），P4 不给它保鲜行为；接线登记 reminder.md，待依赖 merge 后另立/并入。

**落点**：`server/src/shelflife/container.rs:30,46,82`（Freeze 派生 + enter/exit）/ `server/src/shelflife/types.rs:248`（`ContainerFreshnessBehavior`）/ `plan-placeable-container-blocks-v1`（依赖根，未落地）/ plan P4 / reminder.md

### #4 门闩/窗栅"防贼"

**决议**：
1. 本 plan 只做实体方块阻挡——door_bolt→`IRON_DOOR`、window_grate→`IRON_BARS`，物理屏障即可
2. NPC 破门/玩家盗窃系统不存在（worldview §十一:915 灵龛保护是"无法破坏方块"的硬规则，非 NPC 破门 AI）→ 实体屏障已对齐正典
3. 不在本 plan 造破门/防盗系统（无对应 NPC AI 接口，会成孤岛）

**落点**：`server/src/world/block_place.rs:243`（4 条 vanilla 映射）/ plan P0

### #5 与 plan-block-lifecycle-v1 P4/P5 边界

**决议**：
1. P4 client 放置 wiring 核心已在 main（`ClientRequestSender.java:156` + mixin `:126`），本 plan P0 只需扩 `BlockVanillaIconMap` + 改 TOML category，不阻塞于 lifecycle P4 整 PR
2. P0 实施前必 `git log`/grep 确认 lifecycle P4 worktree 分支与 main 差异——若 P4 已含部分 category 改写，P0 按其落地范围收缩，避免 TOML 同条目 merge conflict
3. 与同族 `plan-workbench-place-runtime-v1` / `plan-placeable-container-blocks-v1` 共用 `block_item_to_state` + `BlockVanillaIconMap`——命名/扩点保持一致（本 plan 只加 8 物品条目，不动它们的 PlaceableBlockKind/ExternalContainerKind 抽象）

**落点**：`server/assets/items/workbench_materials.toml`（8 条 category）/ `client/.../BlockVanillaIconMap.java:22` / plan P0、P1

### #6 qi_regen_multiplier 死字段归属 + practice_session_tick 不接活

**决议**：
1. 蒲团走 `CultivationAcceleration`（`tick.rs:181` 已有消费），**不接** `DerivedAttrs.qi_regen_multiplier`（`components.rs:291`）
2. `qi_regen_multiplier`（HeartLung scar circuit 写入无读取）继续无归属——本 plan 不顺带激活（接它需改 `cultivation/tick.rs:199-212` qi_regen tick 乘区链，超本 plan 范围）
3. reminder.md:23 该字段保持"暂无归属"登记，留专门 plan 处理；本 plan 只激活 `healing_rate_multiplier`（reminder.md:22，已归本 plan P2）
4. **`practice_session_tick` 不接活（实测加项，原骨架误把它当 P3 一行副任务）**：实地核查（2026-06-10）暴露两点——(a) `practice_session_tick`（`practice_session.rs:69`）是纯叶子函数（参数 `zone_qi`/`current_qi` 裸值无 Query/Res），全模块仅 `pub mod practice_session;`（mod.rs:66）无任何 ECS 注册，三处调用全在自身测试内，"注册进 ECS schedule"需新写供给 qi/zone/proficiency 的包装 system（非一行）；(b) 其第 78 行 `*current_qi = (*current_qi - cost).max(0.0)` 直扣玩家真元、**无 zone credit、不走 `qi_physics::ledger`**，接成 live system 即触发守恒律红旗。**故本 plan 不接 practice_session_tick**——蒲团 +20% 修炼速度由守恒安全的 `CultivationAcceleration` 乘区独力承载，打坐累积系统留专门 plan（届时须写出包装 system 的 qi 来源 + ledger 归还 zone 路径 + 守恒断言：扣 qi 必有 zone 等额变化）。已登记 reminder.md。

**落点**：`server/src/cultivation/tick.rs:181`（CultivationAcceleration 消费）/ `server/src/cultivation/practice_session.rs:69,78`（不接活的根因）/ plan P3 + qi_physics 锚点 / reminder.md:23、放置类族段

---

## §10 实施工作流

本 plan scope = 5 PR（P0-P4），按 docs/CLAUDE.md §六执行。

### §10.1 多 PR 拆分点（依赖顺序，前一 merge 后开下一）

1. **PR-1（P0）纯方块落地**：TOML category × 4 + `block_item_to_state` 4 条 vanilla + `BlockVanillaIconMap` 4 条（`Map.of`→`Map.ofEntries`）。独立可验，不依赖后续。**先行**确认 lifecycle P4 与 main 差异（§8.1 #5）
2. **PR-2（P1）FurnitureRegistry + bong_blocks**：新 `world/furniture.rs` + 4 家具按 README 6 步走 bong_blocks（json 追加 + codegen + 客户端 `BongBlocks.java` 注册 + `./gradlew generateBongBlockIds` + `BongBlocksTest`）+ registry 增删/持久化（**跨 server/client，非纯 server PR**）
3. **PR-3（P2）血量恢复体系**：`health_regen_tick` + `HealthRegenBoost` 变体 + status 乘区 + proto/bridge wire + 激活 `healing_rate_multiplier`。纯 server 逻辑，独立成 PR 避免与视听 review 混杂
4. **PR-4（P3）aura buff + 限制 + 视听**：依赖 PR-2（registry）+ PR-3（HealthRegenBoost）。aura tick + 4 限制 + 2 VfxPlayer + narration（**蒲团仅走 CultivationAcceleration，不接 practice_session_tick——见 §8.1 #6**）
5. **PR-5（P4）降级/bbmodel/验收**：依赖 PR-2/4。spirit_stone_rack + moisture_base 降级装饰（moisture_base 保鲜接线为 follow-up，依赖 `plan-placeable-container-blocks-v1` 未落地，**不在本 PR**）+ 4 家具 bbmodel + e2e

### §10.2 视觉资产 3 轮打磨 + PROMISE

PR-5 的 4 家具 bbmodel 属视觉资产，**强制走 docs/CLAUDE.md §10.1 三轮自我打磨**（Round 1 first cut → Round 2 render 验布局 → Round 3 spec/叙事一致），终轮 commit message 末尾写 `<PROMISE>` 担保块（拼写 PROMISE）。参 `scripts/models/gen_*_coffin.py` 分部件流程 + `render_bbmodel.py` 预览。P0-P4 的纯逻辑/vanilla 占位 TODO 不适用本节，常规 atomic commit + 测试全绿即可。

### §10.3 PR 用独立 subagent（context 隔离）

每个 PR 起独立 subagent（`subagent_type: "claude"`, `model: "opus"`, prompt 末尾 `ultrathink`），主线只接收 result + 走 merge。subagent 只实施 + 提 PR，不等 review。

### §10.4 CodeRabbit 等待协议

每 PR `gh pr checks` 看 CR：`pass`→merge / `pending`→`ScheduleWakeup 1200s`（最多 3 回合 = 60 min）/ `fail`→按 consume-plan step 7 严重性桶处理。修完 review 必重等 re-review，不自判通过。前一 PR 收敛才开下一。

### §10.5 单次 consume-plan 全自动到 merge

用户提交 `/consume-plan plan-furniture-buff-v1` 后即可下班；醒来看本 plan 是否已带 `## Finish Evidence` 迁入 `docs/finished_plans/`。

## Finish Evidence

（迁入前必填）
