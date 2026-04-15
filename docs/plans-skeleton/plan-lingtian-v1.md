# Bong · plan-lingtian-v1 · 模板

**灵田专项**。HUD 事件流固定 channel `bong:system/notice · 灵田 tick` 是长线钩子，本 plan 定义灵气零和约束下的种植模型。

**世界观锚点**：`worldview.md §十` "全服灵气总量固定（SPIRIT_QI_TOTAL=100）+ 天道缓慢回收"——灵田**消耗的是区域灵气**，种得越多区域越贫瘠。`worldview.md §七` 灵物密度阈值——大型灵田会被天道盯上。`worldview.md §六` 真元只有"染色谱"没有"五行属"——不得引入"雷竹/火稻"等五行属性作物。

**交叉引用**：`plan-worldgen-v3.1.md`（地形/灵气）· `plan-HUD-v1.md §2.3` · `plan-alchemy-v1.md`（作物 → 丹方材料）· `worldview.md §六/§七/§十`。

---

## §0 设计轴心

- [ ] 灵田吸的是**区域灵气**（零和），种田 ≠ 凭空创造
- [ ] 长线低操作：离线也在生长，但灵气池见底就停
- [ ] 大型灵田触发天道密度阈值 → 高阶道伥/灵气清零
- [ ] NPC 散修也种，但规模都不大（worldview §十一 默认敌对，无大农庄）

## §1 田块模型

```rust
pub struct LingtianPlot {
    pub pos: BlockPos,
    pub crop: Option<CropId>,
    pub growth: f32,           // [0, 1]
    pub local_qi_drain: f32,   // 当前 tick 从区域 SPIRIT_QI 抽取量
    ...
}
```

- [ ] 田块来源：玩家开垦 / 野生灵植斑块
- [ ] 质量影响因子（地形 · 区域灵气浓度 · 水源）
- [ ] 抽取量直接计入 `zone.spirit_qi` 衰减

## §2 作物表（按用途，不按"五行属"）

| 作物 | 生长时长 | 用途 | 灵气需求 |
|---|---|---|---|
| 灵稻 | | 凡人口粮 / 低阶丹基底 | 低 |
| 百草 | | 通用治疗丹药材料 | 中 |
| 开脉草 | | 开脉丹（worldview §三 突破辅助）| 中 |
| 凝脉藤 | | 凝脉散 | 中高 |
| 灵眼旁稀有灵植 | 极慢 | 固元/通灵突破辅助 | 极高（仅灵眼旁可种）|

## §3 Tick 节奏

- [ ] 每 N 分钟推进一次
- [ ] 离线进度结算（按区域灵气池可支撑量截断）
- [ ] 区域灵气 < 阈值 → 生长停滞，不会"借不存在的灵气长"
- [ ] 节流到 HUD 事件流（按 §6.1 cultivation event 3/s）

## §4 产出与天道反制

- [ ] 收获 UI / 品质分级
- [ ] 大型灵田触发密度阈值后果（worldview line 480）
- [ ] 「欺天阵」与灵田配合（伪装权重）
- [ ] NPC 散修偶发交易（不做"市场系统"）

## §5 数据契约

- [ ] LingtianStore / CropRegistry
- [ ] Channel `bong:lingtian/tick`, `bong:lingtian/harvest`
- [ ] 接入 `zone.spirit_qi` 抽取/天道密度计算

## §6 实施节点

## §7 开放问题
