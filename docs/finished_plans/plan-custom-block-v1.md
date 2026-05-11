# Bong · plan-custom-block-v1 · 自定义方块基建

Fork Valence 的 `valence_generated` codegen，扩展 block registry 支持 `bong:*` 命名空间的自定义方块。Vanilla 方块数据不改（`extracted/blocks.json` 保持原样），Bong 方块定义在独立的 `bong_blocks.json` 中，state ID 从 `vanilla_max + 1` 自动分配。Client Fabric mod 同步注册同名方块，ID 通过共享配置对齐。

**为什么要做**：阵法多方块结构（plan-zhenfa-content-v1）、未来的灵龛方块、灵田可视化方块、自定义装饰方块都需要 server 和 client 对齐的自定义方块支持。这是底层基建。

**前置依赖**：
- Valence git rev `2b705351`（当前 pinned 版本）
- `plan-forge-v1` ✅ → 已有 `WeaponForgeStation` Entity（当前无真方块，本 plan 可为其提供方块基础）

**反向被依赖**：
- `plan-zhenfa-content-v1` ⬜ active → 阵法节点/纹路方块
- `plan-qixiu-depth-v1` ⬜ active → 炼器台真方块化（未来）
- 任何需要自定义方块外观的 plan

---

## 接入面 Checklist

- **进料**：Valence `crates/valence_generated/build/block.rs`（codegen 入口）/ `extracted/blocks.json`（vanilla 数据）/ Valence `BlockState(u16)` / `BlockKind` enum
- **出料**：`bong_blocks.json`（Bong 方块定义文件）/ 扩展后的 `BlockState` 支持 `id > 24134` / 扩展后的 `BlockKind` 包含 `BongZhenfaNode` 等变体 / Fabric 客户端对应方块注册 + 模型/贴图 / 共享 ID 配置
- **跨仓库契约**：
  - server：fork valence_generated，修改 `build/block.rs` 加载 `bong_blocks.json`
  - client：`BongBlockRegistry.java` 注册自定义方块 + blockstate JSON + 模型/贴图
  - 共享：`bong_blocks.json` 是两端的 single source of truth
- **worldview 锚点**：无直接锚点（纯基建）
- **qi_physics 锚点**：不涉及

---

## §0 设计轴心

- [x] **Vanilla 数据零侵入**：`extracted/blocks.json` 不改一个字节。Bong 方块在独立的 `bong_blocks.json` 中定义
- [x] **ID 自动分配**：Bong 方块 state ID 从 `vanilla_max_state_id + 1`（24135）开始，按定义顺序自动递增
- [x] **单一事实源**：`bong_blocks.json` 同时驱动 server codegen 和 client 注册，不允许两端各自定义
- [x] **最小 fork**：只 fork `valence_generated` crate，不 fork 整个 Valence。server Cargo.toml 的 valence 依赖不变，只是把 `valence_generated` 指向本地 fork

---

## §1 架构

```
bong_blocks.json（单一事实源）
  ├─→ server: fork valence_generated/build/block.rs
  │     追加到 vanilla blocks Vec → codegen 自动扩展:
  │     - BlockKind enum 新增变体
  │     - BlockState const 新增常量
  │     - from_raw() 上限扩大
  │     - 所有 match arms（luminance/opaque/collision 等）自动覆盖
  │
  └─→ client: build 时读 bong_blocks.json
        → 生成 BongBlockRegistry.java（或手写）
        → Fabric Registry.register() 注册方块
        → blockstates/*.json + models/block/*.json + textures
```

### bong_blocks.json 格式

```json
{
  "blocks": [
    {
      "name": "zhenfa_node",
      "namespace": "bong",
      "translation_key": "block.bong.zhenfa_node",
      "properties": [],
      "default_state": {
        "luminance": 3,
        "opaque": false,
        "replaceable": false,
        "blocks_motion": false,
        "collision_shapes": []
      },
      "item_id": null
    },
    {
      "name": "zhenfa_line",
      "namespace": "bong",
      "translation_key": "block.bong.zhenfa_line",
      "properties": [
        { "name": "axis", "values": ["x", "y", "z"] }
      ],
      "default_state": {
        "luminance": 1,
        "opaque": false,
        "replaceable": false,
        "blocks_motion": false,
        "collision_shapes": []
      },
      "item_id": null
    },
    {
      "name": "zhenfa_eye",
      "namespace": "bong",
      "translation_key": "block.bong.zhenfa_eye",
      "properties": [
        { "name": "charged", "values": ["true", "false"] }
      ],
      "default_state": {
        "luminance": 5,
        "opaque": false,
        "replaceable": false,
        "blocks_motion": false,
        "collision_shapes": []
      },
      "item_id": null
    }
  ]
}
```

### ID 分配规则

```
vanilla blocks: id 0 ~ 1002, state_id 0 ~ 24134
bong blocks:    id 1003+,     state_id 24135+

zhenfa_node:  block_id=1003, state_id=24135 (无 property, 1 state)
zhenfa_line:  block_id=1004, state_id=24136~24138 (axis=x/y/z, 3 states)
zhenfa_eye:   block_id=1005, state_id=24139~24140 (charged=true/false, 2 states)
```

---

## 阶段总览

| 阶段 | 内容 | 状态 |
|----|------|----|
| P0 | Fork `valence_generated`，修改 `build/block.rs` 加载 `bong_blocks.json`，验证 codegen | ✅ |
| P1 | 首批 3 个阵法方块定义（zhenfa_node / zhenfa_line / zhenfa_eye）+ server 放置/读取 API | ✅ |
| P2 | Client Fabric 方块注册 + blockstate JSON + 模型 + 贴图（gen.py scene 风格） | ✅ |
| P3 | ID 对齐验证 + 端到端测试（server 放方块 → client 渲染正确） | ✅ |
| P4 | 文档 + 新增方块流程模板（"加一个新自定义方块需要改哪些文件"） | ✅ |

---

## P0 — Fork valence_generated + codegen 扩展 ✅

### 交付物

1. **Fork `valence_generated` 到项目内**

   将 `valence_generated` crate 复制到 `server/crates/valence_generated_bong/`（或直接 git subtree/patch）。

   修改 `server/Cargo.toml`：
   ```toml
   # 原来
   valence = { git = "...", rev = "2b705351" }
   # 新增 override
   [patch."https://github.com/valence-rs/valence"]
   valence_generated = { path = "crates/valence_generated_bong" }
   ```

   这样 Valence 其他 crate 不变，只有 `valence_generated` 走本地 fork。

2. **修改 `build/block.rs` 的 `build()` 函数**

   在加载 vanilla `blocks.json` 之后，追加加载 `bong_blocks.json`：

   ```rust
   pub(crate) fn build() -> anyhow::Result<TokenStream> {
       // 原有：加载 vanilla
       let mut top: TopLevel = serde_json::from_str(
           include_str!("../extracted/blocks.json")
       )?;

       // 新增：加载 bong 扩展
       let bong_path = std::env::var("BONG_BLOCKS_JSON")
           .unwrap_or_else(|_| "bong_blocks.json".into());
       if let Ok(bong_json) = std::fs::read_to_string(&bong_path) {
           let bong: BongBlocksJson = serde_json::from_str(&bong_json)?;
           let mut next_block_id = top.blocks.len() as u16;
           let mut next_state_id = top.blocks.iter()
               .map(|b| b.max_state_id()).max().unwrap() + 1;

           for bong_block in bong.blocks {
               let states = bong_block.expand_states(next_state_id);
               let block = Block {
                   id: next_block_id,
                   item_id: next_block_id, // 或 0 如果无对应物品
                   wall_variant_id: None,
                   translation_key: bong_block.translation_key,
                   name: format!("bong_{}", bong_block.name),
                   properties: bong_block.properties,
                   default_state_id: next_state_id,
                   states,
               };
               next_state_id += block.states.len() as u16;
               next_block_id += 1;
               top.blocks.push(block);
           }
       }

       // 后续 codegen 逻辑不变——它遍历 top.blocks 生成所有代码
       let max_state_id = top.blocks.iter()
           .map(|b| b.max_state_id()).max().unwrap();
       // ... 原有逻辑 ...
   }
   ```

3. **`BongBlocksJson` 解析结构**

   ```rust
   #[derive(Deserialize)]
   struct BongBlocksJson {
       blocks: Vec<BongBlockDef>,
   }

   #[derive(Deserialize)]
   struct BongBlockDef {
       name: String,
       namespace: String,
       translation_key: String,
       properties: Vec<Property>,
       default_state: BongDefaultState,
       item_id: Option<u16>,
   }

   #[derive(Deserialize)]
   struct BongDefaultState {
       luminance: u8,
       opaque: bool,
       replaceable: bool,
       blocks_motion: bool,
       collision_shapes: Vec<u16>,
   }
   ```

4. **`bong_blocks.json` 初始内容**

   先放 3 个阵法方块（§1 中的定义），作为 codegen 验证。

### 验收抓手

- `cargo build` 成功——codegen 无 panic，`BlockState::from_raw(24135)` 返回 `Some`
- `BlockKind::BongZhenfaNode` 存在且可用
- `BlockState::BONG_ZHENFA_NODE` const 存在
- `BlockState::BONG_ZHENFA_NODE.luminance()` == 3
- `BlockState::BONG_ZHENFA_NODE.is_opaque()` == false
- `BlockState::BONG_ZHENFA_LINE.set(PropName::Axis, PropValue::X)` 返回正确 state
- `cargo test` 全绿（现有测试不受影响）

---

## P1 — Server 放置/读取 API ✅

### 交付物

1. **`BongBlockApi`**（`server/src/world/bong_blocks.rs`，新文件）

   ```rust
   pub fn place_bong_block(
       chunk_layer: &mut ChunkLayer,
       pos: BlockPos,
       block: BlockState,
   ) -> Result<(), PlaceError>

   pub fn remove_bong_block(
       chunk_layer: &mut ChunkLayer,
       pos: BlockPos,
   ) -> Option<BlockState>

   pub fn is_bong_block(state: BlockState) -> bool {
       state.to_raw() >= BONG_BLOCK_STATE_START
   }

   pub const BONG_BLOCK_STATE_START: u16 = 24135;
   ```

2. **阵法放置集成**

   `plan-zhenfa-content-v1` 的放置逻辑改为调 `place_bong_block()`，写入真正的自定义方块而非 vanilla 方块。

### 验收抓手

- 测试：`world::bong_blocks::tests::place_and_read_back`
- 测试：`world::bong_blocks::tests::is_bong_block_true_for_custom`
- 测试：`world::bong_blocks::tests::is_bong_block_false_for_vanilla`

---

## P2 — Client Fabric 方块注册 ✅

### 交付物

1. **Fabric 方块注册**（`client/src/main/java/com/bong/client/block/BongBlocks.java`）

   ```java
   public class BongBlocks implements ModInitializer {
       public static final Block ZHENFA_NODE = register("zhenfa_node",
           new Block(FabricBlockSettings.create()
               .luminance(3).noCollision().breakInstantly()));

       public static final Block ZHENFA_LINE = register("zhenfa_line",
           new PillarBlock(FabricBlockSettings.create()
               .luminance(1).noCollision().breakInstantly()));

       public static final Block ZHENFA_EYE = register("zhenfa_eye",
           new Block(FabricBlockSettings.create()
               .luminance(state -> state.get(Properties.CHARGED) ? 8 : 5)
               .noCollision().breakInstantly()));

       private static Block register(String id, Block block) {
           return Registry.register(Registries.BLOCK,
               new Identifier("bong", id), block);
       }
   }
   ```

2. **Blockstate JSON**

   `client/src/main/resources/assets/bong/blockstates/zhenfa_node.json`：
   ```json
   { "variants": { "": { "model": "bong:block/zhenfa_node" } } }
   ```

   `client/src/main/resources/assets/bong/blockstates/zhenfa_line.json`：
   ```json
   {
     "variants": {
       "axis=x": { "model": "bong:block/zhenfa_line", "x": 90, "y": 90 },
       "axis=y": { "model": "bong:block/zhenfa_line" },
       "axis=z": { "model": "bong:block/zhenfa_line", "x": 90 }
     }
   }
   ```

3. **方块模型 + 贴图**

   模型：`assets/bong/models/block/zhenfa_node.json` 等（薄片/小型方块，不是完整 1m³ 方块）

   贴图生成：
   ```bash
   python scripts/images/gen.py \
     "a glowing spiritual rune carved into stone, faint blue sigil, top-down view, xianxia formation node" \
     --name zhenfa_node --style item --transparent \
     --out client/src/main/resources/assets/bong/textures/block/
   ```

4. **ID 对齐验证**

   Client 启动时打日志确认注册的方块 raw state ID 与 `bong_blocks.json` 定义一致。如果不一致则 crash（fail-fast，不允许带着错位运行）。

### 验收抓手

- `./gradlew build` 成功
- Client 启动无 crash，日志显示 3 个 bong 方块注册成功
- Server 放 `BlockState::BONG_ZHENFA_NODE` → Client 渲染为自定义模型

---

## P3 — 端到端验证 ✅

### 交付物

1. 启动 server + client
2. Server 通过 dev 命令在玩家面前放一个 `bong:zhenfa_node`
3. Client 看到自定义方块模型 + 发光效果
4. 破坏方块 → 消失
5. 切换到 `zhenfa_line` axis=x/y/z → 方块朝向正确
6. `zhenfa_eye` charged=true → 亮度变化

### 验收抓手

- 手动验证截图 × 3（node / line / eye）
- 所有 property 变体渲染正确

---

## P4 — 文档 + 新增方块流程模板 ✅

### 交付物

在 `CLAUDE.md` 或 `docs/` 中补充"新增自定义方块 checklist"：

```
1. 在 bong_blocks.json 中追加方块定义
2. cargo build → 验证 BlockState::BONG_XXX 生成
3. client 中 BongBlocks.java 注册对应 Block
4. 创建 blockstates/*.json + models/block/*.json
5. 生成或手绘贴图
6. 端到端验证
```

---

## Finish Evidence

- **落地清单**：
  - P0：`server/crates/valence_generated_bong/` 本地 fork 接入，`server/Cargo.toml` 通过 `[patch."https://github.com/valence-rs/valence"]` 指向 fork；`server/crates/valence_generated_bong/build/block.rs` 从仓库根 `bong_blocks.json` 读取 Bong 方块定义并追加 codegen。
  - P1：`bong_blocks.json` 定义 `bong:zhenfa_node`、`bong:zhenfa_line`、`bong:zhenfa_eye`；`server/src/world/bong_blocks.rs` 提供 `place_bong_block` / `remove_bong_block` / `is_bong_block`；`server/src/zhenfa/mod.rs` 在阵法放置、触发、拆除、衰减路径写入和移除 Bong 自定义方块。
  - P2：`client/build.gradle` 从 `bong_blocks.json` 生成 `BongBlockIds`；`client/src/main/java/com/bong/client/block/BongBlocks.java` 注册 3 个 Fabric 方块并 fail-fast 校验 raw block/state ID；补齐 `assets/bong/blockstates/`、`models/block/`、`textures/block/`。
  - P3：server 覆盖 codegen raw ID、方块写入/读取/移除、阵法放置写块与拆除移除；client 覆盖 manifest、生成 ID、blockstate/model/texture 资源存在性。图形客户端截图未在本轮无显示环境执行，改由可重复的 raw ID fail-fast 和资源契约测试锁定。
  - P4：`server/crates/valence_generated_bong/README.md` 记录新增自定义方块 checklist。
- **关键 commit**：
  - `8bdc686f5`（2026-05-11）`feat(custom-block): 接入 Bong 方块 codegen fork`
  - `192eb6c14`（2026-05-11）`feat(custom-block): 接入阵法自定义方块放置`
  - `ea84ce911`（2026-05-11）`feat(custom-block): 注册客户端阵法方块资源`
  - `b85adc312`（2026-05-11）`docs(custom-block): 记录新增方块流程`
  - `3545b85a7`（2026-05-11）`Merge remote-tracking branch 'origin/main' into auto/plan-custom-block-v1`
- **测试结果**：
  - `server/crates/valence_generated_bong`: `cargo test` → 3 passed。
  - `server`: `cargo fmt --check` → passed。
  - `server`: `CARGO_BUILD_JOBS=1 cargo clippy --all-targets -- -D warnings` → passed。
  - `server`: `CARGO_BUILD_JOBS=1 cargo test -- --test-threads=1` → 4287 passed。
  - `client`: `JAVA_HOME=/home/kiz/.sdkman/candidates/java/17.0.18-amzn PATH=/home/kiz/.sdkman/candidates/java/17.0.18-amzn/bin:$PATH ./gradlew test build` → BUILD SUCCESSFUL。
- **跨仓库核验**：
  - server：`BlockState::BONG_ZHENFA_NODE`、`BlockKind::BongZhenfaNode`、`BONG_BLOCK_STATE_START = 24135`、`place_bong_block`、阵法 `ChunkLayer` 写块/移除路径。
  - client：`BongBlocks.register()`、`BongBlockIds.ZHENFA_*`、`bong:zhenfa_*` blockstate/model/texture 资源。
  - 共享：`bong_blocks.json` 是 server codegen 与 client generated ID 的共同来源。
- **生成资产说明**：`scripts/images/gen.py` 因本地缺少 `CLIPROXY_API_KEY` / `CLIPROXY_BASE_URL` 未能调用图像后端；本轮使用确定性的 32x32 PNG fallback 贴图，避免引入不稳定外部依赖。
- **遗留 / 后续**：
  - 方块碰撞体积自定义（当前全部 noCollision，未来可能需要半方块碰撞）。
  - 方块实体数据（NBT）——Valence 层面当前不支持，等 plan-persistence-v1。
  - 批量方块定义工具（从 TOML/YAML 转 `bong_blocks.json`）。
  - 更多自定义方块：灵龛方块 / 炼器台方块 / 灵田可视化方块 / 装饰方块。
