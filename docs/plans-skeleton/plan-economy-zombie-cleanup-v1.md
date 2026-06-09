# plan-economy-zombie-cleanup-v1 — 经济/流派类僵尸物品消杀(骨架)

> 一句话:处理僵尸物品审计「经济/站台/流派系统缺失(10)」类——2 个蜕壳流伪装道具接通、2 个换代放置 kit 清理、6 个无系统经济物品删除。
>
> 来源:僵尸物品审计;用户拍板 2026-06-10:「#1/#3 立一个,#2 就算了删除吧」。

**依赖**:无(纯接通/清理,不依赖其他 plan;kit 处理与 plan-block-lifecycle-v1 已落地的放置链路只读对接)。

| 阶段 | 主题 | 状态 |
|------|------|------|
| P0 | 蜕壳流伪装道具接通(disguise_wrap / camouflage_net) | ⬜ |
| P1 | 放置 kit 换代清理(forge_station_kit / furnace_kit_fantie) | ⬜ |
| P2 | 6 个经济物品删除(bone_coin_blank 等) | ⬜ |

---

## 接入面(防孤岛 checklist)

- **进料**:
  - 蜕壳流系统:`server/src/schema/tuike.rs`、`network/false_skin_state_emit.rs`、`npc/npc_skill.rs`(现物品入口硬编码只认 `tuike_false_skin_silk` / `tuike_rotten_wood_armor`)
  - 配方:`server/src/craft/workbench_recipes.rs:1183`(#87 伪装包裹,CraftCategory::TuikeSkin)、`:1295`(#94 伪装网)
  - 物品模板:`server/assets/items/workbench_materials.toml`(10 个 id 全在此)
  - 放置链路:plan-block-lifecycle-v1 已接通的 block_place 协议;真接通的放置物是 `fan_iron_anvil`(`forge/station.rs`)与 `furnace_fantie`(`alchemy/furnace.rs`)
- **出料**:
  - P0:两伪装道具进入蜕壳流真实使用闭环(假皮/遮蔽状态)
  - P1/P2:registry 收缩——删除项在 TOML、配方表、loot、`/give` 补全中全部消失
- **共享类型 / event**:复用蜕壳流现有状态 emit(false_skin_state),不新造 event;不新增 ItemCategory
- **跨仓库契约**:client 侧若有删除项 icon 资产一并清理(`assets/bong-client/textures/gui/items/`);agent 不参与
- **worldview 锚点**:§五防御三流之二「替尸/蜕壳流」(伪装道具);删除项无锚点正是删除理由(摆摊/铸币/游商经济在 worldview §九有叙事但无 plan 支撑,整套系统不立则物品不留)
- **qi_physics 锚点**:无。伪装道具不涉及真元流动;若 P0 决议给 disguise_wrap"掩盖灵物气息"做气息抑制效果,只读现有侦测逻辑,不碰 ledger。

---

## P0 — 蜕壳流伪装道具接通(2)

两条路线 pre-P0 收口拍板(见 §8 #1):

- **A(推荐)**:蜕壳系统物品入口从硬编码 ID 改为按 CraftCategory::TuikeSkin / 模板字段过滤,disguise_wrap、camouflage_net 各挂差异化效果(包裹=单件物品气息掩盖,网=驻地遮蔽)
- **B(保底)**:配方 #87/#94 产物直接改产 `tuike_false_skin_silk` 系,删两个独立 id

- 交付物:`schema/tuike.rs` 入口改造 / 或 `workbench_recipes.rs:1183,1295` 产物替换;使用闭环 e2e(合成 → 使用 → false_skin_state emit → client 收到)
- 测试:两道具各自效果分支 + 蜕壳系统原两 ID 回归不破

## P1 — 放置 kit 换代清理(2)

- `forge_station_kit` / `furnace_kit_fantie` 为 `fan_iron_anvil` / `furnace_fantie` 的旧代 ID,放置 handler 不认(缺 `forge_station_spec`)
- 处理:删 TOML 模板 + 对应配方,或配方产物改指新 ID(pre-P0 收口拍板,见 §8 #2;倾向改产物——保留"合成出可放置炉/砧"的玩法路径)
- 测试:配方表无产出死 ID;`/give` 补全列表无死 ID;放置链路回归

## P2 — 6 个经济物品删除

删除清单(模板 + 配方 + icon + 全仓引用清零):

| id | 名称 | 备注 |
|----|------|------|
| `bone_coin_blank` | 骨币胚 | 铸币闭环无 plan 支撑 |
| `trade_scale_stand` | 交易秤台 | 摊位系统不存在 |
| `price_tag` | 标价签 | 挂牌定价不存在 |
| `trade_puppet_frame` | 交易傀儡骨架 | 玩家自营游商不存在 |
| `waymark_stone` | 标记石 | 世界标记不存在 |
| `rat_bait` | 鼠群诱饵 | 鼠群系统在(`spawn_tutorial.rs`)但用户拍板一并删;若将来复活走新 plan 重立 |

- 现存量处理:玩家背包/箱内已有实例的迁移策略(直接消失 vs 折 bone_coin 返还),pre-P0 收口拍板(见 §8 #3)
- 测试:registry 加载无死 ID;loot/NPC stock/配方全仓 grep 0 引用;启动 smoke 不崩

---

## §8 开放问题(P0 决策门前需收口)

1. **P0 路线 A vs B**:蜕壳入口改过滤(A,玩法增量)还是配方改产物(B,纯收缩)?A 需确认蜕壳系统状态机能否表达"物品气息掩盖"与"驻地遮蔽"两种新效果
2. **P1 删 ID vs 改产物**:倾向改产物,需先核 `fan_iron_anvil`/`furnace_fantie` 是否已有自己的配方(有则旧配方直接删,无则改产物)
3. **存量实例迁移**:删除的 6 个 id 在已存档玩家库存中的处理(建议:加载时折算 bone_coin 返还,按 base_weight 估价;或直接清除+narration 一句)
4. **camouflage_net 的 grid 2×2**:若走 A 路线,遮蔽网放置形态是否依赖 plan-workbench-place-runtime-v1(是则 P0 拆出该效果延后)
