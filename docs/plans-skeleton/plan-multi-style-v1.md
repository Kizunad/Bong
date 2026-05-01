# Bong · plan-multi-style-v1 · 骨架

全流派精通路径。玩家可解锁全部 7 流派,但代价是阶段停留时间 + 突破要求叠加 + 染色必须为混元色(单一不超 25% 总修炼时长)。**对应 plan-gameplay-journey-v1 §A.5**。

**世界观锚点**：`worldview.md §六 line 1`(路径倾向) · `§六.二 真元染色(混元色存疑)`

**library 锚点**：`cultivation-0005 真元十一色考`(混元色章节)

**交叉引用**：`plan-style-pick-v1` ⬜ · `plan-cultivation-canonical-align-v1` ⬜ · `plan-cultivation-v1` ✅(QiColor 染色) · `plan-gameplay-journey-v1` §A.5/O.14

---

## 接入面 Checklist

- **进料**：UnlockedStyles 当前激活数量 + QiColor 主色检测 + 各流派累计修炼时长
- **出料**：突破要求加成公式 + 混元色检测 + 多流派激活时的真元池要求 +25%(7 流派) + 战斗效率 -20%
- **共享类型**：扩展 `Cultivation` 增 `style_count_modifier` 字段 + `QiColor::Hunyuan` ✅(已实装)
- **worldview 锚点**：§六 line 1 + §六.二

---

## §0 设计轴心

- [ ] **代价递增**：1 流派 100h / 2 流派 110h / 3 流派 120h / 全 7 流派 135h
- [ ] **染色锁**：任一流派修炼超过总时长 25% → 主色锁定;要混元色必须刻意均衡
- [ ] **混元色 -20%**：所有流派伤害 ×0.8,但**不被任何单流派克制**(战略价值)
- [ ] **真元池 +25%(7 流派满)**：突破节点要积更多真元
- [ ] **不可绕过**：经脉拓扑/突破丹/渡虚劫等硬门槛不变

---

## §1 阶段总览

| 阶段 | 内容 | 验收 |
|---|---|---|
| **P0** ⬜ | UnlockedStyles 多激活支持(原仅单/双激活,扩到 7) | 7 流派可同时激活 |
| **P1** ⬜ | 突破要求加成公式: `required_qi_pool ×= (1 + 0.04 × (n-1))` | 全 7 流派玩家真元池要求 1.24x ≈ +25% |
| **P2** ⬜ | 混元色硬规则: 任一流派修炼时长 > 25% 总时长 → 主色锁,无法成混元 | 单元测试覆盖 |
| **P3** ⬜ | 战斗效率 -20% 应用(混元色玩家所有流派伤害 ×0.8) | 与 §P 矩阵交叉验证 |
| **P4** ⬜ | client UI: 显示当前激活流派数 + 染色趋向 + 突破要求加成 | 玩家可见代价 |

---

## §2 关键公式

```
required_qi_pool(realm, style_count) = base_qi_pool(realm) × (1 + 0.04 × max(0, style_count - 1))

例(通灵期 base = 300):
  单流派:    300
  双流派:    312  (+4%)
  三流派:    324  (+8%)
  全 7 流派: 372  (+24%)  ← 接近 §A.5 表的 +25%
```

混元色判定：
```
is_hunyuan = (max(style_qi_time[i] / total_qi_time) for i in styles) <= 0.25
```

---

## §3 数据契约

- [ ] `server/src/cultivation/style_count.rs` 多激活计数 + 公式
- [ ] `server/src/cultivation/breakthrough.rs::required_qi_pool` 加成
- [ ] `server/src/cultivation/qi_color.rs::lock_main_color` 25% 阈值检测
- [ ] `server/src/cultivation/qi_color.rs::is_hunyuan` 混元色判定
- [ ] `client/.../cultivation/StyleSummaryHud.java` 显示激活流派数 + 加成

---

## §4 开放问题

- [ ] 突破时累计修炼时长统计精度(每秒/每分钟/每境界)?
- [ ] "时长占比 25%" 是按真元修炼时长还是按攻击次数?
- [ ] 混元色 -20% 战斗效率是统一减还是按流派减?
- [ ] 是否允许玩家"洗色"(中性环境长时间静坐重置染色)? worldview §六 提到可洗,需确认机制
- [ ] +4% per style 的系数能否直接走 telemetry 校准(像 §P)?

## §5 进度日志

- 2026-05-01：骨架创建。plan-gameplay-journey-v1 §A.5 / O.14 决策落点。
