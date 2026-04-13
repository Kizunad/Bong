# Bong · plan-lingtian-v1 · 模板

**灵田 / 农业专项**。HUD 事件流固定 channel `bong:system/notice · 灵田 tick`，是长线玩法核心。本 plan 定义：田块 / 作物 / tick 节奏 / 产出。

**交叉引用**：`plan-worldgen-v3.1.md`（地形/灵气）· `plan-HUD-v1.md §2.3` · `plan-alchemy-v1.md`（作物 → 丹方材料）。

---

## §0 设计轴心

- [ ] 长线低操作：离线也在生长
- [ ] 灵气密度 = 核心因子
- [ ] 区位 × 作物 × 季节
- [ ] NPC 也会种（农业社会）

## §1 田块模型

```rust
pub struct LingtianPlot {
    pub pos: BlockPos,
    pub crop: Option<CropId>,
    pub growth: f32,          // [0, 1]
    pub quality_modifier: f32,
    ...
}
```

- [ ] 田块来源：玩家开垦 / NPC 已有 / 随机生成
- [ ] 质量影响因子（地形 · 灵气 · 水源）

## §2 作物表

| 作物 | 生长时长 | 用途 | 灵气需求 |
|---|---|---|---|
| 灵稻 | | 凡人口粮 / 低阶丹 | 低 |
| 百草 | | 治疗丹药 | 中 |
| 雷竹 | | 炼器 | 高 + 雷属 |
| ... | | | |

## §3 Tick 节奏

- [ ] 每 N 分钟推进一次
- [ ] 离线进度如何结算
- [ ] 节流到 HUD 事件流规则（按 §6.1 cultivation event 3/s）

## §4 产出与交易

- [ ] 收获 UI
- [ ] 品质分级
- [ ] NPC 市场价格

## §5 数据契约

- [ ] LingtianStore / CropRegistry
- [ ] Channel `bong:lingtian/tick`, `bong:lingtian/harvest`

## §6 实施节点

## §7 开放问题
