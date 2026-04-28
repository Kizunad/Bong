# Bong · plan-botany-v2

**末法残土植物生态扩展专项**。在 `plan-botany-v1`（22 种正典 / 野生采集闭环，已 finished）+ `plan-lingtian-v1`（人工种植，merged）的基础上，按 **「地形定调」** 原则向 9 个尚未铺植物的主世界 / 位面 worldgen profile 扩展 **17** 种新物种；引入 **生存模式（SurvivalMode）**、**环境锁（EnvLock）**、**采集风险（HarvestHazard）** 三层抽象——让"为什么能在这鬼地方活下来"成为代码可表达的契约，而不是只看 `biome` + `spirit_qi` 阈值。

| Phase | 内容 | 状态 |
|---|---|---|
| P0 | SurvivalMode/EnvLock/HarvestHazard 三抽象 + 5 MVP 物种注册 + TerrainProvider::sample_layer 多通道接口 + DecorationManifest | ⬜ |
| P1 | 余下 **12** 物种注册 + 9 worldgen profile spawn rule 全接入 | ⬜ |
| P2 | HarvestHazard 全实装（fauna / 工具 / 相位类做 stub）+ HarvestSessionHud hazard 行 + ResonanceVision HUD 干扰 | ⬜ |
| P3 | 17 张 item icon（gen-image 批量）+ 客户端渲染管线（entity 路线 + tint 资产 + emissive / DualPhase overlay） | ⬜ |
| P4 | alchemy / forge / zhenfa 配方接入 v2 高阶物种；plan-tools-v1 落地后回填 WoundOnBareHand 真伤 | ⬜ |
| P5 | 极稀事件触发：portal_rift 开闭驱动 lie_yuan_tai；灵气井相位驱动 jing_xin_zao；雪线漂移驱动 xue_po_lian；fauna 立后回填 AttractsMobs 真 spawn | ⬜ |

**世界观锚点**：
- worldview.md §二（灵压三态）/ §四 / §六（真元易挥发 + 染色谱）/ §七（动态生物生态）/ §十三（六域地理）/ §十六（坍缩渊：4 起源 / 三层负压 / 入场过滤）
- library/ecology：灵气零和记 / 异兽三形考 / 灵物磨损笔记 / 末法药材十七种 / 辛草试毒录
- library/geography：北荒坍缩渊记 / 灵泉湿地探案 / 血谷残志 / 六域舆图考

**关键决策**：

1. **地形 = worldgen profile，不是 worldview 六域 1:1**：worldview §十三 列六域（初醒原 / 青云残峰 / 灵泉湿地 / 血谷 / 幽暗地穴 / 北荒），但 worldgen 还实装了 sky_isle / abyssal_maze / ancient_battlefield / TSY 4 origin 等扩展 profile。本 plan 的"地形"是 **worldgen profile 维度**，不必 1:1 对应六域。映射约定：abyssal_maze ≈ 幽暗地穴更深三层；ancient_battlefield ≈ 主世界古战场遗迹（worldview §九 道伥来源处）；sky_isle 暂作青云残峰高空延伸（待 worldview 主文档收编）；TSY 4 origin 走独立位面（worldview §十六 已立，plan-tsy-* 全栈实装中）
2. **SurvivalMode 可代码化**：声明 10 种"能量来源"（QiAbsorb / NegPressureFeed / PressureDifferential / SpiritCrystallize / RuinResonance / ThermalConvection / PortalSiphon / DualMetabolism / PhotoLuminance / WaterPulse），每种有显式生长公式
3. **EnvLock 是声明式硬筛，与 BiomeTag 不重复**：v2 物种**用 EnvLock，不用 BiomeTag**——`BotanyPlantKind.biomes` 字段对 v2 物种留空，筛选权全交给 `env_locks`（worldgen 已 export 的 grid layer + Zone `active_events` + 邻接 decoration 复合判定）
4. **HarvestHazard 镜像生存**：吸负压物种 → **在 zone 自身负灵压抽吸基础上额外叠加** drain（不是单独定值）；锁怨念物种 → 触发心境损 + HUD 幻视；高温火脉 → 徒手 LACERATION
5. **客户端渲染走 entity 路线（与 v1 一致）**：v1 现状 plant = ECS entity（`Plant` Component），不走 BlockKind 注册。v2 复用此路线——单一 `bong:botany_plant_v2` entity 类型 + `BotanyPlantEntityRenderer` 按 PlantKind.id 查 `base_mesh_ref`（vanilla model resource id）+ `tint_rgb`，texture 上叠 RGB 滤镜。**避开 Fabric `BlockColorProvider` 多对一限制**——同一 vanilla block 在多 pos 上无法返回不同 tint，不适用 v2 多物种共享 base_model 场景；BlockEntity 备选路线列入 §8 风险表
6. **wither 路径统一归 zone.spirit_qi，不写 layer**：worldgen layer（neg_pressure / qi_vein_flow / fracture_mask / ruin_density 等）在 P0–P5 全程**只读**；任何 SurvivalMode 物种死亡都按 v1 §1.2.4 同款归 RESTORE_RATIO × growth_cost 到 zone.spirit_qi（即使能量来源是 layer 而非 zone qi——不存在 layer mutability，留 §7 v3+）
7. **数值偏低不膨胀**：v2 物种 alchemy / forge 价值 ≤ v1 同档；本 plan 主要产出是地形深度与世界观完整度
8. **不冲突 v1 / lingtian**：v2 物种**只进 BotanyKindRegistry**（v1 22 种正典静态表），**不进 PlantKindRegistry / SeedRegistry**——lingtian 完全不受影响；v2 物种不存在 `cultivable` 字段（该字段是 PlantKindRegistry 专有）；v1 22 种 ID + 3 别名不动

**与 v1 / lingtian 的关系**：
- 复用 BotanyKindRegistry（22 种 → 39 种；扩 17 种）/ harvest-popup / wither 通路 / `Plant` Component；扩展 `BotanyPlantKind` struct 增加 v2 字段（survival_mode / env_locks / harvest_hazards / base_mesh_ref / tint_rgb / model_overlay / icon_prompt）
- 复用 InventoryStateStore + HarvestSession；新物种 drop 走现有 herbs.toml 注册
- v1 plant entity 路线沿用——v2 不引入新 BlockKind / 不改 Plant Component schema

**交叉引用**：plan-botany-v1.md（finished）· plan-lingtian-v1.md（merged）· plan-tsy-zone-v1.md / plan-tsy-dimension-v1.md / plan-tsy-worldgen-v1.md（TSY 5 物种载体）· plan-alchemy-v1.md / plan-forge-v1.md / plan-zhenfa-v1.md（v2 物种作高阶原料 / 载体）· plan-mineral-v2.md（共享 worldgen layer 接入范式）· **plan-fauna-v1.md（待立 — AttractsMobs 真 spawn 依赖）** · **plan-tools-v1.md（待立 — WoundOnBareHand 真伤依赖）** · **plan-lingtian-weather-v1.md（骨架 — 灵气井相位 driver 依赖）** · plan-narrative-v1.md（骨架 — agent narration 模板）

---

## §0 设计轴心

- [ ] **地形定调 + 物种自洽**：每种 v2 物种的注释必须显式回答："灵气近零 / 灵气倒吸 / 双高末法 / 高空稀薄 / 三层深穴 / 古战场怨念 / 坍缩渊负压 等环境下，这株凭什么能活？"——答不上来的物种砍掉
- [ ] **零和约束不破**：v2 物种全部野生 only（不入 PlantKindRegistry / lingtian SeedRegistry）；wither 仅归还到 zone.spirit_qi（v1 §1.2.4 同款），**不写 layer**
- [ ] **共生 / 寄生硬编码**：渊泥红玉必须 yuan_ni_ebony 5 格内 / 断戟刺必须 broken_spear_tree | war_banner_post 1 格内 / 雪魄莲必须 SnowSurface（broken_peaks profile 雪线常量）—— 共生节点缺则不刷新；EnvLock 每 tick 复检，节点消失（玩家挖 / 漂移）触发 wither
- [ ] **采集风险镜像生存**：吸负压物种 → **在 zone 自身负灵压抽吸基础上额外叠加** drain（不是替换环境抽吸）；锁怨念物种 → 触发心境损 + 缝合兽核同款 HUD 幻视；高温火脉物种 → 徒手 LACERATION
- [ ] **视觉资产复用 vanilla mesh**：所有 v2 物种共享一个 `bong:botany_plant_v2` entity 类型，client 按 PlantKind.id 查 `base_mesh_ref`（vanilla model resource id）+ `tint_rgb` + `model_overlay`（None / Emissive / DualPhase）；**不画新 mesh、不注册新 BlockKind**（备选 BlockEntity 路线见 §8 风险表）
- [ ] **数值偏低不膨胀**：v2 物种 alchemy / forge 价值 ≤ v1 同档；本 plan 产出是地形深度与世界观完整度
- [ ] **跨 plan 钩子保留 stub 依赖**：fauna / 工具 / 灵气井相位未实装时，相关 hazard 字段保留**stub**（注册不报错），具体效果延后接入；不阻塞 P0–P3 主线

---

## §1 子系统拆解

### §1.1 SurvivalMode（10 种枚举）

每植物声明 1 个 SurvivalMode，决定生长公式。**与 EnvLock 协同**：EnvLock 卡硬阈值（"必须满足才存活"），SurvivalMode 决定具体生长速率（"满足后多快生长"）。

| Mode | 能量来源 | 物理依据 | 生长公式 |
|---|---|---|---|
| `QiAbsorb` | zone.spirit_qi（**必须 > 0**） | worldview §十 / library 灵气零和记 | `growth += zone.spirit_qi × density_factor`（zone.spirit_qi ≤ 0 时停滞）—— v1 默认 |
| `NegPressureFeed` | grid 级 neg_pressure layer | worldview §二 负灵域 + library 北荒坍缩渊记 | `growth += grid.neg_pressure × 0.8`（反向呼吸；**不读 zone.spirit_qi**——硬阈值由 EnvLock NegPressure 卡） |
| `PressureDifferential` | grid `qi_vein_flow × (1 - mofa_decay)` | worldview §二 馈赠区 + library 灵泉湿地探案"灵气井" | `growth += diff × density_factor`；diff 跌至 0 则停滞 |
| `SpiritCrystallize` | grid `qi_density × dryness_factor` | worldview §二 末法稀薄 + 灵物磨损笔记"灵气挥发" | `growth += qi_density × 0.5`；**仅 in-game daylight phase**（夜间 / 雨天 stalled） |
| `RuinResonance` | grid ruin_density + 邻接 ruin decoration 残留怨念 | worldview §四 异体排斥 + 异兽三形考"求生冲动凝聚"；**与 zone.spirit_qi 正负无关**（在 TSY 负灵气位面亦成立） | `growth += ruin_density × 0.6`；EnvLock AdjacentDecoration 必须满足 |
| `ThermalConvection` | underground_tier 温差 | abyssal_maze tier 1 暖 / tier 3 寒 / tier 2 对流 | `growth += 0.3` 仅 `underground_tier == 2`；其他 tier 停滞 |
| `PortalSiphon` | 跨位面压差恒定泵 | worldview §十六 主世界 ↔ TSY 跨位面口 | `growth += 0.05/tick`（**恒定常数；不读 zone qi / layer**——硬阈值由 EnvLock PortalRiftActive 卡，事件失效即枯） |
| `DualMetabolism` | grid `qi_density × 0.4 + mofa_decay × 0.4` | worldview §四 真元污染 + 古战场"乱脉" | `growth += dual_sum`；**双相位**（白天 / 夜晚翻转）需 5 min 翻转窗口不被打断 |
| `PhotoLuminance` | 邻接发光体（shroomlight / amethyst / glow_lichen）反馈光 | cave_network / abyssal_maze 已有发光生态 | `growth += 0.4` 仅当 EnvLock AdjacentLightBlock 满足；否则萎缩 |
| `WaterPulse` | 灵气井吐纳相位 | library 灵泉湿地探案"井三日吐两日吸" | 吐期：`growth += 0.5`；吸期：`wither_progress += 0.3`（自吸萎）；相位由 plan-lingtian-weather-v1 driver 提供 |

> **wither 路径统一**：所有 SurvivalMode 物种 wither 到死时，按 v1 §1.2.4 同款归还 `RESTORE_RATIO × growth_cost` 到 zone.spirit_qi——**不写回任何 worldgen layer**（layer 在 P0–P5 全程只读）

### §1.2 EnvLock（环境硬筛）

每植物可选声明 1+ 个 EnvLock 条件，spawn 选位时**逐条 AND**；每 wither tick 复检，任一转 false → 进入 wither_progress。

```rust
pub enum EnvLock {
    NegPressure { min: f32 },                          // grid layer
    QiVeinFlow { min: f32 },                           // grid layer
    FractureMask { min: f32 },                         // grid layer
    RuinDensity { min: f32 },                          // grid layer
    SkyIslandMask { min: f32, surface: SkyIsleSurface },  // Top / Bottom
    UndergroundTier { tier: u8 },                      // 1 / 2 / 3
    PortalRiftActive,                                  // active_events 含 "portal_rift"
    AdjacentDecoration { kind: DecorationKind, radius: u8 },
    AdjacentLightBlock { radius: u8 },                 // shroomlight / amethyst / glow_lichen
    SnowSurface,                                       // height >= broken_peaks::SNOW_LINE_Y 常量
    TimePhase(WaterPulsePhase),                        // 灵气井开 / 合 / 过渡
}
```

**实现路径**：
- **grid layer 类**：通过 `TerrainProvider::sample_layer(pos, layer_name) -> Option<f32>` 查（**P0 关键依赖**——v1 现状 only height/biome 有便捷 API，需补 mmap raster 多通道；详见 §9）
- **AdjacentDecoration**：worldgen 启动期注册 `DecorationManifest`（decoration_name → 主导 BlockKind 集合 + 形状 pattern）；server 在半径内通过 ChunkLayer 扫主导 BlockKind 命中即视为邻接（worldgen profile 现有 16 种 decoration 已穷举）
- **PortalRiftActive**：读 zone 的 `active_events: Vec<String>` 是否含 "portal_rift"
- **SnowSurface**：从 `worldgen::profiles::broken_peaks::SNOW_LINE_Y` 常量读，**不在 plan / 代码硬编码 285 数字**——profile 漂移自动跟
- **TimePhase**：依赖 plan-lingtian-weather-v1（骨架）；P0–P3 内 jing_xin_zao 注册但**不刷新**；P5 待相位 driver 实装后接入

**spawn ↔ wither 对称性**：spawn 选格时 EnvLock 全为真 → 候选；每 BotanyTick wither 复检任一 EnvLock 转 false → 该植物进 wither_progress；wither 满则死、归 zone.spirit_qi（不写 layer）

### §1.3 HarvestHazard（采集风险）

每植物可选声明 0+ HarvestHazard，HarvestSession 在 start / tick / complete 三时机调用：

```rust
pub enum HarvestHazard {
    QiDrainOnApproach { radius_blocks: u8, drain_per_sec: f32 },
    // 玩家进入半径 → session 期间每秒漏真元
    // **叠加规则**：在 zone.spirit_qi < 0 环境本身抽吸之上额外叠加；不替换环境抽吸

    WoundOnBareHand { wound: WoundLevel, required_tool: Option<ItemId> },
    // 不持指定工具采集 → 主手挂指定档伤口
    // P0–P3 退化：required_tool=None 视为 "工具系统未到，等价 dispersal_chance=1.0"
    //   （玩家空手只能采空，不真挂伤；P4 plan-tools-v1 立后回填真伤）

    DispersalOnFail { dispersal_chance: f32 },
    // session 被打断 / 受击 / 错相位 / 退化 BareHand 无工具 → 植物按概率"散气消失"

    ResonanceVision { duration_secs: u8, composure_loss: f32 },
    // 完成时触发 HUD 视觉干扰（缝合兽核 ingest 同款）+ composure -loss
    // 仅干扰采集者本人，半径 0；不影响他人

    SeasonRequired { phase: WaterPulsePhase },
    // 错相位采 → 植物自毁 + 玩家被反吸
    // P0–P3 stub：不真触发；P5 待相位 driver 接入

    AttractsMobs { mob_kind: FaunaKind, count_range: (u8, u8) },
    // session 期间小概率刷怪
    // P0–P3 stub：FaunaKind 仅声明（spirit_mice / mimic_spider 等）；
    // 真 spawn 等 plan-fauna-v1（待立）实装
}
```

**P0–P3 实装边界**：
- **全实装**：QiDrainOnApproach / DispersalOnFail / ResonanceVision
- **stub 占位**（字段注册但效果延后）：WoundOnBareHand（required_tool=None → 退化 dispersal=1.0）/ SeasonRequired / AttractsMobs
- P4 工具 plan 立 → 回填 WoundOnBareHand 真伤；P5 fauna 立 → 接 AttractsMobs；P5 相位 driver 立 → 接 SeasonRequired

### §1.4 渲染管线（entity 路线）

**目标**：不画新 mesh、不注册新 BlockKind，单一 entity 类型承载 17 物种视觉差异。

**Server 侧**：
- `BotanyPlantKind` struct 加：`base_mesh_ref: &'static str`（vanilla model resource id）+ `tint_rgb: u32`（0xRRGGBB）+ `tint_rgb_secondary: Option<u32>`（DualPhase 用）+ `model_overlay: ModelOverlay`（None / Emissive / DualPhase）
- v1 已有 `Plant` ECS Component；v2 不改 component schema，只扩 BotanyPlantKind
- 网络同步走 `schema/botany.rs::BotanyPlantV2RenderProfileV1`（启动期一次性推 client）

**Client 侧（Fabric 1.20.1）**：
- 注册 `bong:botany_plant_v2` entity type + `BotanyPlantEntityRenderer extends EntityRenderer<BotanyPlantEntity>`
- 启动期接收 RenderProfile payload → 建 `{plant_id → profile}` 注册表
- 每 entity 渲染：按 plant_id 查 profile → 加载 vanilla mesh → RenderLayer.translucent + RGB 滤镜（着色不靠 BlockColorProvider）→ overlay==Emissive 时叠 emissive layer（RenderLayer.eyes 风格）→ overlay==DualPhase 时按 `world.timeOfDay` 切 tint_rgb / tint_rgb_secondary（同 mesh）

**为何不走 BlockColorProvider 路线**（必修悖论修复）：
- Fabric `BlockColorProvider.getColor(state, world, pos, tintIndex)` 仅按 BlockState 区分色 —— 同一 vanilla block（如 `lily_of_the_valley`）在多 pos 上无法返回不同 tint
- 而 v2 多物种（yun_ding_lan / xue_po_lian）共享 base_model `lily_of_the_valley`，BlockColorProvider 直接走不通
- 备选 BlockEntity 路线（每物种独立 BongBlock 继承 vanilla，17 个新 BlockKind）开销大且与 v1 entity 路线打架，列 §8 fallback

### §1.5 gen-image 物品图标管线

每物种 1 张 1×1 inventory item icon：
- 生成：`local_images/botany_v2/{plant_id}.png` → `remove_bg.py` 抠图
- 落点：`client/src/main/resources/assets/bong-client/textures/gui/items/{plant_id}.png`
- prompt 集中文件：`scripts/images/prompts/botany_v2.toml`（17 条统一 lock 后批量跑，防风格漂移）
- item 注册：`server/assets/items/herbs.toml` 加 17 条（id = plant_id；spirit_quality_initial 按物种稀有度档定，**TSY 物种 0.3-0.5 区间**——见 §2.8 论点）

调用模板：
```bash
python scripts/images/gen.py "<icon_prompt>" --name {plant_id} --style item --save-prompt
python scripts/images/remove_bg.py local_images/botany_v2/{plant_id}.png
```

---

## §2 v2 新植物表（17 种 × 9 worldgen profile）

> **正典先行**（CLAUDE.md "worldview 正典优先"）：所有新物种**全部入 library**——`docs/library/ecology/末法残土后录·新十七味.json`（新建一卷，遵循 worldview 风格）必须在 plan 落地前先写完并通过 `/review-book`，再回来给本 plan 钉 ID。
>
> 命名约定：`{拼音蛇形}` 风格；display_name 用规范中文名；ID 不与 v1 现有 22 种或 3 别名（kai_mai_cao / xue_cao / bai_cao）冲突。
>
> **Tier 分类**（与 v1 §1.1 风格对齐）：
> - **区域专属**（13）：受地形 / decoration 锁定，重生稳定
> - **极稀 / 事件触发**（3）：lie_yuan_tai（portal_rift 联动）/ jing_xin_zao（灵气井相位）/ xue_po_lian（雪线漂移）
> - **双相位毒性**（1）：xue_se_mai_cao（白叶可入药 / 红叶毒性，按相位区分）

### §2.1 北荒废原 / waste_plateau（qi_base=0.05，neg_pressure 0.3-0.8）

| ID | 中文名 | Tier | SurvivalMode | EnvLock | HarvestHazard | base_mesh_ref | tint | 用途 |
|---|---|---|---|---|---|---|---|---|
| `fu_yuan_jue` | 负元蕨 | 区域专属 | NegPressureFeed | NegPressure(min=0.3) | QiDrainOnApproach(r=5, drain=0.4，**叠加于 zone 负灵压本身的抽吸**) | `large_fern` | `0x4A2E5A` 暗紫 | 逆灵符 / 负元丹（worldview §五 涡流流防御加成） |
| `bai_yan_peng` | 白盐蓬 | 区域专属 | SpiritCrystallize | （无 grid layer 锁；SurvivalMode 内置 daytime 限制） | DispersalOnFail(0.6) + AttractsMobs(`spirit_mice`, 2..5) **stub** | `dead_bush` | `0xF8F8E8` 灰白 | 低品灵石替代 / 回元散辅料 / 鼠群诱饵 |

**生存自洽**：
- **负元蕨**——负灵域是天地反向抽吸（worldview §二）。蕨叶反向利用这口"吸"作代谢，把外界负压抽进叶脉。**为何不在馈赠区生长**：正向灵气流压垮反向气孔，结构崩溃（NegPressureFeed 公式不读 zone.spirit_qi，但 EnvLock NegPressure(min=0.3) 自动排除馈赠区——**双重判定的悖论已修，统一走 grid 级 neg_pressure，无 zone 级判定**）。**为何近身吸玩家**：玩家在负灵域本就被抽吸（library 北荒坍缩渊记凝脉中期池"每息约失一点" ≈ 0.67/s），蕨叶与环境**同向**叠加 0.4/s，不是替换——这就是为什么 hazard 数值看起来温和但叠加后致命
- **白盐蓬**——末法残土灵气稀薄到几近"散粒态"，白盐蓬叶尖结晶腔把游离灵气逼着析出为可见盐晶。**为何夜间散尽**：盐晶结构需日照能量驱动持续析出；夜间不储是它的代谢现实。**为何引来鼠群**：盐晶外壁灵气波动 spectrum 与修士打坐释放波动同型——异兽三形考"压差网"识别为"山"

### §2.2 上古战场 / ancient_battlefield（双高 qi+mofa，ruin_density 高）

| ID | 中文名 | Tier | SurvivalMode | EnvLock | HarvestHazard | base_mesh_ref | tint | 用途 |
|---|---|---|---|---|---|---|---|---|
| `duan_ji_ci` | 断戟刺 | 区域专属 | RuinResonance | RuinDensity(0.3) + AdjacentDecoration("broken_spear_tree" \| "war_banner_post", r=1) | ResonanceVision(3s, composure -0.05) | `sweet_berry_bush` | `0x5C1E0F` 暗血 | 毒蛊飞针复合污染载体 / 伪心丹 |
| `xue_se_mai_cao` | 血色脉草 | 双相位毒性 | DualMetabolism | RuinDensity(0.2) | DispersalOnFail(0.4) 错相位采（5min 翻转窗口被打断 → 整株枯）| `tall_grass` + DualPhase overlay | 双相位（daytime `0xC03020` 红 / nighttime `0x205040` 青）| 白叶（昼）入清浊散；红叶（夜）入侵骨毒 |

> **xue_se_mai_cao 命名澄清**：v1 别名表 `XUE_CAO_ALIAS = "xue_cao"` 是 chi_sui_cao（血谷赤髓草）的旧俗名 alias；本 v2 物种 `xue_se_mai_cao` 是古战场新物种，与 xue_cao 不重合（地理分布、生存机制、canonicalize_herb_id 接受范围三层都区分）

**生存自洽**：
- **断戟刺**——上古战场堆积的不是"灵气"，而是**未中和的污染真元**（worldview §四 异体排斥）。断戟刺根扎金属遗物缝吸污染当能量。**为何近遗物才长**：污染必须物理接触金属载体，否则弥散过快无法捕获。**采集触发幻视**：怨念回响释放 —— 同款异兽三形考兽核 ingest HUD 干扰
- **血色脉草**——古战场是"灵气与怨念交织"的特殊场。双面叶演化：日面叶绿素吸 qi_density、夜面"叶赤素"吸 mofa_decay。每天黎明 / 黄昏自动翻转，翻转持续 5 min；期间被打断则代谢中断、整株枯萎

### §2.3 九霄浮岛 / sky_isle（qi_base=0.8，远程钩取才能采）

| ID | 中文名 | Tier | SurvivalMode | EnvLock | HarvestHazard | base_mesh_ref | tint | 用途 |
|---|---|---|---|---|---|---|---|---|
| `yun_ding_lan` | 云顶兰 | 区域专属 | QiAbsorb | SkyIslandMask(0.2, surface=Top) | DispersalOnFail(0.7)（极易吹散）| `lily_of_the_valley` | `0xE8F4FF` 银白 | 御物 / 飘逸色染色加速 / 轻身丹 |
| `xuan_gen_wei` | 悬根薇 | 区域专属 | PressureDifferential | SkyIslandMask(0.2, surface=Bottom) | WoundOnBareHand(LACERATION，required_tool=None → P0 退化 dispersal=1.0) | `vine` | `0x60D080` 翠绿 | 器修凝实色载体（灵木上位）/ 虹吸阵原料 |

**生存自洽**：
- **云顶兰**——浮岛顶离主地表 200+ 格，"高处不胜寒"反成立——空气稀薄、天地"吸力"弱（worldview §四 距离衰减），离体灵气衰减极慢；开放兰科结构正适合此区域吸收。**为何吹散即死**：根浅、结构靠浮岛顶面灵气稳定流维持；离开就散
- **悬根薇**——浮岛顶 0.8、主地表 0.3，悬根薇当天然虹吸管，根尖延伸到 200 格下吸取上行流（云顶兰未饱和的部分）。**为何根尖锐晶**：高速虹吸结晶化沉淀

### §2.4 无垠深渊 / abyssal_maze（三层 tier）

| ID | 中文名 | Tier | SurvivalMode | EnvLock | HarvestHazard | base_mesh_ref | tint | 用途 |
|---|---|---|---|---|---|---|---|---|
| `ying_yuan_gu` | 萤渊菇 | 区域专属 | PhotoLuminance | UndergroundTier(1) + AdjacentLightBlock(r=2) | DispersalOnFail(0.3) + AttractsMobs(`mimic_spider`, 1..2) **stub** | `red_mushroom` + Emissive | `0xFFA040` 暖橙 | 夜视丹 / 渊光阵光源 |
| `xuan_rong_tai` | 玄绒苔 | 区域专属 | ThermalConvection | UndergroundTier(2) | WoundOnBareHand(ABRASION，required_tool=None → P0 真元 -2 + dispersal=0.8) | `moss_carpet` + Emissive（银光）| `0x101015` 漆黑 | 替尸 / 蜕壳流伪灵皮高阶夹层 / 养经苔人工替代（效力 60%） |
| `yuan_ni_hong_yu` | 渊泥红玉 | 区域专属 | PressureDifferential | UndergroundTier(3) + AdjacentDecoration("yuan_ni_ebony", r=5) + QiVeinFlow(0.5) | DispersalOnFail(0.5) | `large_fern` | `0xC02040` 玉红 | 高阶炼器载体（堪比髓铁）/ 上品丹药主原料；herbs.toml 设 spirit_quality_initial=1.0（按 library 灵物磨损笔记的 1-5% 税扣得最狠） |

**生存自洽**：
- **萤渊菇**——cave_network 已有 glow_lichen 极致形态。菇盖发出微光被自身菌丝色素吸收，反向给灵脉补能（生物负反馈）。**为何必须邻接发光体**：自发光强度不足维持代谢，需邻接 shroomlight / amethyst / glow_lichen 提供启动光源
- **玄绒苔**——tier 2 是温度过渡带（tier 1 暖、tier 3 寒）；cave 通风带来微温差，玄绒苔靠这点温差驱动叶片细胞内的微循环。tier 1 / tier 3 单一温度场无梯度，苔藓饿死。**为何徒手失真元**：苔藓的"温差吸收"机制对接触体也生效，玩家手温 vs 周围岩温的差被它吸去
- **渊泥红玉**——tier 3 是 abyssal_maze 灵脉 cluster 处。worldview §二 馈赠区"灵气富集"在物理上对应这种深处脉口。红玉以 yuan_ni_ebony（黑石巨树）为共生伞——黑石树吸潮 + 阻挡负压扩散，红玉趁伞下吸饱漏气。**为何易磨损**：spirit_quality_initial=1.0 在 inventory 操作中按灵物磨损笔记的 1-5% 税扣得最狠

### §2.5 灵泉湿地 / spring_marsh（补"灵泉眼专属"）

| ID | 中文名 | Tier | SurvivalMode | EnvLock | HarvestHazard | base_mesh_ref | tint | 用途 |
|---|---|---|---|---|---|---|---|---|
| `jing_xin_zao` | 井心藻 | 极稀 / 事件触发 | WaterPulse | QiVeinFlow(0.6) + AdjacentDecoration("ling_yun_mangrove" \| "spirit_willow", r=8) + TimePhase(WaterPulsePhase::Open)（仅开期生长）| SeasonRequired(Open，错相位反吸玩家真元 -10) **P0–P3 stub** | `seagrass` + Emissive（微淡光）| `0x40A0A0` 翠青 | 水脉系丹方独立主料（如新建"井心散"——**不蹭灵眼石芝赛道**）/ 凝脉散高阶替补 |

**生存自洽**：
- **井心藻**——library 灵泉湿地探案确立"灵气井三日吐两日吸"机制，井心藻就是井口物种。开（吐）期吸饱、合（吸）期休眠不动。在合期被采 = 反吸玩家（井合期是反向气流）。**为何需红树共生**：红树根系在井口外圈织网，缓冲剧烈相位变化、保护藻不被瞬态吸力撕碎
- **不蹭 ling_yan_shi_zhi 用途**：v1 灵眼石芝是固元突破最佳辅料 + 灵眼实装前禁用；jing_xin_zao 是水脉系丹方独立主料，不蹭灵眼石芝赛道（保 v1 极稀价值不被稀释）

### §2.6 青云残峰 / broken_peaks（雪线以上）

| ID | 中文名 | Tier | SurvivalMode | EnvLock | HarvestHazard | base_mesh_ref | tint | 用途 |
|---|---|---|---|---|---|---|---|---|
| `xue_po_lian` | 雪魄莲 | 极稀 / 事件触发 | SpiritCrystallize | SnowSurface（**broken_peaks::SNOW_LINE_Y 常量；profile 漂移自动跟，不在 plan 硬编码数字**）+ QiVeinFlow(0.3) | WoundOnBareHand(无冰甲手套触碰则蒸发；required_tool=None → P0 退化 dispersal=1.0) | `lily_of_the_valley` | `0xF0F8FF` 极白 + 霜蓝 | 通灵境共鸣突破辅料（缩短奇经第 4 条 30%）/ 霜骨丹 |

**生存自洽**：
- **雪魄莲**——worldview §四 真元易挥发：低温抑制散失。高山雪线 + qi_vein_flow 是"低温保灵气"的稀有点位，灵气在低温下被封冻为霜晶慢释放。**为何手温即化**：霜晶结构对体温（37°C）相变临界点低；冰甲手套必须冷源材质（冰原狼皮 + 灵铁导冷）—— 由 plan-tools-v1 落地

### §2.7 血谷 / rift_valley（fracture_mask 高分裂）

| ID | 中文名 | Tier | SurvivalMode | EnvLock | HarvestHazard | base_mesh_ref | tint | 用途 |
|---|---|---|---|---|---|---|---|---|
| `jiao_mai_teng` | 焦脉藤 | 区域专属 | PressureDifferential | FractureMask(0.4) + AdjacentDecoration("fire_vein_cactus", r=3) | WoundOnBareHand(LACERATION 灼伤，required_tool=None → P0 退化 dispersal=1.0) | `weeping_vines` + Emissive（橙红芯）| `0x301010` 焦黑 | 暴烈色染色加速 / 狂雷丹 |

**生存自洽**：
- **焦脉藤**——rift_valley 是 lift / drop 交错带，岩浆产生的灼热反推灵气，藤蔓生于"对流真空"夹层。火脉熄灭则 1 天内枯。**为何超热**：藤须本体储存对流真空的高温能量作为"反辐射披风"维持灵脉稳定

### §2.8 TSY 坍缩渊位面（4 起源 × portal_rift）

> **TSY 物种 spirit_quality 论点澄清**（必修悖论修复）：worldview §十六 + plan-tsy-zone-v1 §0.6 的"入场过滤"只在玩家**从主世界进 TSY 的入口传送时**生效（剥离背包内 spirit_quality≥0.3 的凡物）；**TSY 内部生长的物种从未过此过滤**。但 TSY 位面内 zone.spirit_qi 全部为负值（zones.tsy.json 模板示例 -0.4 等），物种在负灵气环境内生长，结构里离体灵气**天然偏低**——这是物理上的而非过滤上的限制。所以 v2 TSY 物种在 herbs.toml 注册时 `spirit_quality_initial` 设 0.3-0.5（区间偏低），呼应 worldview §十六.三 上古遗物特征（"低灵压抑制老化，离体灵气稀薄"）。出关到主世界后这些 item 不再被任何过滤裁剪，按 initial 值正常使用。

| ID | 中文名 | Tier | TSY origin | SurvivalMode | EnvLock | HarvestHazard | base_mesh_ref | tint | 用途 |
|---|---|---|---|---|---|---|---|---|---|
| `lie_yuan_tai` | 裂渊苔 | 极稀 / 事件触发 | 任意 origin（仅 _shallow 入口子 zone）| PortalSiphon | PortalRiftActive | DispersalOnFail(0.4)（裂缝塌缩瞬间所有株消失）| `glow_lichen` + Emissive | `0x402060` 紫黑 | 渊息丹（5s 抗负灵压）/ 地师伪虚阵载体 |
| `ming_gu_gu` | 冥骨菇 | 区域专属 | tsy_zhanchang | RuinResonance | RuinDensity(0.4) + AdjacentDecoration("bone_mountain", r=3) | ResonanceVision(5s, composure -0.08) + AttractsMobs(`mimic_spider`, 1..3) **stub** | `brown_mushroom` | `0xE8E0D0` 骨白 | 替尸流骨灰夹层 / 骨魂丹（一次性回魂 5 真元）|
| `bei_wen_zhi` | 碑文芝 | 区域专属 | tsy_zongmen_ruin | RuinResonance | RuinDensity(0.3) + AdjacentDecoration("array_disc_remnant", r=2) | DispersalOnFail(0.5)（碎裂后阵纹一并散）| `red_mushroom` | `0x808890` 灰青 + `0x6020A0` 紫纹 | 阵法师摹阵纸（拓印 20 种残阵图样）|
| `ling_jing_xu` | 灵晶须 | 区域专属 | tsy_daneng_crater | PressureDifferential | AdjacentDecoration("qi_crystal_pillar", r=3) + QiVeinFlow(0.5) | WoundOnBareHand(ABRASION，required_tool=None → P0 退化 dispersal=1.0) + DispersalOnFail(0.6) | `twisting_vines` + Emissive | `0xA060FF` 紫晶 | 器修上古载体强化（一次性给上古遗物 +1 耐久）|
| `mao_xin_wei` | 茅心薇 | 区域专属 | tsy_gaoshou_hermitage | **RuinResonance**（**修正：原 QiAbsorb 在 TSY 负灵气位面不成立**）| AdjacentDecoration("thatched_hermitage" \| "lone_grave_mound" \| "daily_artifact_cache", r=2) | （无）| `wheat`（生长阶段 7）| `0xE8C040` 暖黄 | 医道温润色染色加速 / 心安散 |

**生存自洽**：
- **裂渊苔**——位面间裂缝是"压差泵"，能量自然从主世界（正灵气）流向 TSY（负灵气）；裂渊苔正好生在缝口处吸"压差能"（PortalSiphon 公式恒定 +0.05/tick，**不依赖 zone qi**）。裂缝塌缩 = 泵停 = 苔同步消失。**为何只在 _shallow 入口子 zone**：plan-tsy-zone-v1 zones.tsy.json 模板中 portal_rift active_event 仅挂在 _shallow（入口）子 zone；mid / deep 没有
- **冥骨菇**——worldview §十六.一 战场沉淀类。古战场遗骸的怨念在低负压下凝结成菇形菌体（异兽三形考"求生冲动凝聚成兽核"的植物版同构）。**为何 RuinResonance 在 TSY 也成立**：RuinResonance 公式吸的是 grid 级 ruin_density（worldgen layer），与 zone.spirit_qi 正负无关——所以可以在 TSY 负灵气位面成立
- **碑文芝**——宗门遗迹类阵盘残片有微弱真元波动；灵芝菌丝攀附阵纹把波动锁住成芝形。**摹阵用途**：碑文芝菌丝结构本身是阵纹的"低保真复印件"
- **灵晶须**——大能陨落类灵气结晶柱有微小漏点（柱体并非完美），灵晶须以漏点为锚生成。**强化上古遗物**：须本身是"漏出灵气的低维稳定态"，反向"补"已耗损的上古遗物
- **茅心薇**（mode 修正：QiAbsorb → RuinResonance）——hermitage 是 4 origin 中最"温和"的，但仍在 TSY 负灵气位面（zone.spirit_qi < 0），QiAbsorb 公式不成立。改 RuinResonance：高手长年隐居渗透到茅草、墓土的"日常残灵"被薇吸取（worldview §十六.一 "近代高手战死/渡劫未果"——这里轻微泛化为"近代高手隐居遗痕"）。**为何只在 hermitage origin**：其他 3 origin 不存在 hermitage decoration（thatched_hermitage / lone_grave_mound / daily_artifact_cache 是 hermitage 专属生成）

---

## §3 MVP

### §3.1 测试 5 物种（每物种代表不同 SurvivalMode + 不同 hazard 组合）

| 物种 | SurvivalMode | 验证意图 |
|---|---|---|
| `fu_yuan_jue` | NegPressureFeed | grid neg_pressure 读取 + EnvLock NegPressure + QiDrainOnApproach（**叠加于环境抽吸**的最尖锐镜像） |
| `bai_yan_peng` | SpiritCrystallize | daytime 限制 + DispersalOnFail + AttractsMobs（stub）|
| `ying_yuan_gu` | PhotoLuminance | UndergroundTier + AdjacentLightBlock + emissive 渲染（abyssal_maze tier 1）|
| `jiao_mai_teng` | PressureDifferential | fracture_mask + AdjacentDecoration + WoundOnBareHand 退化路径（替原 xue_po_lian——避免与 bai_yan_peng 同 mode） |
| `lie_yuan_tai` | PortalSiphon | active_events 检查 + portal_rift 同步消失 + 恒定生长（不依赖 zone qi） |

> 5 物种覆盖 5 种**不同** SurvivalMode（必修悖论修复——原 MVP 含两个 SpiritCrystallize 重复）+ 5 类不同 EnvLock + 4 类不同 hazard，跑通即证 §1.1–§1.3 三抽象正确。

### §3.2 P0 范围

- [ ] `SurvivalMode` / `EnvLock` / `HarvestHazard` 三 enum + 字段加入 `BotanyKindRegistry::BotanyPlantKind`
- [ ] 5 个 MVP 物种 const + Hash 项注册（library 入卷 → registry 加 const）
- [ ] `TerrainProvider::sample_layer(pos, layer_name) -> Option<f32>` 接口（**P0 关键依赖**——v1 现状 only height/biome 有便捷 API，需补 mmap raster 多通道接口；约 80-150 行 Rust）
- [ ] `EnvLockChecker::check(plant, pos, terrain, zone) -> bool` 纯函数 + 单测覆盖 5 物种各 EnvLock 类型
- [ ] `DecorationManifest` 注册：worldgen 9 profile 现有 16 种 decoration name → 主导 BlockKind 集合 + 形状 pattern
- [ ] `BotanyTick.spawn_v2`：先按 SurvivalMode 选 base 公式，再 EnvLockChecker 过滤候选格
- [ ] `BotanyTick.wither_v2`：每 tick 复检 EnvLock；不再满足 → wither_progress + 死亡归 zone.spirit_qi（**不写 layer**）
- [ ] e2e：`fu_yuan_jue` 在 neg_pressure>0.3 区域生成；玩家走近触发 真元漏（**叠加 zone 抽吸**）；离开停漏；NegPressure 失效后枯死归还 zone.spirit_qi

### §3.3 客户端 MVP

- [ ] `bong:botany_plant_v2` entity type 注册（v1 已有 Plant Component；v2 此 entity 类型扩展共享）
- [ ] `BotanyPlantEntityRenderer` 按 plant_id 查 PlantRenderProfile + base_mesh_ref + tint_rgb
- [ ] 5 物种 vanilla mesh + tint 生效（screenshot 比对）
- [ ] Emissive overlay 实装（ying_yuan_gu）
- [ ] 5 物种 item icon（gen-image item × 5 张）+ 抠图 + 入 herbs.toml + InventoryStateStore drop 显示
- [ ] HarvestSessionHud 加 hazard 提示行（"靠近 -0.4 真元/s 叠加" / "无工具采空 100%"）

---

## §4 数据契约

### §4.1 server 侧

```rust
// botany/plant_kind.rs 扩展（v1 BotanyPlantKind 现有字段保留，v2 新增）
pub struct BotanyPlantKind {
    pub id: BotanyPlantId,
    // v1 已有：display_name / growth_cost / rarity / biomes / ...
    //   注：v2 物种 biomes 字段留空，env_locks 为唯一筛选——避免 BiomeTag vs EnvLock 双轨
    // v2 新增：
    pub survival_mode: SurvivalMode,
    pub env_locks: Vec<EnvLock>,
    pub harvest_hazards: Vec<HarvestHazard>,
    pub base_mesh_ref: &'static str,             // vanilla model resource id
    pub tint_rgb: u32,                           // 0xRRGGBB
    pub tint_rgb_secondary: Option<u32>,         // DualPhase 物种第二相位 tint
    pub model_overlay: ModelOverlay,             // None / Emissive / DualPhase
    pub icon_prompt: &'static str,               // gen-image item 用
}

pub enum ModelOverlay {
    None,
    Emissive,        // 自发光层（RenderLayer.eyes 风格）
    DualPhase,       // 昼/夜双色（仅 xue_se_mai_cao）
}
```

- [ ] **`EnvLockChecker`**：纯函数 `check(plant, pos, terrain, zone) -> bool` + 单测覆盖 11 种 EnvLock 各分支
- [ ] **`HarvestHazardApplier`**：HarvestSession start/tick/complete 三时机调用；stub 类型注册不报错
- [ ] **`BotanyTick.spawn_v2 / wither_v2`**：v2 物种独立路径，不动 v1 spawn 路径
- [ ] **schema**：`schema/botany.rs` 加 `BotanyPlantV2RenderProfileV1` payload（含 base_mesh_ref + tint + tint_secondary + overlay，启动期一次性推 client）
- [ ] **`DecorationManifest`**：worldgen 启动期注册 `{decoration_name → DecorationDescriptor { primary_blocks: Vec<BlockKind>, shape_pattern: ShapePattern }}`；EnvLock AdjacentDecoration 检查时按 primary_blocks 在 ChunkLayer 半径内扫描

### §4.2 client 侧

- [ ] `bong:botany_plant_v2` entity type 注册
- [ ] `BotanyPlantEntityRenderer extends EntityRenderer<BotanyPlantEntity>`
- [ ] 启动期接收 `BotanyPlantV2RenderProfileV1` payload 建注册表
- [ ] `BotanyEmissiveLayer`（Fabric RenderLayer.eyes 风格）—— overlay==Emissive 时叠
- [ ] `BotanyDualPhaseColor` —— overlay==DualPhase 时按 `world.timeOfDay` 切 tint_rgb / tint_rgb_secondary
- [ ] `HarvestSessionHud` hazard 提示行
- [ ] `ResonanceVisionOverlay` —— 复用 v1 缝合兽核 ingest 同款 HUD shader（v1 未实装则本 plan 内补；仅采集者本人可见，半径 0）

### §4.3 资产

- [ ] 17 张 item icon：`client/.../textures/gui/items/{plant_id}.png`
- [ ] icon prompts 集中文件：`scripts/images/prompts/botany_v2.toml`（17 条 prompt 一次 lock 后批量跑）
- [ ] library 入卷：`docs/library/ecology/末法残土后录·新十七味.json`（plan 落地前必须先写 + `/review-book` 通过）

---

## §5 阶段划分

| Phase | 内容 | 验收 |
|---|---|---|
| P0 | 三抽象 + 5 MVP 物种 + TerrainProvider::sample_layer + DecorationManifest + e2e（fu_yuan_jue spawn / 近身吸 / 失锁枯）| 5 MVP 各按声明的 EnvLock 在对应 profile 生成；不在对应 profile 不生成；wither 归 zone.spirit_qi 不写 layer |
| P1 | 余下 **12** 物种注册 + 9 worldgen profile spawn rule 全接入 | 9 profile 各能跑出 1+ 对应物种（screenshot + log）|
| P2 | HarvestHazard 全实装（fauna / 工具 / 相位 stub）+ HarvestSessionHud hazard 行 + ResonanceVision HUD 干扰复用缝合兽核同源 shader | 所有 hazard 在 e2e 中可观测（screenshot / 单测）；stub 类不报错 |
| P3 | 17 张 item icon（gen-image 批量）+ entity 渲染 + emissive layer + DualPhase tint 切换 | 截图比对：每个新物种在游戏内可视，颜色与 plan 一致；DualPhase 物种昼/夜切换正确 |
| P4 | alchemy 配方：6+ 种 v2 物种入丹方（fu_yuan_jue→负元丹 / xue_po_lian→霜骨丹 / yuan_ni_hong_yu→上品回元丹 等）；forge 载体：xuan_gen_wei / yuan_ni_hong_yu 接 BlueprintRegistry；zhenfa 原料：lie_yuan_tai / bei_wen_zhi 入阵法配方；同时回填 WoundOnBareHand 真伤（依 plan-tools-v1 落地） | 跨 plan e2e：从采集到丹方 / 阵法 / 锻造可消费完整链 |
| P5 | 极稀触发：portal_rift 开闭驱动 lie_yuan_tai 同步生灭；灵气井相位驱动 jing_xin_zao（依 plan-lingtian-weather-v1）；雪线漂移驱动 xue_po_lian 重定位；plan-fauna-v1 立后回填 AttractsMobs 真 spawn | agent narration 中能感知 v2 物种生灭（zone 聚合 ecology channel）|

---

## §6 跨 plan 钩子

- [ ] **plan-botany-v1**（finished）：复用 BotanyKindRegistry / harvest-popup / wither / Plant Component；BotanyPlantKind struct 扩展是兼容增量；不动 v1 22 种 ID
- [ ] **plan-lingtian-v1**（merged）：v2 物种**只进 BotanyKindRegistry，不进 PlantKindRegistry / SeedRegistry**——lingtian 完全不受影响（v2 物种没有 cultivable 字段——该字段是 PlantKindRegistry 专有）
- [ ] **plan-tsy-zone-v1 / plan-tsy-dimension-v1 / plan-tsy-worldgen-v1**：5 种 TSY 物种依赖三 plan 提供的位面 + zone + portal_rift POI + decoration anchor；**v2 P5 需 TSY 全栈实装到位**
- [ ] **plan-alchemy-v1**：P4 接入 v2 物种到丹方
- [ ] **plan-forge-v1**：P4 把 xuan_gen_wei / yuan_ni_hong_yu 作为高阶载体接 BlueprintRegistry
- [ ] **plan-zhenfa-v1**（未立 / 骨架）：P4 把 lie_yuan_tai / bei_wen_zhi 作为阵法原料；本 plan 留 hook
- [ ] **plan-mineral-v2**（骨架）：共享 worldgen layer 接入范式（`TerrainProvider::sample_layer`）
- [ ] **plan-skill-v1**：herbalism XP 与 v1 一致；**WoundOnBareHand 等危险采集**给 +50% XP（高风险高回报）—— 待 skill 系统接入后回填
- [ ] **plan-fauna-v1**（**待立 — 强依赖**）：AttractsMobs hazard 真 spawn 需此 plan；P0–P3 stub 占位；P5 接入
- [ ] **plan-tools-v1**（**待立 — 强依赖**）：WoundOnBareHand 的 required_tool（采药刀 / 灵铲 / 灵镰 / 冰甲手套 / 骨骸钳 / 灵铁夹 / 刮刀）—— P0–P3 退化为 dispersal=1.0；P4 工具系统立后回填真伤
- [ ] **plan-lingtian-weather-v1**（骨架 — 强依赖）：灵气井相位 driver；jing_xin_zao SeasonRequired hazard 与 WaterPulse mode 依赖；P0–P3 stub，P5 接入
- [ ] **plan-narrative-v1**（骨架）：每种 v2 物种给 agent narration 提供 1-3 个语调模板
- [ ] **agent / tiandao**：BotanyEcologySnapshotV1 channel 自动包含 v2 物种（v1 已建好 channel）；agent narration 模板可引用 v2 物种语义（"北荒今见负元蕨遍野" 等）

---

## §7 TODO / 开放问题（v3+）

- [ ] **采集工具系统**：冰甲手套 / 骨骸钳 / 灵铁夹 / 刮刀 / 采药刀 / 灵铲 / 灵镰——本 plan P0–P3 不实装；待 plan-tools-v1（待立）
- [ ] **季节系统**：v2 暂用 in-game daytime/nighttime 二分；完整四季 + 灵气井相位待 plan-lingtian-weather-v1
- [ ] **变异系统**：v1 已有 PlantVariant::Thunder / Tainted；v2 是否新增 Cracked（断戟刺被采过的"已警觉"变种）/ Frosted（雪魄莲休眠态）/ Withered（裂渊苔塌缩前夜）—— 留 v3
- [ ] **layer mutability 系统**：v2 NegPressureFeed / RuinResonance 物种生长理论上消耗 grid 级 layer 信号（neg_pressure / ruin_density），但 worldgen layer 当前是只读 raster；待独立 NegPressureAccount / RuinDensityAccount 等"可写 layer"系统建立后再实装层间消耗（v2 P0–P5 仅在 zone.spirit_qi 维度归还，不写 layer）
- [ ] **TSY 物种与上古遗物耐久绑定**：灵晶须（ling_jing_xu）能补上古遗物耐久——耐久系统是 worldview §十六.三 / plan-tsy-loot-v1 范畴；本 plan P4 仅留 hook
- [ ] **agent 语义模板**：每种 v2 物种给 agent narration 提供 1-3 个语调模板—— 待 plan-narrative-v1（骨架）
- [ ] **library JSON 入卷**：v2 落地前 `docs/library/ecology/末法残土后录·新十七味.json` 必须先写完并通过 `/review-book` 流程（CLAUDE.md "worldview 正典优先"原则）
- [ ] **sky_isle 进 worldview 正典**：本 plan 暂将 sky_isle 物种锚到"青云残峰高空延伸"；worldview 主文档若立"九霄浮岛"为正式区域，则回头补正
- [ ] **ancient_battlefield / abyssal_maze 进 worldview 正典**：同上——本 plan 已在 §0 关键决策 1 给出过渡映射，但终究是 worldgen profile 维度而非六域；待 worldview 主文档收编

---

## §8 风险与对策

| 风险 | 对策 |
|---|---|
| 玩家把 fu_yuan_jue / lie_yuan_tai 当 farm 站 | 这些物种**不可种植 + 采集即环境消耗 + 高风险**——farm 在物理上不划算；fu_yuan_jue 的 QiDrainOnApproach **叠加于负灵域本身抽吸**，玩家在该区域真元几分钟见底 |
| 17 张 icon 风格漂移 | gen-image 批量前一次 lock prompts toml；统一 prompt prefix（worldview lore + STYLE_ITEM）；先跑 1 张 review、再批量；prompt 单条可迭代不调全局 prefix |
| 双 spawn 路径（v1 zone-based + v2 layer-based）混乱 | v2 物种 spawn 走独立 `BotanyTick.spawn_v2` / `wither_v2`；不动 v1 spawn 路径 |
| TSY 入场过滤与 v2 TSY 物种关系误读 | 已澄清（§2.8 引言）：TSY 物种从未过入口过滤；spirit_quality 偏低是物理现实非过滤强制；出关后正常使用 |
| **ColorProvider 多对一限制（必修悖论）** | **走 entity 渲染路线（v1 现状）**——单一 entity 类型 + 按 plant_id 查 PlantRenderProfile 着色；不走 BlockColorProvider |
| BlockEntity / 独立 BongBlock 路线作为 fallback | 若 P3 entity renderer 性能不足或 v1 entity 路线变更，可 fallback 到每物种独立 BongBlock 继承 vanilla（17 个新 BlockKind 注册）；P3 优先走 entity，开销最小 |
| WoundOnBareHand 在工具系统未实装时变成"100% 空手 wound" | MVP 用 `required_tool=None` → 退化为 `dispersal_chance=1.0`（玩家空手只是采空，不挂真伤）；P4 工具立后回填真伤——避免 P0–P3 错挂未本意的伤口档 |
| AttractsMobs 在 fauna 系统未实装时无效 | hazard 字段保留作 stub，注册不报错；P5 fauna 立后回填 |
| SeasonRequired 在相位系统未实装时无效 | 同上，stub 占位；P5 相位 driver 立后回填 |
| jing_xin_zao 整株物种依赖未立的相位系统 | P0–P3 注册但**不刷新**（spawn rate=0）；玩家在 P0–P3 中看不到此物种是设计；P5 上线 |
| 双相位（xue_se_mai_cao 白叶/红叶）增加 inventory item 数量 | 单 ID + NBT meta_tag 区分（昼/夜）；不新建 item ID |
| 共生关系（红玉↔黑石树 / 井心藻↔红树）spawn 时机未同步 | spawn 顺序：worldgen 先生成 decoration（chunk 加载）→ server BotanyTick 在 chunk 加载完成 callback 内 spawn 共生植物；不在 worldgen 同 tick 互依（避免 race） |
| ResonanceVision HUD 干扰过强干扰 PVP | 仅作用于采集者 client，半径 0；不影响他人 |
| 雪线 / portal_rift / 灵气井相位变动后已 spawn 物种是否同步消失 | 每 wither tick 复检 EnvLock；任一转 false → wither_progress + 死亡归 zone.spirit_qi（DispersalOnFail 与 wither 是两路：前者由 session 失败触发，后者由 EnvLock 持续不满足触发）|
| `TerrainProvider::sample_layer` 接口缺口 | **P0 关键依赖** —— 现状只有 height/biome 便捷 API，需补 mmap raster 多通道（约 80-150 行 Rust）；缺则 P0 卡住，物种无法注册——实施备忘 §9 已列 |
| layer 写回需求（v3+） | 暂不开口子；layer 只读，wither 归 zone.spirit_qi——避免 v2 偷偷引入"可写 layer"成既成事实 |

---

## §9 实施备忘（开工前的待 review 项）

- [ ] **library 入卷优先（CLAUDE.md "worldview 正典优先"）**：开工前先写 `docs/library/ecology/末法残土后录·新十七味.json`，`/review-book` 通过后再回本 plan 钉 ID；任何代码注册早于 library 入卷视为违规
- [ ] **`TerrainProvider::sample_layer` 接口审计**：P0 关键依赖。v1 现状 `terrain/raster.rs` 有单 layer mmap 读（height / biome），但 multi-layer 通用接口需要审。如缺口存在，P0 必须先补此接口（约 80-150 行 Rust），再做物种注册——见 P0 验收
- [ ] **`DecorationManifest` 完整性审计**：worldgen 9 profile 现有 16 种 decoration name 必须穷举到 `DecorationManifest`（每 decoration → 主导 BlockKind 集合 + shape pattern）；缺一即 EnvLock AdjacentDecoration 失效
- [ ] **gen-image prompts 一次 lock**：`scripts/images/prompts/botany_v2.toml` 17 条先写完、人工 review 一遍、再批量跑；批量跑后 review 整体风格、若漂移立即调单条 prompt 重跑（不调全局 prefix）
- [ ] **TSY 物种与 plan-tsy-* 实装顺序**：建议 v2 P0–P3 先做主世界 12 种（北荒 2 + 古战场 2 + 浮岛 2 + abyssal 3 + 湿地 1 + 雪线 1 + 血谷 1 = 12）；TSY 5 种留到 P4–P5（待 TSY 全栈成熟后接入）—— P1 注册时声明 TSY 5 种但不开 spawn

按下不表。
