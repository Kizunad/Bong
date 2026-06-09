# plan-economy-zombie-cleanup-v1 — 经济/流派/材料/工具类僵尸物品消杀(骨架)

> 一句话:僵尸物品审计的集中删除/接通 plan——2 个蜕壳流伪装道具接通、2 个换代放置 kit 清理、6 个无系统经济物品删除、9 个材料断链/工具类删除(2026-06-10 扩入)。
>
> 来源:僵尸物品审计;用户拍板 2026-06-10:「#1/#3 立一个,#2 就算了删除吧」+「工具 4 直接删除,该做适配的适配」(适配 2 件见 [[plan-gathering-tool-bind-v1]])。

**依赖**:无(纯接通/清理,不依赖其他 plan;kit 处理与 plan-block-lifecycle-v1 已落地的放置链路只读对接)。

| 阶段 | 主题 | 状态 |
|------|------|------|
| P0 | 蜕壳流伪装道具接通(disguise_wrap / camouflage_net) | ⬜ |
| P1 | 放置 kit 换代清理(forge_station_kit / furnace_kit_fantie) | ⬜ |
| P2 | 6 个经济物品删除(bone_coin_blank 等) | ⬜ |
| P3 | 9 个材料断链/工具类删除(2026-06-10 扩入,调查 workflow 裁决) | ⬜ |

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

## P3 — 9 个材料断链/工具类删除(2026-06-10 扩入)

来源:材料断链调查 workflow(opus 抽查 5/5 证据属实)。裁决原则同 P2:需整套新系统支撑才能活的删;差临门一脚的适配(herb_bundle/cao_lian 两件)移 [[plan-gathering-tool-bind-v1]]。

**材料断链 5(含 sling_weapon 连带 1)**:

| id | 名称 | 删除理由 | 落点 |
|----|------|---------|------|
| `powder_zhu_sha` | 朱砂粉 | 炼丹直接吃原矿 NBT(`material=zhu_sha_aux, mineral_id=zhu_sha`),粉中间体被设计放弃,符箓无 plan | 模板 workbench_materials.toml:142-150 + 配方 #29 |
| `iron_sword_blank` | 铁剑胚 | 所有 forge blueprint 一步直用 fan_tie,接通需重做 multi-step tempering 整套 | 模板 :677-685 + 配方 #53;blueprint 不受影响 |
| `stone_spearhead` | 石矛头 | 无矛 WeaponKind/模板/蓝图(NPC 的矛是硬编码 zhinian_spear),接通=新武器类型 | 模板 :616-624 + 配方 #56 |
| `sling_stone` | 弹弓石 | 弹弓退化为近战刺击(player_attack.rs:117/125)无弹药消耗;weapon-v1 正典明确不做 ranged | 模板 :644-652 + 配方 #58 |
| `sling_weapon` | 弹弓(连带) | sling_stone 删+ranged 不立项后,这把近战兜底"弓"无独立价值 | 模板 :627-641 + 配方 #57 |

**工具 4(用户指示直接删)**:

| id | 名称 | 落点 |
|----|------|------|
| `stone_hoe` | 石锄 | 模板 :340-348 + 配方 #3(L124-131);无 ToolKind/GatheringToolSpec 接线零 runtime |
| `mortar_stone` | 研钵 | 模板 :351-359 + 配方 #7(L162-171) |
| `heat_gloves` | 隔热手套 | 模板 :362-370 + 配方 #10(L192-201,随行删 scroll_workbench_heat_gloves unlock)。⚠️**严防误删 `bing_jia_shou_tao`(冰甲手套)**——后者是 xue_po_lian 的 required_tool(botany/registry.rs:240),两者是不同 item |
| `trade_scale` | 交易秤 | 模板 :771 + 配方 #9(L182-191)。**修 P2 红旗:原 P2 列了 trade_scale_stand 却漏 trade_scale 本体**——stand 配方(#85,L1161-1171)消耗 trade_scale,四处必须同 PR 原子删:scale 模板/配方#9/stand 模板/stand 配方#85 |

- client icon 一并删:`textures/gui/items/{stone_hoe,mortar_stone,heat_gloves,trade_scale,trade_scale_stand,powder_zhu_sha,iron_sword_blank,stone_spearhead,sling_stone,sling_weapon}.png`(存在的删,build 产物随 gradle 重建不手删)
- 影响面已核验:ToolKind 枚举/GATHERING_TOOL_SPECS/loot/NPC/agent/client Java 均零引用,删除不产生测试红灯
- 存量迁移:这批仅 dev `/give` 可得,无 gameplay 来源,风险极低——item-load 路径加"未知 template_id 静默丢弃 + warn 日志"兜底(若尚无),不做主动迁移脚本
- 测试:同 P2(registry 加载/全仓 grep 0 引用/smoke);兜底丢弃路径专属用例(未知 id 不 panic + 日志断言)

---

## §8 开放问题(P0 决策门前需收口)

1. **P0 路线 A vs B**:蜕壳入口改过滤(A,玩法增量)还是配方改产物(B,纯收缩)?A 需确认蜕壳系统状态机能否表达"物品气息掩盖"与"驻地遮蔽"两种新效果
2. **P1 删 ID vs 改产物**:倾向改产物,需先核 `fan_iron_anvil`/`furnace_fantie` 是否已有自己的配方(有则旧配方直接删,无则改产物)
3. **存量实例迁移**:删除的 6 个 id 在已存档玩家库存中的处理(建议:加载时折算 bone_coin 返还,按 base_weight 估价;或直接清除+narration 一句)
4. **camouflage_net 的 grid 2×2**:若走 A 路线,遮蔽网放置形态是否依赖 plan-workbench-place-runtime-v1(是则 P0 拆出该效果延后)
5. **P3 存量兜底现状**:item-load 对未知 template_id 的行为(panic? 跳过?)实施前 grep 确认,无兜底则 P3 先补
